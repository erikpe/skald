use skald_compiler::{
    backend::{emit_assembly, BackendInput, Target},
    diagnostics::render_diagnostics,
    driver::EntrySelector,
    hir::dump_hir,
    mir::dump_mir,
    module::{
        dump_module_graph, load_module_graph, normalize_provider_roots, ProviderRootConfiguration,
    },
    resolve::{dump_resolved, resolve_module_graph},
    typeck::type_check,
};

use crate::standard_library::canonical_standard_library_sources;

use super::super::{
    fixture::{write_source, ModuleFixture},
    normalization::normalize_module_fixture_output,
    source::lower_final_hir,
};

pub(crate) fn string_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("string-products", variant);
    let application = fixture.path().join("application");
    let standard_library = fixture.path().join("standard-library");
    let mut sources = vec![(
        application.join("app.ska"),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/primitive_strings/string_values.ska"
        )),
    )];
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

    normalize_module_fixture_output(
        fixture.path(),
        format!(
            "GRAPH\n{}DIAGNOSTICS\n{}RESOLVED\n{}HIR\n{}MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            render_diagnostics(graph.sources(), &resolved.diagnostics),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
            dump_mir(&mir),
            assembly,
        ),
    )
}

pub(crate) fn string_diagnostic_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("string-diagnostics", variant);
    let application = fixture.path().join("application");
    let standard_library = fixture.path().join("standard-library");
    let malformed_string = concat!(
        "public class Str {\n",
        "  private _storage: shared u64[];\n",
        "  private _start: u8;\n",
        "  private _length: i64;\n",
        "  private _extra: u64;\n",
        "  init() {\n",
        "    self._storage = new u64[]();\n",
        "    self._start = 0u8;\n",
        "    self._length = 0;\n",
        "    self._extra = 0u;\n",
        "  }\n",
        "}\n",
    );
    let mut sources = vec![
        (
            application.join("app.ska"),
            "import feature;\nfn main() -> i64 { \"app\"; return 0; }\n",
        ),
        (
            application.join("feature.ska"),
            "public fn value() -> unit { \"feature\"; }\n",
        ),
    ];
    sources.extend(
        canonical_standard_library_sources(&[("std/str.ska", malformed_string)])
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
    assert!(resolved.diagnostics.has_errors());

    normalize_module_fixture_output(
        fixture.path(),
        format!(
            "GRAPH\n{}DIAGNOSTICS\n{}",
            dump_module_graph(&graph),
            render_diagnostics(graph.sources(), &resolved.diagnostics),
        ),
    )
}
