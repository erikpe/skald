use std::fmt::Write;

use super::model::ModuleGraph;

/// Dumps graph identity, provenance, and canonical direct edges.
pub fn dump_module_graph(graph: &ModuleGraph) -> String {
    let mut output = String::new();
    let entry = graph
        .module(graph.entry())
        .expect("a graph entry references a loaded module");
    let _ = writeln!(
        output,
        "entry {} {}",
        graph.entry(),
        entry.provenance().module_path()
    );
    for module in graph.modules() {
        let provenance = module.provenance();
        let source = provenance.source_location();
        let _ = writeln!(
            output,
            "module {} {} source{} {} {}",
            provenance.module_id(),
            provenance.module_path(),
            provenance.source_id().index(),
            provenance.provider_id(),
            provenance.package_id()
        );
        let _ = writeln!(
            output,
            "  relative {}",
            source.root_relative_path().display()
        );
        let _ = writeln!(
            output,
            "  display {}",
            source.display_source_path().display()
        );
        if let Some(path) = source.canonical_io_path() {
            let _ = writeln!(output, "  canonical {}", path.display());
        }
        for import in module.imports() {
            let target = graph
                .module(import.target())
                .expect("a graph edge references a loaded module");
            let _ = writeln!(
                output,
                "  import {} {} occurrences={}",
                import.target(),
                target.provenance().module_path(),
                import.import_spans().len()
            );
        }
    }
    output
}
