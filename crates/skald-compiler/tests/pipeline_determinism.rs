//! Cross-process determinism coverage for representative complete pipelines.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use skald_compiler::{
    backend::{emit_assembly, Target},
    diagnostics::render_diagnostics,
    hir::dump_hir,
    lexer::{dump_tokens, lex},
    mir::{dump_mir, lower_hir},
    passes::run_mir_pipeline,
    resolve::{dump_resolved, resolve},
    source::SourceDatabase,
    syntax::{dump_ast, parse},
    typeck::type_check,
};

const OBJECT_HELPER_OUTPUT: &str = "SKALD_OBJECT_DETERMINISM_OUTPUT";
const OBJECT_TEST_NAME: &str = "object_lifetime_phase_products_are_deterministic_across_processes";
const POLYMORPHISM_HELPER_OUTPUT: &str = "SKALD_POLYMORPHISM_DETERMINISM_OUTPUT";
const POLYMORPHISM_TEST_NAME: &str =
    "polymorphism_phase_products_are_deterministic_across_processes";
const SHARED_HELPER_OUTPUT: &str = "SKALD_SHARED_DETERMINISM_OUTPUT";
const SHARED_TEST_NAME: &str = "shared_ownership_phase_products_are_deterministic_across_processes";
const OPTIONAL_HELPER_OUTPUT: &str = "SKALD_OPTIONAL_DETERMINISM_OUTPUT";
const OPTIONAL_TEST_NAME: &str = "optional_value_phase_products_are_deterministic_across_processes";
const ARRAY_HELPER_OUTPUT: &str = "SKALD_ARRAY_DETERMINISM_OUTPUT";
const ARRAY_TEST_NAME: &str = "array_resolution_products_are_deterministic_across_processes";

#[test]
fn object_lifetime_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "object",
        OBJECT_HELPER_OUTPUT,
        OBJECT_TEST_NAME,
        object_phase_dump,
    );
}

#[test]
fn polymorphism_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "polymorphism",
        POLYMORPHISM_HELPER_OUTPUT,
        POLYMORPHISM_TEST_NAME,
        polymorphism_phase_dump,
    );
}

#[test]
fn shared_ownership_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "shared-ownership",
        SHARED_HELPER_OUTPUT,
        SHARED_TEST_NAME,
        shared_ownership_phase_dump,
    );
}

#[test]
fn optional_value_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "optional-values",
        OPTIONAL_HELPER_OUTPUT,
        OPTIONAL_TEST_NAME,
        optional_phase_dump,
    );
}

#[test]
fn array_resolution_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "arrays",
        ARRAY_HELPER_OUTPUT,
        ARRAY_TEST_NAME,
        array_resolution_dump,
    );
}

fn assert_cross_process_determinism(
    label: &str,
    helper_output: &str,
    test_name: &str,
    generate: fn() -> String,
) {
    if let Some(output) = env::var_os(helper_output) {
        fs::write(output, generate()).unwrap();
        return;
    }

    let artifacts = TemporaryArtifacts::new(label);
    run_helper_process(&artifacts.first, helper_output, test_name);
    run_helper_process(&artifacts.second, helper_output, test_name);

    assert_eq!(
        fs::read(&artifacts.first).unwrap(),
        fs::read(&artifacts.second).unwrap(),
        "{label} phase products changed across independent compiler processes"
    );
}

fn run_helper_process(output: &Path, helper_output: &str, test_name: &str) {
    let result = Command::new(env::current_exe().unwrap())
        .args(["--exact", test_name, "--nocapture"])
        .env(helper_output, output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "determinism helper failed:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn object_phase_dump() -> String {
    let text = concat!(
        "class Box { value: i64; init(value: i64) { self.value = value; } ",
        "copy(ref other: Box) { self.value = other.value; } ",
        "assign(ref other: Box) { self.value = other.value; } ",
        "mut fn set(value: i64) -> unit { self.value = value; } ",
        "fn get() -> i64 { return self.value; } destroy {} }\n",
        "class Snapshot { box: Box; init(ref source: Box) { self.box = Box(read(source)); } ",
        "destroy {} }\n",
        "fn read(ref value: Box) -> i64 { return value.get(); }\n",
        "fn write(mut ref value: Box, amount: i64) -> unit { value.set(amount); }\n",
        "fn forward(mut ref value: Box) -> unit { write(value, read(value) + 1); }\n",
        "fn produce(value: i64) -> Box { return Box(value); }\n",
        "fn choose(ref source: Box, first: bool) -> Box { ",
        "if (first) { return source; } else { return (Box(source.get())); } }\n",
        "fn consume(value: Box, ref alias: Box) -> i64 { ",
        "value = produce(alias.get()); return value.get(); }\n",
        "fn main() -> i64 { var value: Box = Box(1); forward(value); ",
        "var grouped: Box = (Box(2)); grouped = produce(read(value)); ",
        "var copied: Box = value; var result: Box = choose(copied, false); ",
        "var snapshot: Snapshot = Snapshot(result); ",
        "return consume(produce(snapshot.box.get()), grouped); }\n",
    );
    complete_phase_dump(text)
}

fn polymorphism_phase_dump() -> String {
    complete_phase_dump(include_str!("../../../tests/golden/run/polymorphism.ska"))
}

fn shared_ownership_phase_dump() -> String {
    complete_phase_dump(include_str!(
        "../../../tests/golden/run/shared_copy_allocation.ska"
    ))
}

fn optional_phase_dump() -> String {
    complete_phase_dump(include_str!(
        "../../../tests/golden/run/optional_shared_profile.ska"
    ))
}

fn array_resolution_dump() -> String {
    let text = concat!(
        "class Item { init() {} }\n",
        "fn inspect(first: Item[][], second: Item[][], owner: shared Item[][], ",
        "elements: (shared? Item)[]) -> Item[] { return first[1:]; }\n",
        "fn main() -> i64 { var values: i64[] = i64[](4u); return values[-1]; }\n",
    );
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("determinism.ska", text);
    let source = sources.get(source_id).unwrap();

    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.has_errors());
    assert!(checked.hir.is_none());

    format!(
        "TOKENS\n{}AST\n{}RESOLVED\n{}TYPECHECK\n{}",
        dump_tokens(source, &lexed.tokens),
        dump_ast(&parsed.ast),
        dump_resolved(&resolved.program),
        render_diagnostics(&sources, &checked.diagnostics),
    )
}

fn complete_phase_dump(text: &str) -> String {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("determinism.ska", text);
    let source = sources.get(source_id).unwrap();

    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty());
    let hir = checked.hir.unwrap();
    let mir = run_mir_pipeline(lower_hir(&hir)).unwrap();
    let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();

    format!(
        "TOKENS\n{}AST\n{}RESOLVED\n{}HIR\n{}MIR\n{}ASSEMBLY\n{}",
        dump_tokens(source, &lexed.tokens),
        dump_ast(&parsed.ast),
        dump_resolved(&resolved.program),
        dump_hir(&hir),
        dump_mir(&mir),
        assembly,
    )
}

struct TemporaryArtifacts {
    first: PathBuf,
    second: PathBuf,
}

impl TemporaryArtifacts {
    fn new(label: &str) -> Self {
        let stem = format!("skald-{label}-determinism-{}", std::process::id());
        let directory = env::temp_dir();
        Self {
            first: directory.join(format!("{stem}-first.txt")),
            second: directory.join(format!("{stem}-second.txt")),
        }
    }
}

impl Drop for TemporaryArtifacts {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.first);
        let _ = fs::remove_file(&self.second);
    }
}
