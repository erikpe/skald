//! Explicit single-source compiler pipelines shared by determinism generators.

use std::borrow::Cow;

use skald_compiler::{
    backend::{emit_assembly, BackendInput, Target},
    diagnostics::render_diagnostics,
    hir::{dump_hir, HirProgram},
    lexer::{dump_tokens, lex},
    mir::{dump_mir, lower_preliminary_hir, verify_preliminary_mir},
    passes::{
        run_mir_pipeline,
        static_lifecycle::{
            dump_planned_mir, plan_static_lifetimes, synthesize_static_lifecycle,
            verify_planned_mir,
        },
        VerifiedFinalMirProgram,
    },
    resolve::{dump_resolved, resolve},
    source::SourceDatabase,
    syntax::{dump_ast, parse},
    typeck::type_check,
};

#[derive(Clone, Copy)]
pub(crate) enum StandardLibraryInput {
    None,
    GoldenCallsAsExternalStubs,
}

pub(crate) fn single_source_full_phase_dump(
    text: &str,
    standard_library: StandardLibraryInput,
) -> String {
    let text = prepare_source(text, standard_library);
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("determinism.ska", text.into_owned());
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
    let mir = lower_final_hir(&hir);
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&mir),
    )
    .unwrap();

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

pub(crate) fn single_source_type_error_dump(
    name: &str,
    text: &str,
    standard_library: StandardLibraryInput,
) -> String {
    let text = prepare_source(text, standard_library);
    let mut sources = SourceDatabase::new();
    let source_id = sources.add(name, text.into_owned());
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.hir.is_none());

    format!(
        "AST\n{}RESOLVED\n{}DIAGNOSTICS\n{}",
        dump_ast(&parsed.ast),
        dump_resolved(&resolved.program),
        render_diagnostics(&sources, &checked.diagnostics),
    )
}

pub(crate) fn single_source_planned_lifecycle_dump(
    text: &str,
    standard_library: StandardLibraryInput,
) -> String {
    let text = prepare_source(text, standard_library);
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("typed-hir-determinism.ska", text.into_owned());
    let source = sources.get(source_id).unwrap();

    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let preliminary = lower_preliminary_hir(&hir);
    let preliminary = verify_preliminary_mir(preliminary).unwrap();
    let planned = plan_static_lifetimes(preliminary).unwrap();
    let planned_dump = dump_planned_mir(&planned);
    let final_mir = run_mir_pipeline(synthesize_static_lifecycle(
        verify_planned_mir(planned).unwrap(),
    ))
    .unwrap();
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&final_mir),
    )
    .unwrap();

    format!(
        "TOKENS\n{}AST\n{}RESOLVED\n{}HIR\n{}PLANNED MIR\n{}FINAL MIR\n{}ASSEMBLY\n{}",
        dump_tokens(source, &lexed.tokens),
        dump_ast(&parsed.ast),
        dump_resolved(&resolved.program),
        dump_hir(&hir),
        planned_dump,
        dump_mir(&final_mir),
        assembly,
    )
}

pub(crate) fn lower_final_hir(hir: &HirProgram) -> VerifiedFinalMirProgram {
    let preliminary = lower_preliminary_hir(hir);
    let preliminary = verify_preliminary_mir(preliminary).unwrap();
    let planned = plan_static_lifetimes(preliminary).unwrap();
    let mir = synthesize_static_lifecycle(verify_planned_mir(planned).unwrap());
    run_mir_pipeline(mir).unwrap()
}

fn prepare_source(text: &str, standard_library: StandardLibraryInput) -> Cow<'_, str> {
    match standard_library {
        StandardLibraryInput::None => Cow::Borrowed(text),
        StandardLibraryInput::GoldenCallsAsExternalStubs => {
            Cow::Owned(replace_golden_standard_library_calls(text))
        }
    }
}

fn replace_golden_standard_library_calls(text: &str) -> String {
    let mut source = text
        .lines()
        .filter(|line| line != &"import std::io;")
        .collect::<Vec<_>>()
        .join("\n");
    let mut declarations = String::new();
    for (public_name, recorder_name, parameter_type) in [
        ("std::io::println_bool", "test_record_bool", "bool"),
        ("std::io::println_i64", "test_record_i64", "i64"),
        ("std::io::println_u64", "test_record_u64", "u64"),
        ("std::io::println_u8", "test_record_u8", "u8"),
        ("std::io::println_f64", "test_record_f64", "f64"),
    ] {
        if source.contains(public_name) {
            declarations.push_str(&format!(
                "extern fn {recorder_name}(value: {parameter_type}) -> unit;\n"
            ));
            source = source.replace(public_name, recorder_name);
        }
    }
    replace_standard_test_assertions(&mut source, &mut declarations);
    declarations.push_str(&source);
    declarations
}

fn replace_standard_test_assertions(source: &mut String, declarations: &mut String) {
    let imports_assertions = source
        .lines()
        .any(|line| line == "import std::test;" || line.starts_with("from std::test import "));
    if !imports_assertions {
        return;
    }

    *source = source
        .lines()
        .filter(|line| line != &"import std::test;" && !line.starts_with("from std::test import "))
        .collect::<Vec<_>>()
        .join("\n");

    for (name, parameters) in [
        ("assert_eq_f64", "left: f64, right: f64"),
        ("assert_eq_i64", "left: i64, right: i64"),
        ("assert_eq_u64", "left: u64, right: u64"),
        ("assert_eq_u8", "left: u8, right: u8"),
        ("assert_false", "value: bool"),
        ("assert_true", "value: bool"),
    ] {
        *source = source.replace(&format!("std::test::{name}"), name);
        if source.contains(&format!("{name}(")) {
            declarations.push_str(&format!("extern fn {name}({parameters}) -> unit;\n"));
        }
    }
}
