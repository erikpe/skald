use skald_compiler::{
    backend::{emit_assembly, BackendInput, Target},
    diagnostics::render_diagnostics,
    driver::EntrySelector,
    hir::dump_hir,
    lexer::{dump_tokens, lex},
    mir::{dump_mir, dump_preliminary_mir, lower_preliminary_hir, verify_preliminary_mir},
    module::{
        dump_module_graph, load_module_graph, normalize_provider_roots, ProviderRootConfiguration,
    },
    passes::{
        run_mir_pipeline,
        static_lifecycle::{
            dump_planned_mir, plan_static_lifetimes, synthesize_static_lifecycle,
            verify_planned_mir,
        },
    },
    resolve::{dump_resolved, resolve_module_graph},
    source::SourceDatabase,
    syntax::{dump_ast, parse, INVALID_RANGE_SYNTAX},
    typeck::type_check,
};

use crate::standard_library::canonical_standard_library_sources;

use super::{
    fixture::{write_source, ModuleFixture},
    normalization::normalize_fixture_paths,
};

pub(crate) fn range_module_phase_dump(variant: usize) -> String {
    const APP_SOURCE: &str = "import model;\n\
         from std::range import Range;\n\
         fn main() -> i64 {\n\
           var primitive: model::Advance<u64> = model::Advance<u64>();\n\
           var objects: model::Advance<model::Value> = model::Advance<model::Value>();\n\
           var total: i64 = (i64) primitive.next(16u) + (i64) objects.next(model::Value(24u)).get();\n\
           for (value in Range<u64>(2u, 5u)) { total = total + (i64) value; }\n\
           for (value in Range<model::Value>(model::Value(6u), model::Value(8u))) { total = total + (i64) value.get(); }\n\
           for (value in 8u .. 10u) { total = total + (i64) value; }\n\
           for (value in model::Value(10u) .. model::Value(12u)) { total = total + (i64) value.get(); }\n\
           return total;\n\
         }\n";
    const MODEL_SOURCE: &str = "from std::ops import OpLess;\n\
         from std::range import Successor;\n\
         public class Value implements OpLess<Value>, Successor<Value> {\n\
           private value: u64;\n\
           init(value: u64) { self.value = value; }\n\
           fn op_less(ref rhs: Value) -> bool { return self.value < rhs.value; }\n\
           fn successor() -> Value { return Value(self.value + 1u); }\n\
           fn get() -> u64 { return self.value; }\n\
         }\n\
         public class Advance<T> where T: Successor<T> {\n\
           init() {}\n\
           fn next(value: T) -> T { return value.successor(); }\n\
         }\n";

    let fixture = ModuleFixture::new("range-products", variant);
    let application = fixture.path().join("application");
    let standard_library = fixture.path().join("standard-library");
    let mut sources = vec![
        (application.join("app.ska"), APP_SOURCE),
        (application.join("model.ska"), MODEL_SOURCE),
    ];
    sources.extend(
        canonical_standard_library_sources(&[])
            .into_iter()
            .map(|(relative, source)| (standard_library.join(relative), source)),
    );
    if variant != 0 {
        sources.reverse();
    }
    for (path, source) in sources {
        write_source(&path, source);
    }

    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::standard_library(standard_library),
            ProviderRootConfiguration::module_root(application),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(application),
            ProviderRootConfiguration::standard_library(standard_library),
        ]
    };
    let providers = normalize_provider_roots(fixture.path(), &configurations).unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        fixture.path(),
        &providers,
    )
    .unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let resolved_dump = dump_resolved(&resolved.program);
    assert!(
        resolved_dump.contains("RangeSource template"),
        "{resolved_dump}"
    );
    assert!(resolved_dump.contains("AddOneU64"), "{resolved_dump}");
    assert!(
        resolved_dump.contains("ClosedBoundSelection 0 class-witness"),
        "{resolved_dump}"
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let hir_dump = dump_hir(&hir);
    assert!(hir_dump.contains("RangeLoopEvidence"));
    assert!(hir_dump.contains("PrimitiveRange endpoint=u64"));
    assert!(hir_dump.contains("Protocol interface="));
    let preliminary = lower_preliminary_hir(&hir);
    let preliminary_dump = dump_preliminary_mir(&preliminary);
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
    assert!(!assembly.contains("skald_rt_range"), "{assembly}");

    normalize_fixture_paths(
        fixture.path(),
        format!(
            "{}GRAPH\n{}RESOLVED\n{}HIR\n{}PRELIMINARY MIR\n{}PLANNED MIR\n{}FINAL MIR\n{}ASSEMBLY\n{}",
            range_frontend_phase_dump(APP_SOURCE, MODEL_SOURCE),
            dump_module_graph(&graph),
            resolved_dump,
            hir_dump,
            preliminary_dump,
            planned_dump,
            dump_mir(&final_mir),
            assembly,
        ),
    )
}

fn range_frontend_phase_dump(app_source: &str, model_source: &str) -> String {
    let mut sources = SourceDatabase::new();
    let source_ids = [
        sources.add("app.ska", app_source),
        sources.add("model.ska", model_source),
    ];
    let mut output = String::new();
    for (label, source_id) in ["APP", "MODEL"].into_iter().zip(source_ids) {
        let source = sources.get(source_id).unwrap();
        let lexed = lex(source);
        assert!(lexed.diagnostics.is_empty());
        let parsed = parse(source, &lexed.tokens);
        assert!(parsed.diagnostics.is_empty());
        output.push_str(&format!(
            "{label} TOKENS\n{}{label} AST\n{}",
            dump_tokens(source, &lexed.tokens),
            dump_ast(&parsed.ast),
        ));
    }
    output
}

pub(crate) fn range_syntax_diagnostic_dump() -> String {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add(
        "range-direct-source-only.ska",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/ranges/direct_source_only.ska"
        )),
    );
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert_eq!(parsed.diagnostics.len(), 4, "{:?}", parsed.diagnostics);
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == INVALID_RANGE_SYNTAX),
        "{:?}",
        parsed.diagnostics
    );

    format!(
        "TOKENS\n{}AST\n{}DIAGNOSTICS\n{}",
        dump_tokens(source, &lexed.tokens),
        dump_ast(&parsed.ast),
        render_diagnostics(&sources, &parsed.diagnostics),
    )
}
