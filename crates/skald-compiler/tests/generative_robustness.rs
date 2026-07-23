//! Deterministic generative coverage for hostile frontend input.
//!
//! The compiler source boundary accepts UTF-8 text. Arbitrary bytes are fed
//! through the same boundary after deterministic lossy UTF-8 decoding, while a
//! separate generator covers valid Unicode scalar sequences directly.

use std::{any::Any, panic};

use skald_compiler::{
    diagnostics::render_diagnostics,
    lexer::lex,
    source::SourceDatabase,
    syntax::{parse, EXCESSIVE_NESTING, MAX_SYNTAX_NESTING},
};

const DEFAULT_GENERATED_CASES: usize = 256;
const MAX_GENERATED_BYTES: usize = 1_024;
const BYTE_SEED: u64 = 0x6a09_e667_f3bc_c909;
const UTF8_SEED: u64 = 0xbb67_ae85_84ca_a73b;
const VALID_CHARACTERS: &[char] = &[
    '\0', '\n', '\r', '\t', ' ', '(', ')', '{', '}', ':', ';', ',', '.', '+', '-', '*', '/', '=',
    '0', '1', '8', '9', 'a', 'c', 'd', 'e', 'f', 'i', 'l', 'n', 'r', 's', 't', 'u', 'x', '_', 'é',
    'λ', '中', '🦀',
];

#[test]
fn arbitrary_bytes_and_utf8_never_panic_in_the_frontend() {
    let cases = generated_case_count();
    exercise_generated_bytes(cases);
    exercise_generated_utf8(cases);
    exercise_class_header_mutations();
}

fn exercise_class_header_mutations() {
    const SEED: &str = "class Derived extends Base { init() {} }";

    for index in 0..SEED.len() {
        let mut deletion = SEED.to_owned();
        deletion.remove(index);
        assert_frontend_does_not_panic(&format!("class-header-delete-{index}"), &deletion);
    }
    for index in 0..=SEED.len() {
        let mut insertion = SEED.to_owned();
        insertion.insert_str(index, " extends ");
        assert_frontend_does_not_panic(&format!("class-header-insert-{index}"), &insertion);
    }
}

#[test]
fn hostile_inputs_terminate_at_the_existing_syntax_resource_limit() {
    let excessive_groups = format!(
        "fn main() -> i64 {{ return {}0{}; }}",
        "(".repeat(MAX_SYNTAX_NESTING + 32),
        ")".repeat(MAX_SYNTAX_NESTING + 32),
    );
    let excessive_blocks = format!(
        "fn main() -> i64 {{ {}return 0;{} }}",
        "{".repeat(MAX_SYNTAX_NESTING + 32),
        "}".repeat(MAX_SYNTAX_NESTING + 32),
    );

    for (name, text) in [
        ("excessive-groups", excessive_groups),
        ("excessive-blocks", excessive_blocks),
        (
            "retained-malformed-source",
            include_str!("../../../tests/compiler/robustness/frontend/malformed.ska").to_owned(),
        ),
        (
            "retained-byte-sequence",
            String::from_utf8_lossy(&decode_hex(include_str!(
                "../../../tests/compiler/robustness/frontend/arbitrary-bytes.hex"
            )))
            .into_owned(),
        ),
    ] {
        let diagnostics = run_frontend_case(name, &text);
        if name.starts_with("excessive-") {
            assert!(
                diagnostics.contains(&EXCESSIVE_NESTING),
                "{name} did not report the syntax nesting limit"
            );
        }
    }
}

fn exercise_generated_bytes(cases: usize) {
    let mut random = DeterministicRandom::new(BYTE_SEED);
    for index in 0..cases {
        let bytes = random.bytes(MAX_GENERATED_BYTES);
        let text = String::from_utf8_lossy(&bytes);
        assert_frontend_does_not_panic(&format!("bytes-{index}"), &text);
    }
}

fn exercise_generated_utf8(cases: usize) {
    let mut random = DeterministicRandom::new(UTF8_SEED);
    for index in 0..cases {
        let character_count = random.length(MAX_GENERATED_BYTES);
        let text: String = (0..character_count)
            .map(|_| VALID_CHARACTERS[random.index(VALID_CHARACTERS.len())])
            .collect();
        assert_frontend_does_not_panic(&format!("utf8-{index}"), &text);
    }
}

fn assert_frontend_does_not_panic(name: &str, text: &str) {
    if let Err(payload) = panic::catch_unwind(|| run_frontend_case(name, text)) {
        panic!("frontend panicked for {name}: {}", panic_message(payload));
    }
}

fn run_frontend_case(name: &str, text: &str) -> Vec<&'static str> {
    assert!(text.len() <= MAX_GENERATED_BYTES * 4 || name.starts_with("excessive-"));

    let mut sources = SourceDatabase::new();
    let source_id = sources.add(format!("{name}.ska"), text);
    let source = sources.get(source_id).expect("inserted source must exist");
    let lexed = lex(source);
    let parsed = parse(source, &lexed.tokens);

    // Rendering is part of the property: every span produced during recovery
    // must remain safe to locate and display against its owning source.
    let _lexed_rendering = render_diagnostics(&sources, &lexed.diagnostics);
    let _parsed_rendering = render_diagnostics(&sources, &parsed.diagnostics);

    lexed
        .diagnostics
        .iter()
        .chain(parsed.diagnostics.iter())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn generated_case_count() -> usize {
    std::env::var("SKALD_ROBUSTNESS_CASES")
        .map(|value| {
            let cases = value
                .parse()
                .expect("SKALD_ROBUSTNESS_CASES must be a positive integer");
            assert!(cases > 0, "SKALD_ROBUSTNESS_CASES must be positive");
            cases
        })
        .unwrap_or(DEFAULT_GENERATED_CASES)
}

fn decode_hex(encoded: &str) -> Vec<u8> {
    let digits: String = encoded
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert_eq!(digits.len() % 2, 0, "hex corpus must contain whole bytes");
    digits
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("hex corpus must be ASCII");
            u8::from_str_radix(pair, 16).expect("hex corpus must contain only hex digits")
        })
        .collect()
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

struct DeterministicRandom(u64);

impl DeterministicRandom {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn length(&mut self, maximum: usize) -> usize {
        self.index(maximum + 1)
    }

    fn index(&mut self, length: usize) -> usize {
        (self.next() % length as u64) as usize
    }

    fn bytes(&mut self, maximum: usize) -> Vec<u8> {
        let length = self.length(maximum);
        (0..length).map(|_| self.next() as u8).collect()
    }
}
