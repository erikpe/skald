//! Deterministic generative coverage for hostile frontend input.
//!
//! The compiler source boundary accepts UTF-8 text. Arbitrary bytes are fed
//! through the same boundary after deterministic lossy UTF-8 decoding, while a
//! separate generator covers valid Unicode scalar sequences directly.

use std::{any::Any, panic};

use skald_compiler::{
    backend::Target,
    diagnostics::render_diagnostics,
    driver::compile_source_to_assembly,
    lexer::lex,
    resolve::resolve,
    source::SourceDatabase,
    syntax::{parse, EXCESSIVE_NESTING, MAX_SYNTAX_NESTING},
    typeck::type_check,
};

const DEFAULT_GENERATED_CASES: usize = 256;
const MAX_GENERATED_BYTES: usize = 1_024;
const BYTE_SEED: u64 = 0x6a09_e667_f3bc_c909;
const UTF8_SEED: u64 = 0xbb67_ae85_84ca_a73b;
const VALID_CHARACTERS: &[char] = &[
    '\0', '\n', '\r', '\t', ' ', '"', '\'', '\\', '(', ')', '{', '}', ':', ';', ',', '.', '+', '-',
    '*', '/', '=', '!', '0', '1', '8', '9', '?', 'a', 'c', 'd', 'e', 'f', 'i', 'l', 'n', 'r', 's',
    't', 'u', 'x', '_', 'é', 'λ', '中', '🦀',
];

#[test]
fn arbitrary_bytes_and_utf8_never_panic_in_the_frontend() {
    let cases = generated_case_count();
    exercise_generated_bytes(cases);
    exercise_generated_utf8(cases);
    exercise_class_header_mutations();
    exercise_optional_syntax_mutations();
    exercise_array_syntax_mutations();
    exercise_generic_syntax_mutations();
    exercise_operator_syntax_mutations();
    exercise_for_in_syntax_mutations();
    exercise_range_syntax_mutations();
    exercise_byte_literal_mutations();
}

fn exercise_range_syntax_mutations() {
    const SEEDS: &[&str] = &[
        "fn main() -> i64 { for (item in 0u .. 8u) { if (item == 4u) { continue; } } return 0; }",
        "fn main() -> i64 { for (item in (1u) .. (4u)) {} return 0; }",
        "fn main() -> i64 { for (outer in 0u8 .. 4u8) { for (inner in -2 .. 2) {} } return 0; }",
        "fn main() -> i64 { for (broken in (1u ..) .. (.. 3u)) {} return 0; }",
    ];
    const INSERTIONS: &[&str] = &["..", ".", "(", ")", "in", "for", "{", "}", ";", "u"];

    for (seed_index, seed) in SEEDS.iter().enumerate() {
        for index in 0..seed.len() {
            let mut deletion = (*seed).to_owned();
            deletion.remove(index);
            assert_deterministic_frontend_recovery(
                &format!("range-{seed_index}-delete-{index}"),
                &deletion,
            );
        }
        for index in 0..=seed.len() {
            let mut insertion = (*seed).to_owned();
            insertion.insert_str(index, INSERTIONS[index % INSERTIONS.len()]);
            assert_deterministic_frontend_recovery(
                &format!("range-{seed_index}-insert-{index}"),
                &insertion,
            );
        }
    }
}

fn exercise_operator_syntax_mutations() {
    const SEEDS: &[&str] = &[
        "fn main() -> i64 { return -(1 + 2 * 3) + (4 << 1); }",
        "fn compare(left: u64, right: u64) -> bool { return /* left */ left <= right && left != right; } fn main() -> i64 { return 0; }",
        "interface OpAdd<Rhs, Output> { fn op_add(ref rhs: Rhs) -> Output; } class Adder<T> where T: OpAdd<T, T> { fn add(ref left: T, ref right: T) -> T { return left + /* rhs */ right; } }",
        "class Box<T> { value: T; } fn nested(ref value: Box<Box<u64>>) -> u64 { return value.value.value + 1u; } fn main() -> i64 { return 0; }",
    ];
    const INSERTIONS: &[&str] = &[
        "+", "-", "*", "/", "%", "==", "!=", "<=", ">=", "<<", ">>", "~", "/*x*/", "(", ")", "<",
        ">", ",", ";",
    ];

    for (seed_index, seed) in SEEDS.iter().enumerate() {
        for index in 0..seed.len() {
            let mut deletion = (*seed).to_owned();
            deletion.remove(index);
            assert_deterministic_frontend_recovery(
                &format!("operator-{seed_index}-delete-{index}"),
                &deletion,
            );
        }
        for index in 0..=seed.len() {
            let mut insertion = (*seed).to_owned();
            insertion.insert_str(index, INSERTIONS[index % INSERTIONS.len()]);
            assert_deterministic_frontend_recovery(
                &format!("operator-{seed_index}-insert-{index}"),
                &insertion,
            );
        }
    }
}

fn exercise_for_in_syntax_mutations() {
    const SEEDS: &[&str] = &[
        "fn main(values: Source) -> unit { for (item in values) { while (false) { continue; } } }",
        "class Scanner<Source> where Source: Iterable<i64, u64> { fn scan(ref values: Source) -> unit { for (item: i64 in values) { for (nested in values) {} } } }",
    ];
    const INSERTIONS: &[&str] = &["for", "(", ")", ":", "in", "{", "}", ";"];

    for (seed_index, seed) in SEEDS.iter().enumerate() {
        for index in 0..seed.len() {
            let mut deletion = (*seed).to_owned();
            deletion.remove(index);
            assert_deterministic_frontend_recovery(
                &format!("for-in-{seed_index}-delete-{index}"),
                &deletion,
            );
        }
        for index in 0..=seed.len() {
            let mut insertion = (*seed).to_owned();
            insertion.insert_str(index, INSERTIONS[index % INSERTIONS.len()]);
            assert_deterministic_frontend_recovery(
                &format!("for-in-{seed_index}-insert-{index}"),
                &insertion,
            );
        }
    }
}

fn exercise_generic_syntax_mutations() {
    const SEEDS: &[&str] = &[
        "class Pair<Left, Right> { left: Left; right: Right; } fn main() -> i64 { return 0; }",
        "class Box<T> { value: T?[]; } fn use(ref value: Box<Box<i64?>[]>) -> unit {} fn main() -> i64 { return 0; }",
        "interface Comparable { fn compare(ref other: Obj) -> i64; } class Sorted<T> where T: Comparable { value: T; } fn main() -> i64 { return 0; }",
        "interface Producer<T> where T: Marker<Outer<T>> { fn produce() -> T; } class Use<T> implements Producer<T> {} fn main() -> i64 { return 0; }",
        "interface PairSource<Left, Right> { fn left() -> Left; fn right() -> Right; } class Pair<Left, Right> implements PairSource<Left, Right> {} fn main() -> i64 { return 0; }",
        "interface Transfer<Left, Right> where Left: Marker<Outer<Right>> { fn move(value: Left, ref fallback: Outer<Right>) -> Right; } fn use(ref value: Transfer<Item, u64>) -> unit {} fn main() -> i64 { return 0; }",
        "interface Value<T> { fn value() -> T; } class Item implements Value<i64> { init() {} fn value() -> i64 { return 1; } } fn main() -> i64 { var item: Item = Item(); var exact: bool = item is Value<i64>; return ((Value<i64>) item).value(); }",
        "interface Sequence<T> { fn index_get(key: i64) -> T; } fn read(ref values: Sequence<Outer<i64[]?>[]>) -> unit {} fn main() -> i64 { return 0; }",
        "interface Marker<T> {} class Pair<Left, Right> implements Marker<Outer<Left>>, Marker<Outer<Right>> where Left: Marker<Right> { init() {} } fn main() -> i64 { return 0; }",
        "class Broken<T where T Comparable, T: { value: Box<T; } fn recovered() -> i64 { return 0; }",
        "interface Broken<T U where T: Marker<Outer<T>>, U Marker { fn read(value: T -> U; } fn recovered() -> i64 { return 0; }",
    ];

    for (seed_index, seed) in SEEDS.iter().enumerate() {
        for index in 0..seed.len() {
            let mut deletion = (*seed).to_owned();
            deletion.remove(index);
            assert_deterministic_frontend_recovery(
                &format!("generic-{seed_index}-delete-{index}"),
                &deletion,
            );
        }
        for index in 0..=seed.len() {
            let mut insertion = (*seed).to_owned();
            let fragments = [
                "<",
                ">",
                ",",
                ":",
                "(",
                ")",
                "[",
                "]",
                "?",
                " where ",
                " implements ",
            ];
            insertion.insert_str(index, fragments[index % fragments.len()]);
            assert_deterministic_frontend_recovery(
                &format!("generic-{seed_index}-insert-{index}"),
                &insertion,
            );
        }
    }
}

fn assert_deterministic_frontend_recovery(name: &str, text: &str) {
    let first = panic::catch_unwind(|| run_frontend_case(name, text)).unwrap_or_else(|payload| {
        panic!("frontend panicked for {name}: {}", panic_message(payload))
    });
    let second = run_frontend_case(name, text);
    assert_eq!(first, second, "frontend recovery changed for {name}");
}

#[test]
fn bounded_source_loops_compile_deterministically_without_pipeline_panics() {
    let cases = generated_case_count();
    let mut random = DeterministicRandom::new(0x3c6e_f372_fe94_f82b);

    for index in 0..cases {
        let limit = 1 + random.index(8);
        let continue_at = 1 + random.index(limit);
        let break_at = continue_at + 1 + random.index(3);
        let nesting = 1 + random.index(3);
        let source = generated_loop_source(limit, continue_at, break_at, nesting);
        let name = format!("generated-loop-{index}.ska");

        let first =
            panic::catch_unwind(|| compile_source_to_assembly(&name, &source, Target::X86_64SysV))
                .unwrap_or_else(|payload| {
                    panic!(
                        "loop pipeline panicked for case {index}: {}",
                        panic_message(payload)
                    )
                })
                .unwrap_or_else(|error| {
                    panic!("generated loop case {index} was rejected: {error:?}")
                });
        let second = compile_source_to_assembly(&name, &source, Target::X86_64SysV)
            .unwrap_or_else(|error| panic!("repeated loop case {index} was rejected: {error:?}"));
        assert_eq!(
            first.assembly, second.assembly,
            "generated loop case {index} was nondeterministic"
        );
    }
}

#[test]
fn bounded_deep_operator_and_generic_sources_compile_deterministically() {
    for depth in 1..=16 {
        let mut nested_type = "u64".to_owned();
        let mut nested_value = "1u".to_owned();
        for _ in 0..depth {
            nested_value = format!("Box<{nested_type}>({nested_value})");
            nested_type = format!("Box<{nested_type}>");
        }
        let chain = (1..=depth)
            .map(|value| format!(" + {value}u"))
            .collect::<String>();
        let source = format!(
            "class Box<T> {{ value: T; init(value: T) {{ self.value = value; }} }} \
             fn observe(ref value: u64) -> u64 {{ return value; }} \
             fn main() -> i64 {{ var nested: {nested_type} = {nested_value}; \
             return (i64) observe(1u{chain}); }}"
        );
        let name = format!("generated-operator-generic-{depth}.ska");
        let first = compile_source_to_assembly(&name, &source, Target::X86_64SysV)
            .unwrap_or_else(|error| panic!("generated operator case {depth} failed: {error:?}"));
        let second = compile_source_to_assembly(&name, &source, Target::X86_64SysV)
            .unwrap_or_else(|error| panic!("repeated operator case {depth} failed: {error:?}"));
        assert_eq!(
            first.assembly, second.assembly,
            "generated operator case {depth} was nondeterministic"
        );
    }
}

fn generated_loop_source(
    limit: usize,
    continue_at: usize,
    break_at: usize,
    nesting: usize,
) -> String {
    let mut source = format!(
        "fn main() -> i64 {{ var value: i64 = 0; while (value < {limit}) {{ \
         value = value + 1; if (value == {continue_at}) {{ continue; }} \
         if (value == {break_at}) {{ break; }} "
    );
    for _ in 0..nesting {
        source.push_str("{ ");
    }
    source.push_str("var observed: i64 = value; value = observed;");
    for _ in 0..nesting {
        source.push_str(" }");
    }
    source.push_str(" } return value; }");
    source
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

fn exercise_optional_syntax_mutations() {
    const SEED: &str =
        "fn main() -> i64 { var value: i64? = none; if (value is some) { return value!; } return 0; }";

    for index in 0..SEED.len() {
        let mut deletion = SEED.to_owned();
        deletion.remove(index);
        assert_frontend_does_not_panic(&format!("optional-delete-{index}"), &deletion);
    }
    for index in 0..=SEED.len() {
        let mut insertion = SEED.to_owned();
        insertion.insert(index, if index % 2 == 0 { '?' } else { '!' });
        assert_frontend_does_not_panic(&format!("optional-insert-{index}"), &insertion);
    }

    const BOX_SEED: &str =
        "fn main() -> i64 { var box: shared (i64[]?)? = new (i64[]?)?(none); return 0; }";
    for index in 0..BOX_SEED.len() {
        let mut deletion = BOX_SEED.to_owned();
        deletion.remove(index);
        assert_frontend_does_not_panic(&format!("optional-box-delete-{index}"), &deletion);
    }
    for index in 0..=BOX_SEED.len() {
        let mut insertion = BOX_SEED.to_owned();
        insertion.insert(index, [',', '?', '(', ')'][index % 4]);
        assert_frontend_does_not_panic(&format!("optional-box-insert-{index}"), &insertion);
    }
}

fn exercise_array_syntax_mutations() {
    const SEED: &str =
        "fn main() -> i64 { var values: (shared? i64[])[] = (shared? i64[])[](2u); return values[0:1].len(); }";

    for index in 0..SEED.len() {
        let mut deletion = SEED.to_owned();
        deletion.remove(index);
        assert_frontend_does_not_panic(&format!("array-delete-{index}"), &deletion);
    }
    for index in 0..=SEED.len() {
        let mut insertion = SEED.to_owned();
        insertion.insert(index, if index % 2 == 0 { '[' } else { ']' });
        assert_frontend_does_not_panic(&format!("array-insert-{index}"), &insertion);
    }
}

fn exercise_byte_literal_mutations() {
    const SEED: &str = "fn main() -> i64 { var value: u8 = '\\xAf'; return (i64) value; }";

    for index in 0..SEED.len() {
        let mut deletion = SEED.to_owned();
        deletion.remove(index);
        assert_frontend_does_not_panic(&format!("byte-literal-delete-{index}"), &deletion);
    }
    for index in 0..=SEED.len() {
        let mut insertion = SEED.to_owned();
        insertion.insert(index, if index % 2 == 0 { '\'' } else { '\\' });
        assert_frontend_does_not_panic(&format!("byte-literal-insert-{index}"), &insertion);
    }

    for (index, malformed) in [
        "''",
        "'ab'",
        "'\\q'",
        "'\\x'",
        "'\\x4'",
        "'\\xgg'",
        "'é'",
        "'\t'",
        "'bad\nnext",
        "'unterminated",
    ]
    .into_iter()
    .enumerate()
    {
        assert_frontend_does_not_panic(&format!("malformed-byte-literal-{index}"), malformed);
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
    let excessive_array_type = format!(
        "fn main() -> i64 {{ var values: i64{} = i64[](); return 0; }}",
        "[]".repeat(MAX_SYNTAX_NESTING + 32),
    );

    for (name, text) in [
        ("excessive-groups", excessive_groups),
        ("excessive-blocks", excessive_blocks),
        ("excessive-array-type", excessive_array_type),
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

    let mut codes = lexed
        .diagnostics
        .iter()
        .chain(parsed.diagnostics.iter())
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    if lexed.diagnostics.is_empty() && parsed.diagnostics.is_empty() {
        let resolved = resolve(&parsed.ast);
        let _resolved_rendering = render_diagnostics(&sources, &resolved.diagnostics);
        codes.extend(
            resolved
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code),
        );
        if resolved.diagnostics.is_empty() {
            let checked = type_check(&resolved.program);
            let _type_rendering = render_diagnostics(&sources, &checked.diagnostics);
            codes.extend(checked.diagnostics.iter().map(|diagnostic| diagnostic.code));
        }
    }
    codes
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
