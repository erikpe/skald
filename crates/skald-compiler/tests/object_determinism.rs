//! Cross-process determinism coverage for the complete object lifetime pipeline.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use skald_compiler::{
    backend::{emit_assembly, Target},
    hir::dump_hir,
    lexer::lex,
    mir::{dump_mir, lower_hir},
    passes::run_mir_pipeline,
    resolve::{dump_resolved, resolve},
    source::SourceDatabase,
    syntax::{dump_ast, parse},
    typeck::type_check,
};

const HELPER_OUTPUT: &str = "SKALD_OBJECT_DETERMINISM_OUTPUT";
const TEST_NAME: &str = "object_lifetime_phase_products_are_deterministic_across_processes";

#[test]
fn object_lifetime_phase_products_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(HELPER_OUTPUT) {
        fs::write(output, complete_phase_dump()).unwrap();
        return;
    }

    let artifacts = TemporaryArtifacts::new();
    run_helper_process(&artifacts.first);
    run_helper_process(&artifacts.second);

    assert_eq!(
        fs::read(&artifacts.first).unwrap(),
        fs::read(&artifacts.second).unwrap(),
        "object phase products changed across independent compiler processes"
    );
}

fn run_helper_process(output: &Path) {
    let result = Command::new(env::current_exe().unwrap())
        .args(["--exact", TEST_NAME, "--nocapture"])
        .env(HELPER_OUTPUT, output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "determinism helper failed:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn complete_phase_dump() -> String {
    let text = concat!(
        "class Box { value: i64; init(value: i64) { self.value = value; } ",
        "init(ref other: Box) { self.value = other.value; } ",
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
    let mir = run_mir_pipeline(lower_hir(&hir).unwrap()).unwrap();
    let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();

    format!(
        "AST\n{}RESOLVED\n{}HIR\n{}MIR\n{}ASSEMBLY\n{}",
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
    fn new() -> Self {
        let stem = format!("skald-object-alias-determinism-{}", std::process::id());
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
