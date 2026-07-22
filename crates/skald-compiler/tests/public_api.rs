//! Compile-time coverage for the intentional repository-internal API paths.

use std::ffi::OsString;

use skald_compiler::{
    backend::{emit_assembly, target_by_name, Target},
    diagnostics::render_diagnostics,
    driver::{compile_source_to_assembly, run_cli, Toolchain},
    hir::{dump_hir, HirProgram},
    lexer::{dump_tokens, lex},
    mir::{dump_mir, lower_hir, verify_mir, MirProgram},
    passes::run_mir_pipeline,
    resolve::{dump_resolved, resolve, ResolvedProgram},
    source::SourceDatabase,
    syntax::{dump_ast, parse, CompilationUnit},
    typeck::type_check,
};

#[test]
fn intentional_phase_and_dump_paths_compose() {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("api.ska", "fn main() -> i64 { return 0; }");
    let source = sources.get(source_id).unwrap();

    let lexed = lex(source);
    let _tokens = dump_tokens(source, &lexed.tokens);
    let parsed = parse(source, &lexed.tokens);
    let ast: &CompilationUnit = &parsed.ast;
    let _ast_dump = dump_ast(ast);
    let resolved = resolve(ast);
    let resolved_program: &ResolvedProgram = &resolved.program;
    let _resolved_dump = dump_resolved(resolved_program);
    let checked = type_check(resolved_program);
    let hir: &HirProgram = checked.hir.as_ref().unwrap();
    let _hir_dump = dump_hir(hir);
    let mir: MirProgram = lower_hir(hir);
    verify_mir(&mir).unwrap();
    let mir = run_mir_pipeline(mir).unwrap();
    let _mir_dump = dump_mir(&mir);
    let target = target_by_name("x86_64-sysv").unwrap();
    let _assembly = emit_assembly(target, &mir).unwrap();
    let _diagnostics = render_diagnostics(&sources, &checked.diagnostics);
}

#[test]
fn intentional_driver_paths_compile() {
    let _cli_entry: fn(Vec<OsString>) -> i32 = run_cli::<Vec<OsString>>;
    let _toolchain = Toolchain::new("cc", "runtime.a");
    let artifact = compile_source_to_assembly(
        "api.ska",
        "fn main() -> i64 { return 0; }",
        Target::X86_64SysV,
    )
    .unwrap();

    assert!(!artifact.assembly.is_empty());
}
