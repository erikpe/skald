use std::path::PathBuf;

use skald_compiler::{
    backend::{emit_assembly, BackendInput, Target},
    diagnostics::render_diagnostics,
    driver::EntrySelector,
    hir::dump_hir,
    mir::{dump_mir, lower_preliminary_hir, verify_preliminary_mir},
    module::{
        dump_module_graph, load_module_graph, normalize_provider_roots, ProviderRootConfiguration,
    },
    passes::{
        run_mir_pipeline,
        static_lifecycle::{
            plan_static_lifetimes, synthesize_static_lifecycle, verify_planned_mir,
        },
    },
    resolve::{dump_resolved, resolve_module_graph},
    typeck::type_check,
};

use super::{
    fixture::{link_directory, write_source, ModuleFixture},
    normalization::normalize_module_fixture_output,
};

pub(crate) fn module_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("module-products", variant);
    let application = fixture.path().join("application");
    let dependencies = fixture.path().join("dependencies");
    let application_alias = fixture.path().join("application-alias");
    link_directory(&application, &application_alias);

    let imports = if variant == 0 {
        "import first;\nimport second;\nfrom second import Item as SecondItem;\n"
    } else {
        "from second import Item as SecondItem;\nimport second;\nimport first;\n"
    };
    let sources = [
        (
            application.join("app.ska"),
            format!(
                "{imports}\n{}",
                source_body_after_imports(include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/golden/modules/cases/cycle/modules/app.ska"
                )))
            ),
        ),
        (
            dependencies.join("first.ska"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/golden/modules/cases/cycle/modules/first.ska"
            ))
            .to_owned(),
        ),
        (
            dependencies.join("second.ska"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/golden/modules/cases/cycle/modules/second.ska"
            ))
            .to_owned(),
        ),
    ];
    for index in if variant == 0 { [0, 1, 2] } else { [2, 1, 0] } {
        write_source(&sources[index].0, &sources[index].1);
    }

    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::module_root(PathBuf::from("application-alias")),
            ProviderRootConfiguration::module_root(PathBuf::from("./dependencies")),
            ProviderRootConfiguration::module_root(PathBuf::from("application")),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(PathBuf::from("application")),
            ProviderRootConfiguration::module_root(PathBuf::from("dependencies/.")),
            ProviderRootConfiguration::module_root(PathBuf::from("./application-alias")),
        ]
    };
    let providers = normalize_provider_roots(fixture.path(), &configurations).unwrap();
    let entry = if variant == 0 {
        EntrySelector::Module("app".parse().unwrap())
    } else {
        EntrySelector::File(application_alias.join("app.ska"))
    };
    let graph = load_module_graph(&entry, fixture.path(), &providers).unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty());
    let hir = checked.hir.unwrap();
    let preliminary = lower_preliminary_hir(&hir);
    let preliminary = verify_preliminary_mir(preliminary).unwrap();
    let planned = plan_static_lifetimes(preliminary).unwrap();
    let mir = run_mir_pipeline(synthesize_static_lifecycle(
        verify_planned_mir(planned).unwrap(),
    ))
    .unwrap();
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

pub(crate) fn module_diagnostic_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("module-diagnostics", variant);
    let modules = fixture.path().join("modules");
    let modules_alias = fixture.path().join("modules-alias");
    let sources = [
        (
            modules.join("app.ska"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/golden/modules/cases/cycle_diagnostics/modules/app.ska"
            )),
        ),
        (
            modules.join("left.ska"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/golden/modules/cases/cycle_diagnostics/modules/left.ska"
            )),
        ),
        (
            modules.join("right.ska"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/golden/modules/cases/cycle_diagnostics/modules/right.ska"
            )),
        ),
    ];
    for index in if variant == 0 { [0, 1, 2] } else { [2, 1, 0] } {
        write_source(&sources[index].0, sources[index].1);
    }
    link_directory(&modules, &modules_alias);
    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::module_root(modules_alias.clone()),
            ProviderRootConfiguration::module_root(modules.clone()),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(modules.clone()),
            ProviderRootConfiguration::module_root(modules_alias),
        ]
    };
    let providers = normalize_provider_roots(fixture.path(), &configurations).unwrap();
    let entry = EntrySelector::Module("app".parse().unwrap());
    let graph = load_module_graph(&entry, fixture.path(), &providers).unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.has_errors());

    normalize_module_fixture_output(
        fixture.path(),
        render_diagnostics(graph.sources(), &resolved.diagnostics),
    )
}

fn source_body_after_imports(source: &str) -> &str {
    source
        .split_once("\n\n")
        .expect("a reusable module fixture must separate imports from its body")
        .1
}
