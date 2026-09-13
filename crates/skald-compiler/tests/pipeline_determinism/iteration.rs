use skald_compiler::{
    backend::{emit_assembly, BackendInput, Target},
    diagnostics::render_diagnostics,
    driver::EntrySelector,
    hir::dump_hir,
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
    typeck::type_check,
};

use crate::standard_library::canonical_standard_library_sources;

use super::{
    fixture::{write_source, ModuleFixture},
    normalization::normalize_module_fixture_output,
};

pub(crate) fn iteration_module_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("general-iteration-products", variant);
    let application = fixture.path().join("application");
    let standard_library = fixture.path().join("standard-library");
    let mut sources = vec![
        (
            application.join("app.ska"),
            "import model;\n\
             fn concrete(ref values: model::Values) -> i64 {\n\
               var sum: i64 = 0;\n\
               for (item in values) { sum = sum + item; }\n\
               return sum;\n\
             }\n\
             fn generic(ref scanner: model::Scanner<model::Values>, ref values: model::Values) -> i64 {\n\
               return scanner.scan(values);\n\
             }\n\
             fn main() -> i64 {\n\
               var values: model::Values = model::Values();\n\
               var scanner: model::Scanner<model::Values> = model::Scanner<model::Values>();\n\
               return concrete(values) + generic(scanner, values);\n\
             }\n",
        ),
        (
            application.join("model.ska"),
            "from std::iter import Iterable;\n\
             public class Values implements Iterable<i64, u64> {\n\
               init() {}\n\
               fn iter_state() -> u64 { return 0u; }\n\
               fn iter_next(mut ref state: u64) -> i64? { return none; }\n\
             }\n\
             public class Scanner<Source> where Source: Iterable<i64, u64> {\n\
               init() {}\n\
               fn scan(ref values: Source) -> i64 {\n\
                 var sum: i64 = 0;\n\
                 for (item in values) { sum = sum + item; }\n\
                 return sum;\n\
               }\n\
             }\n",
        ),
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
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
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

    normalize_module_fixture_output(
        fixture.path(),
        format!(
            "GRAPH\n{}RESOLVED\n{}HIR\n{}PRELIMINARY MIR\n{}PLANNED MIR\n{}FINAL MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
            preliminary_dump,
            planned_dump,
            dump_mir(&final_mir),
            assembly,
        ),
    )
}

pub(crate) fn iteration_diagnostic_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("general-iteration-diagnostics", variant);
    let application = fixture.path().join("application");
    let standard_library = fixture.path().join("standard-library");
    let claims = if variant == 0 {
        "Iterable<i64, u64>, Iterable<f64, i64>"
    } else {
        "Iterable<f64, i64>, Iterable<i64, u64>"
    };
    let app = format!(
        "from std::iter import Iterable;\n\
         class Both implements {claims} {{ init() {{}} }}\n\
         fn scan(ref values: Both) -> unit {{ for (item in values) {{}} }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    );
    write_source(&application.join("app.ska"), &app);
    for (relative, source) in canonical_standard_library_sources(&[]) {
        write_source(&standard_library.join(relative), source);
    }
    let providers = normalize_provider_roots(
        fixture.path(),
        &[
            ProviderRootConfiguration::module_root(application),
            ProviderRootConfiguration::standard_library(standard_library),
        ],
    )
    .unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        fixture.path(),
        &providers,
    )
    .unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.has_errors());
    let rendered = render_diagnostics(graph.sources(), &resolved.diagnostics).replace(
        &format!("class Both implements {claims} {{ init() {{}} }}"),
        "class Both implements <first-claim>, <second-claim> { init() {} }",
    );
    normalize_module_fixture_output(fixture.path(), rendered)
}
