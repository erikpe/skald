use skald_compiler::{
    backend::{emit_assembly, BackendInput, Target},
    diagnostics::render_diagnostics,
    driver::EntrySelector,
    hir::dump_hir,
    mir::dump_mir,
    module::{
        dump_module_graph, load_module_graph, normalize_provider_roots, ModuleGraph,
        ProviderRootConfiguration,
    },
    resolve::{dump_resolved, resolve_module_graph},
    typeck::type_check,
};

use crate::standard_library::{canonical_standard_library_sources, CANONICAL_IO_SOURCE};

use super::super::{
    fixture::{write_source, ModuleFixture},
    normalization::normalize_module_fixture_output,
    source::lower_final_hir,
};

pub(crate) fn io_phase_dump(variant: usize) -> String {
    let (fixture, graph) = load_io_graph(variant, CANONICAL_IO_SOURCE);
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
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
            "GRAPH\n{}RESOLVED\n{}HIR\n{}MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
            dump_mir(&mir),
            assembly,
        ),
    )
}

pub(crate) fn io_diagnostic_dump(variant: usize) -> String {
    let malformed_io =
        CANONICAL_IO_SOURCE.replace("intrinsic fn _io_close", "public intrinsic fn _io_close");
    let (fixture, graph) = load_io_graph(variant, &malformed_io);
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.has_errors());

    normalize_module_fixture_output(
        fixture.path(),
        format!(
            "GRAPH\n{}DIAGNOSTICS\n{}RESOLVED\n{}",
            dump_module_graph(&graph),
            render_diagnostics(graph.sources(), &resolved.diagnostics),
            dump_resolved(&resolved.program),
        ),
    )
}

fn load_io_graph(variant: usize, io_source: &str) -> (ModuleFixture, ModuleGraph) {
    let fixture = ModuleFixture::new("io-products", variant);
    let application = fixture.path().join("application");
    let standard_library = fixture.path().join("standard-library");
    let mut sources = vec![(
        application.join("app.ska"),
        concat!(
            "import std::io;\n",
            "from std::str import Str;\n",
            "fn main() -> i64 {\n",
            "  var path: Str = \"input.bin\";\n",
            "  var stdin: Str = std::io::read_stdin();\n",
            "  var file: Str = std::io::read_file(path);\n",
            "  std::io::write_stdout(stdin);\n",
            "  std::io::write_stderr(file);\n",
            "  return 0;\n",
            "}\n",
        ),
    )];
    sources.extend(
        canonical_standard_library_sources(&[("std/io.ska", io_source)])
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
    (fixture, graph)
}
