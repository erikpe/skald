use super::*;
use crate::{
    backend::{emit_assembly, Target},
    mir::{lower_hir, verify_mir},
    test_support::load_module_sources,
    typeck::type_check,
};

#[test]
fn single_file_resolution_reports_imports_as_unsupported_module_syntax() {
    let output = resolve_text(
        "import std::Str;\n\
         fn main() -> unit {}\n",
    );

    assert!(output.has_errors());
    let diagnostics = output.diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, UNSUPPORTED_MODULE_SYNTAX);
    assert!(diagnostics[0]
        .message
        .contains("whole-program module compilation"));
}

#[test]
fn qualified_uses_do_not_panic_or_degrade_to_unknown_name_diagnostics() {
    let output = resolve_text(
        "fn main() -> unit {\n\
           std::Str::make();\n\
         }\n",
    );

    assert!(output.has_errors());
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == UNSUPPORTED_MODULE_SYNTAX));
}

#[test]
fn graph_resolution_allocates_canonical_global_ids_and_direct_public_surfaces() {
    let (_workspace, graph) = load_module_sources(
        "z::entry",
        &[
            (
                "z/entry.ska",
                concat!(
                    "import m;\n",
                    "import a;\n",
                    "fn main() -> i64 { return local(); }\n",
                    "fn local() -> i64 { return 9; }\n",
                ),
            ),
            (
                "m.ska",
                concat!(
                    "class Thing { init() {} }\n",
                    "public fn value() -> i64 { return local(); }\n",
                    "fn local() -> i64 { return 2; }\n",
                ),
            ),
            (
                "a.ska",
                concat!(
                    "public class Thing { init() {} }\n",
                    "public fn value() -> i64 { return local(); }\n",
                    "fn local() -> i64 { return 1; }\n",
                    "fn main() -> unit {}\n",
                ),
            ),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(
        output.diagnostics.is_empty(),
        "graph must resolve: {:?}",
        output.diagnostics
    );
    let program = output.program;
    let resolved_dump = dump_resolved(&program);
    assert!(resolved_dump.contains("Module m0\n      public c0 \"Thing\""));
    assert!(resolved_dump.contains("Module m2\n      private f5 \"main\""));
    assert_eq!(program.modules.selected().index(), 2);
    assert_eq!(program.entry_function, Some(FunctionId::new(5)));
    assert_eq!(
        program
            .declarations
            .iter()
            .map(|declaration| (declaration.id.index(), declaration.module.index()))
            .collect::<Vec<_>>(),
        [(0, 0), (1, 0), (2, 0), (3, 1), (4, 1), (5, 2), (6, 2)]
    );
    assert_eq!(
        program
            .classes
            .iter()
            .map(|class| (class.id.index(), class.module.index(), class.name.as_str()))
            .collect::<Vec<_>>(),
        [(0, 0, "Thing"), (1, 1, "Thing")]
    );

    let a = program
        .module_declarations
        .get(crate::identity::ModuleId::new(0))
        .unwrap();
    assert_eq!(
        a.public_surface()
            .map(|declaration| declaration.name.as_str())
            .collect::<Vec<_>>(),
        ["Thing", "value"]
    );
    let m = program
        .module_declarations
        .get(crate::identity::ModuleId::new(1))
        .unwrap();
    assert_eq!(
        m.public_surface()
            .map(|declaration| declaration.name.as_str())
            .collect::<Vec<_>>(),
        ["value"]
    );

    let checked = type_check(&program);
    assert!(
        checked.diagnostics.is_empty(),
        "flat program must type check: {:?}",
        checked.diagnostics
    );
    let hir = checked.hir.unwrap();
    let hir_dump = crate::hir::dump_hir(&hir);
    assert!(hir_dump.contains("Declaration f0 module m0 \"value\""));
    assert!(hir_dump.contains("Declaration f5 module m2 \"main\""));
    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let mir_dump = crate::mir::dump_mir(&mir);
    assert!(mir_dump.contains("Declaration f3 module m1 \"value\""));
    assert!(mir_dump.contains("Class c1 module m1 \"Thing\""));
    assert_eq!(mir.definitions.iter().count(), 7);
    let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    assert!(assembly.contains(".Lska_fn_0:"));
    assert!(assembly.contains(".Lska_fn_6:"));
    assert!(assembly.contains("call .Lska_fn_5"));
    assert!(assembly.contains("call .Lska_fn_6"));
}

#[test]
fn graph_resolution_keeps_unqualified_lookup_module_local() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import dep;\nfn main() -> i64 { return value(); }\n",
            ),
            ("dep.ska", "public fn value() -> i64 { return 1; }\n"),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(output.has_errors());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == UNKNOWN_NAME
            && diagnostic
                .message
                .contains("unknown function or class `value`")
            && diagnostic.labels[0].span.source_id()
                == graph
                    .module(graph.entry())
                    .unwrap()
                    .provenance()
                    .source_id()
    }));
}

#[test]
fn graph_resolution_rejects_duplicates_only_within_the_owning_module() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import dep;\nfn same() -> i64 { return 1; }\nfn main() -> i64 { return same(); }\n",
            ),
            (
                "dep.ska",
                "fn same() -> i64 { return 2; }\nfn same() -> i64 { return 3; }\n",
            ),
        ],
    );

    let output = resolve_module_graph(&graph);
    let duplicates = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DUPLICATE_TOP_LEVEL)
        .collect::<Vec<_>>();
    assert_eq!(duplicates.len(), 1);
    assert_eq!(
        duplicates[0].labels[0].span.source_id(),
        graph
            .find(&"dep".parse().unwrap())
            .unwrap()
            .provenance()
            .source_id()
    );
}

#[test]
fn graph_resolution_reports_cross_file_hierarchy_and_signature_uses_in_the_owning_source() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import dep;\n",
                    "class Derived extends Base { init() {} }\n",
                    "fn consume(value: Base) -> unit {}\n",
                    "fn main() -> unit {}\n",
                ),
            ),
            ("dep.ska", "public class Base { init() {} }\n"),
        ],
    );

    let output = resolve_module_graph(&graph);
    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == INVALID_BASE_CLASS)
        .expect("an imported class is not an unqualified local base before MS6/MS7");
    assert_eq!(
        diagnostic.labels[0].span.source_id(),
        graph
            .module(graph.entry())
            .unwrap()
            .provenance()
            .source_id()
    );
    let signature_diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == UNKNOWN_TYPE)
        .expect("an imported class is not an unqualified signature type before MS6/MS7");
    assert_eq!(
        signature_diagnostic.labels[0].span.source_id(),
        graph
            .module(graph.entry())
            .unwrap()
            .provenance()
            .source_id()
    );
}
