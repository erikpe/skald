//! Compile-time coverage for the intentional repository-internal API paths.

use std::ffi::OsString;

use skald_compiler::{
    backend::{emit_assembly, target_by_name, Target},
    diagnostics::{render_diagnostics, Diagnostics},
    driver::{compile_source_to_assembly, run_cli, Toolchain},
    hir::{dump_hir, HirObjectSlice, HirObjectView, HirProgram, HirViewTarget, ObjectProjection},
    identity::CallableId,
    lexer::{dump_tokens, lex, LexOutput},
    literal::NumericLiteralKind,
    mir::{
        dump_mir, lower_hir, verify_mir, MirBaseCopy, MirDirectBase, MirObjectView,
        MirPlaceProjection, MirProgram, MirViewTarget,
    },
    passes::run_mir_pipeline,
    resolve::{
        dump_resolved, resolve, ResolveOutput, ResolvedClassHierarchy, ResolvedClassMember,
        ResolvedProgram,
    },
    source::SourceDatabase,
    syntax::{dump_ast, parse, CompilationUnit, ParseOutput},
    typeck::{type_check, TypeCheckOutput},
};

#[test]
fn intentional_phase_and_dump_paths_compose() {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add(
        "api.ska",
        "class Empty { init() {} } fn main() -> i64 { return 0; }",
    );
    let source = sources.get(source_id).unwrap();

    let lexed: LexOutput = lex(source);
    let _tokens = dump_tokens(source, &lexed.tokens);
    let parsed: ParseOutput = parse(source, &lexed.tokens);
    let ast: &CompilationUnit = &parsed.ast;
    let _ast_dump = dump_ast(ast);
    let resolved: ResolveOutput = resolve(ast);
    let resolved_program: &ResolvedProgram = &resolved.program;
    let hierarchy: &ResolvedClassHierarchy = &resolved_program.hierarchy;
    let class = resolved_program.classes.iter().next().unwrap().id;
    let _base_chain = hierarchy.base_chain(class);
    let _member: Option<ResolvedClassMember> = hierarchy.member(class, "member");
    let _resolved_dump = dump_resolved(resolved_program);
    let checked: TypeCheckOutput = type_check(resolved_program);
    let hir: &HirProgram = checked.hir.as_ref().unwrap();
    let _hir_dump = dump_hir(hir);
    let _base_projection: Option<ObjectProjection> = None;
    let _object_slice: Option<HirObjectSlice> = None;
    let _object_view: Option<HirObjectView> = None;
    let _view_target: Option<HirViewTarget> = None;
    let mir: MirProgram = lower_hir(hir).unwrap();
    let _mir_base: Option<MirDirectBase> = None;
    let _mir_base_copy: Option<MirBaseCopy<skald_compiler::identity::InitializerId>> = None;
    let _mir_projection: Option<MirPlaceProjection> = None;
    let _mir_view: Option<MirObjectView> = None;
    let _mir_view_target: Option<MirViewTarget> = None;
    verify_mir(&mir).unwrap();
    let mir = run_mir_pipeline(mir).unwrap();
    let _mir_dump = dump_mir(&mir);
    let target = target_by_name("x86_64-sysv").unwrap();
    let _assembly = emit_assembly(target, &mir).unwrap();
    let diagnostics: &Diagnostics = &checked.diagnostics;
    let _diagnostics = render_diagnostics(&sources, diagnostics);
    let _identity_path: Option<CallableId> = None;
    let _literal_path: Option<NumericLiteralKind> = None;
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
