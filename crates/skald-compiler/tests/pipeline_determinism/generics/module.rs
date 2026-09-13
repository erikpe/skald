use std::path::PathBuf;

use skald_compiler::{
    diagnostics::render_diagnostics,
    driver::EntrySelector,
    hir::dump_hir,
    mir::{dump_mir, lower_preliminary_hir, verify_preliminary_mir},
    module::{
        dump_module_graph, load_module_graph, normalize_provider_roots, ProviderRootConfiguration,
    },
    passes::static_lifecycle::{
        dump_planned_mir, plan_static_lifetimes, synthesize_static_lifecycle, verify_planned_mir,
    },
    resolve::{dump_resolved, resolve_module_graph},
    typeck::type_check,
};

use super::super::{
    fixture::{link_directory, write_source, ModuleFixture},
    normalization::normalize_fixture_paths,
};

pub(crate) fn generic_module_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("generic-module-products", variant);
    let modules = fixture.path().join("modules");
    let modules_alias = fixture.path().join("modules-alias");
    link_directory(&modules, &modules_alias);
    let sources = [
        (
            modules.join("app.ska"),
            "import model;\n\
             from wrapper import Envelope;\n\
             fn accept(\n\
               ref cache: model::Cache<model::Item>,\n\
               ref envelope: Envelope<model::Item>\n\
             ) -> unit {}\n\
             fn main() -> i64 {\n\
               model::Cache<model::Item>.count = 42;\n\
               return model::Cache<model::Item>.count;\n\
             }\n",
        ),
        (
            modules.join("model.ska"),
            "public class Item {\n\
               value: i64;\n\
               init(value: i64) { self.value = value; }\n\
               copy(ref source: Item) { self.value = source.value; }\n\
               assign(ref source: Item) { self.value = source.value; }\n\
             }\n\
             public class Cache<T> {\n\
               static cached: T?;\n\
               static count: i64 = 0;\n\
               value: T;\n\
               init(ref value: T) { self.value = value; }\n\
             }\n",
        ),
        (
            modules.join("wrapper.ska"),
            "import model;\n\
             public class Envelope<T> {\n\
               value: model::Cache<T>;\n\
               init(ref value: model::Cache<T>) { self.value = value; }\n\
             }\n",
        ),
    ];
    for index in if variant == 0 { [0, 1, 2] } else { [2, 0, 1] } {
        write_source(&sources[index].0, sources[index].1);
    }

    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::module_root(PathBuf::from("modules-alias")),
            ProviderRootConfiguration::module_root(PathBuf::from("modules")),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(PathBuf::from("modules")),
            ProviderRootConfiguration::module_root(PathBuf::from(
                "modules-alias/..//modules-alias",
            )),
        ]
    };
    let providers = normalize_provider_roots(fixture.path(), &configurations).unwrap();
    let entry = EntrySelector::Module("app".parse().unwrap());
    let graph = load_module_graph(&entry, fixture.path(), &providers).unwrap();
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
    let preliminary = verify_preliminary_mir(preliminary).unwrap();
    let planned = plan_static_lifetimes(preliminary).unwrap();
    let planned_dump = dump_planned_mir(&planned);
    let final_mir = synthesize_static_lifecycle(verify_planned_mir(planned).unwrap());

    normalize_fixture_paths(
        fixture.path(),
        format!(
            "GRAPH\n{}DIAGNOSTICS\n{}RESOLVED\n{}HIR\n{}PLANNED MIR\n{}FINAL MIR\n{}",
            dump_module_graph(&graph),
            render_diagnostics(graph.sources(), &resolved.diagnostics),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
            planned_dump,
            dump_mir(&final_mir),
        ),
    )
}
