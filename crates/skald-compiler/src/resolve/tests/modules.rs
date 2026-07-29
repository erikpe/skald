use super::*;
use crate::{
    backend::{emit_assembly, Target},
    mir::{lower_hir, verify_mir},
    test_support::{load_module_sources, run_native_assembly},
    typeck::type_check,
};

#[test]
fn source_text_resolution_requires_a_module_context_for_imports() {
    let output = resolve_text(
        "import std::Str;\n\
         fn main() -> unit {}\n",
    );

    assert!(output.has_errors());
    let diagnostics = output.diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, MODULE_CONTEXT_REQUIRED);
    assert!(diagnostics[0]
        .message
        .contains("whole-program module compilation"));
}

#[test]
fn source_text_string_use_reports_the_missing_language_item() {
    let (_, parsed) = crate::test_support::parse_source(concat!(
        "fn first() -> i64 { return \"first\"; }\n",
        "fn main() -> i64 { return \"provider required\"; }\n",
    ));
    assert!(!parsed.has_errors());

    let output = resolve(&parsed.ast);
    let diagnostics = output.diagnostics.iter().collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, MISSING_STRING_LANGUAGE_ITEM);
    assert!(diagnostics[0].message.contains("std::str::Str"));
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
        .all(|diagnostic| diagnostic.code == MODULE_CONTEXT_REQUIRED));
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
    assert!(assembly.contains(".Lska.fn.a.value.f0:"));
    assert!(assembly.contains(".Lska.fn.m.value.f3:"));
    assert!(assembly.contains(".Lska.fn.z.entry.local.f6:"));
    assert!(assembly.contains(".Lska.class.a.Thing.c0.dispatch:"));
    assert!(assembly.contains(".Lska.class.m.Thing.c1.dispatch:"));
    assert!(assembly.contains("call .Lska.fn.z.entry.main.f5"));
    assert!(assembly.contains("call .Lska.fn.z.entry.local.f6"));
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
fn member_privacy_is_independent_of_module_visibility() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import dep;\n",
                    "fn inspect(ref secret: dep::Secret) -> i64 { return secret.value; }\n",
                    "fn main() -> i64 { return 0; }\n",
                ),
            ),
            (
                "dep.ska",
                concat!(
                    "public class Secret {\n",
                    "  private value: i64;\n",
                    "  init(value: i64) { self.value = value; }\n",
                    "}\n",
                ),
            ),
        ],
    );

    let output = resolve_module_graph(&graph);
    let diagnostics = output.diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, PRIVATE_MEMBER_ACCESS);
    assert!(diagnostics[0]
        .message
        .contains("member `value` is private to class `Secret`"));
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
        .expect("a module import does not add unqualified names");
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
        .expect("a module import does not add unqualified signature types");
    assert_eq!(
        signature_diagnostic.labels[0].span.source_id(),
        graph
            .module(graph.entry())
            .unwrap()
            .provenance()
            .source_id()
    );
}

#[test]
fn qualified_imports_resolve_all_declaration_use_contexts_to_existing_ids() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import lib::types;\n",
                    "import lib::types as types;\n",
                    "class Derived extends lib::types::Base implements types::View {\n",
                    "  item: types::Thing;\n",
                    "  init() { super(); self.item = types::Thing(); }\n",
                    "  fn copy(item: types::Thing) -> types::Thing {\n",
                    "    var made: types::Thing = types::Thing();\n",
                    "    var owned: shared types::Thing = new types::Thing();\n",
                    "    return types::identity(made);\n",
                    "  }\n",
                    "}\n",
                    "fn main() -> i64 {\n",
                    "  var made: types::Thing = lib::types::Thing();\n",
                    "  return types::external_value();\n",
                    "}\n",
                ),
            ),
            (
                "lib/types.ska",
                concat!(
                    "public interface View {}\n",
                    "public class Base { init() {} }\n",
                    "public class Thing { init() {} }\n",
                    "public fn identity(value: Thing) -> Thing { return value; }\n",
                    "public extern fn external_value() -> i64;\n",
                ),
            ),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(
        output.diagnostics.is_empty(),
        "qualified program must resolve: {:?}",
        output.diagnostics
    );
    let program = output.program;
    let app_bindings = program
        .module_bindings
        .get(graph.entry())
        .expect("the entry module has a binding namespace");
    assert_eq!(app_bindings.iter().count(), 2);
    assert!(app_bindings
        .iter()
        .all(|binding| binding.target == crate::identity::ModuleId::new(1)));

    let dump = dump_resolved(&program);
    assert!(dump.contains("lib::types -> m1 lib::types"));
    assert!(dump.contains("types -> m1 lib::types"));
    assert!(dump.contains("DirectBase c1"));
    assert!(dump.contains("Class c2"));
    assert!(dump.contains("Interface i0"));
    assert!(dump.contains("DirectCall f2"));

    let checked = type_check(&program);
    assert!(
        checked.diagnostics.is_empty(),
        "qualified identities must type check: {:?}",
        checked.diagnostics
    );
    let hir = checked.hir.unwrap();
    assert!(!crate::hir::dump_hir(&hir).contains("ModuleBindings"));
    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    assert!(!crate::mir::dump_mir(&mir).contains("ModuleBindings"));
    let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    assert!(assembly.contains("call external_value"));
}

#[test]
fn qualified_cast_and_type_test_targets_resolve_to_canonical_declarations() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import dep as types;\n",
                    "fn inspect(ref value: Obj) -> unit {\n",
                    "  value is types::Thing;\n",
                    "  (types::Thing) value;\n",
                    "}\n",
                    "fn main() -> unit {}\n",
                ),
            ),
            ("dep.ska", "public class Thing { init() {} }\n"),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(
        output.diagnostics.is_empty(),
        "qualified cast targets must resolve: {:?}",
        output.diagnostics
    );
    let dump = dump_resolved(&output.program);
    assert!(dump.contains("TypeTest target class c0"));
    assert!(dump.contains("ObjectCast target class c0"));
}

#[test]
fn module_bindings_allow_multiple_names_and_an_independent_ordinary_namespace() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import dep as second;\n",
                    "import dep;\n",
                    "import dep as first;\n",
                    "class first { init() {} }\n",
                    "fn read(first: i64) -> i64 {\n",
                    "  var second: i64 = first;\n",
                    "  return dep::value() + first::value() + second::value() + second;\n",
                    "}\n",
                    "fn main() -> i64 { return read(0); }\n",
                ),
            ),
            ("dep.ska", "public fn value() -> i64 { return 1; }\n"),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(
        output.diagnostics.is_empty(),
        "module and ordinary namespaces must remain separate: {:?}",
        output.diagnostics
    );
    let bindings = output
        .program
        .module_bindings
        .get(graph.entry())
        .unwrap()
        .iter()
        .map(|binding| binding.local_path.to_string())
        .collect::<Vec<_>>();
    assert_eq!(bindings, ["dep", "first", "second"]);
}

#[test]
fn module_binding_collisions_are_rejected_in_source_order() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import dep;\n",
                    "import dep;\n",
                    "import dep as shared;\n",
                    "import other as shared;\n",
                    "fn main() -> unit {}\n",
                ),
            ),
            ("dep.ska", "public fn dep_value() -> unit {}\n"),
            ("other.ska", "public fn other_value() -> unit {}\n"),
        ],
    );

    let output = resolve_module_graph(&graph);
    let diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DUPLICATE_MODULE_BINDING)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0]
        .message
        .contains("repeated module binding `dep`"));
    assert!(diagnostics[1]
        .message
        .contains("conflicting module binding `shared`"));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.labels.len() == 2));
}

#[test]
fn qualified_lookup_enforces_visibility_leaf_and_declaration_kind() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import dep;\n",
                    "fn consume(value: dep::public_fn) -> unit {}\n",
                    "fn main() -> unit {\n",
                    "  dep::private_fn();\n",
                    "  dep::missing();\n",
                    "  dep::PublicView();\n",
                    "}\n",
                ),
            ),
            (
                "dep.ska",
                concat!(
                    "fn private_fn() -> unit {}\n",
                    "public fn public_fn() -> unit {}\n",
                    "public interface PublicView {}\n",
                ),
            ),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == PRIVATE_DECLARATION
            && diagnostic.message.contains("dep::private_fn")
            && diagnostic.labels.len() == 2));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == UNKNOWN_QUALIFIED_DECLARATION
            && diagnostic
                .message
                .contains("no declaration named `missing`")
            && diagnostic.labels.len() == 2
    }));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == UNKNOWN_TYPE
            && diagnostic.message.contains("does not name a type")
            && diagnostic.labels.len() == 2
    }));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_CALL_TARGET
            && diagnostic
                .message
                .contains("interface `dep::PublicView` is not callable")
    }));
}

#[test]
fn qualified_lookup_requires_the_exact_direct_binding() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import bridge;\n",
                    "import tree;\n",
                    "fn main() -> unit {\n",
                    "  target::value();\n",
                    "  tree::leaf::value();\n",
                    "  bridge::target::value();\n",
                    "}\n",
                ),
            ),
            (
                "bridge.ska",
                "import target;\npublic fn value() -> unit {}\n",
            ),
            ("target.ska", "public fn value() -> unit {}\n"),
            (
                "tree.ska",
                "import tree::leaf;\npublic fn value() -> unit {}\n",
            ),
            ("tree/leaf.ska", "public fn value() -> unit {}\n"),
        ],
    );

    let output = resolve_module_graph(&graph);
    let diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == UNKNOWN_MODULE_BINDING)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics[0].message.contains("`target`"));
    assert!(diagnostics[0].labels.len() >= 2);
    assert!(diagnostics[1].message.contains("`tree::leaf`"));
    assert!(diagnostics[2].message.contains("`bridge::target`"));
    assert!(diagnostics[2]
        .notes
        .iter()
        .any(|note| note.contains("descendant module directly")));
}

#[test]
fn module_aliases_are_the_only_local_spelling_they_introduce() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import dep as renamed;\n",
                    "fn valid() -> i64 { return renamed::value(); }\n",
                    "fn main() -> i64 { return dep::value(); }\n",
                ),
            ),
            ("dep.ska", "public fn value() -> i64 { return 1; }\n"),
        ],
    );

    let output = resolve_module_graph(&graph);
    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == UNKNOWN_MODULE_BINDING)
        .expect("the canonical path is not also bound when an alias is present");
    assert!(diagnostic.message.contains("`dep`"));
    assert!(diagnostic
        .notes
        .iter()
        .any(|note| note.contains("use `renamed::value`")));
}

#[test]
fn qualified_internal_calls_execute_through_the_native_backend() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import dep as library;\nfn main() -> i64 { return library::value(); }\n",
            ),
            ("dep.ska", "public fn value() -> i64 { return 42; }\n"),
        ],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty());
    let mir = lower_hir(&checked.hir.unwrap());
    verify_mir(&mir).unwrap();
    let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();

    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn qualified_and_selectively_imported_static_methods_execute_by_class_identity() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import dep as library;\n",
                    "from dep import Tools;\n",
                    "fn main() -> i64 { return library::Tools.answer(40) + Tools.answer(2); }\n",
                ),
            ),
            (
                "dep.ska",
                concat!(
                    "public class Tools {\n",
                    "  init() {}\n",
                    "  static fn answer(value: i64) -> i64 { return value; }\n",
                    "}\n",
                ),
            ),
        ],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let mir = lower_hir(&checked.hir.unwrap());
    verify_mir(&mir).unwrap();
    let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();

    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn cross_module_private_static_access_uses_member_privacy() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import dep;\nfn main() -> i64 { return dep::Tools.hidden(); }\n",
            ),
            (
                "dep.ska",
                concat!(
                    "public class Tools {\n",
                    "  init() {}\n",
                    "  private static fn hidden() -> i64 { return 42; }\n",
                    "}\n",
                ),
            ),
        ],
    );
    let resolved = resolve_module_graph(&graph);

    assert!(resolved
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == PRIVATE_MEMBER_ACCESS));
}

#[test]
fn selective_imports_resolve_supported_declarations_in_all_use_contexts() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "from dep import Base, View, Thing, identity as copy, external_value;\n",
                    "class Derived extends Base implements View {\n",
                    "  item: Thing;\n",
                    "  init() { super(); self.item = Thing(); }\n",
                    "}\n",
                    "fn consume(value: Thing) -> unit {\n",
                    "  var copied: Thing = copy(value);\n",
                    "}\n",
                    "fn accept(ref value: Thing) -> unit {}\n",
                    "fn inspect(ref value: Obj) -> unit {\n",
                    "  var matches: bool = value is Thing;\n",
                    "  accept((Thing) value);\n",
                    "}\n",
                    "fn main() -> i64 {\n",
                    "  var value: Thing = Thing();\n",
                    "  var owned: shared Thing = new Thing();\n",
                    "  consume(value);\n",
                    "  return external_value();\n",
                    "}\n",
                ),
            ),
            (
                "dep.ska",
                concat!(
                    "public interface View {}\n",
                    "public class Base { init() {} }\n",
                    "public class Thing { init() {} }\n",
                    "public fn identity(value: Thing) -> Thing { return value; }\n",
                    "public extern fn external_value() -> i64;\n",
                ),
            ),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(
        output.diagnostics.is_empty(),
        "selective program must resolve: {:?}",
        output.diagnostics
    );
    let program = output.program;
    let bindings = program.ordinary_bindings.get(graph.entry()).unwrap();
    assert_eq!(
        bindings
            .iter()
            .map(|binding| binding.local_name.as_str())
            .collect::<Vec<_>>(),
        ["Base", "Thing", "View", "copy", "external_value"]
    );
    assert!(bindings
        .iter()
        .all(|binding| binding.target_module == crate::identity::ModuleId::new(1)));

    let dump = dump_resolved(&program);
    assert!(dump.contains("OrdinaryBindings"));
    assert!(dump.contains("copy -> f4 m1 dep::identity"));
    assert!(dump.contains("DirectBase c1"));
    assert!(dump.contains("Implements i0"));
    assert!(dump.contains("DirectCall f4"));

    let checked = type_check(&program);
    assert!(
        checked.diagnostics.is_empty(),
        "selected identities must type check: {:?}",
        checked.diagnostics
    );
    let hir = checked.hir.unwrap();
    assert!(!crate::hir::dump_hir(&hir).contains("OrdinaryBindings"));
    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    assert!(!crate::mir::dump_mir(&mir).contains("OrdinaryBindings"));
    assert!(emit_assembly(Target::X86_64SysV, &mir)
        .unwrap()
        .contains("call external_value"));
}

#[test]
fn selective_imports_allow_multiple_names_and_lexical_shadowing() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "from dep import value as first, value as second, Thing;\n",
                    "fn shadow(Thing: i64) -> i64 {\n",
                    "  var first: i64 = Thing;\n",
                    "  return first;\n",
                    "}\n",
                    "fn main() -> i64 { return second() + shadow(40); }\n",
                ),
            ),
            (
                "dep.ska",
                concat!(
                    "public fn value() -> i64 { return 2; }\n",
                    "public class Thing { init() {} }\n",
                ),
            ),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(
        output.diagnostics.is_empty(),
        "lexical names may shadow selective imports: {:?}",
        output.diagnostics
    );
    let bindings = output
        .program
        .ordinary_bindings
        .get(graph.entry())
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 3);
    assert_eq!(bindings[1].target, bindings[2].target);
}

#[test]
fn selective_imports_reject_local_and_repeated_ordinary_bindings() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "from dep import value;\n",
                    "from dep import value;\n",
                    "from dep import other as local;\n",
                    "fn local() -> unit {}\n",
                    "fn main() -> unit {}\n",
                ),
            ),
            (
                "dep.ska",
                concat!(
                    "public fn value() -> unit {}\n",
                    "public fn other() -> unit {}\n",
                ),
            ),
        ],
    );

    let output = resolve_module_graph(&graph);
    let diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DUPLICATE_ORDINARY_BINDING)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0]
        .message
        .contains("repeated imported name `value`"));
    assert!(diagnostics[1]
        .message
        .contains("conflicts with a local declaration"));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.labels.len() == 2));
}

#[test]
fn selective_imports_require_direct_public_ownership() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "from dep import hidden, missing, member, value;\n",
                    "from bridge import value;\n",
                    "fn consume(item: value) -> unit {}\n",
                    "fn main() -> unit {}\n",
                ),
            ),
            (
                "bridge.ska",
                concat!(
                    "from dep import value;\n",
                    "public fn bridge_value() -> unit { value(); }\n",
                ),
            ),
            (
                "dep.ska",
                concat!(
                    "fn hidden() -> unit {}\n",
                    "public fn value() -> unit {}\n",
                    "public class Container { init() {} fn member() -> unit {} }\n",
                ),
            ),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == PRIVATE_DECLARATION
            && diagnostic.message.contains("dep::hidden")
            && diagnostic.labels.len() == 2
    }));
    let missing = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == UNKNOWN_IMPORTED_DECLARATION)
        .collect::<Vec<_>>();
    assert_eq!(missing.len(), 3);
    assert!(missing
        .iter()
        .any(|diagnostic| diagnostic.message.contains("declaration named `member`")));
    assert!(missing
        .iter()
        .any(|diagnostic| diagnostic.message.contains("bridge")
            && diagnostic.message.contains("`value`")));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == UNKNOWN_TYPE
            && diagnostic.message.contains("does not name a type")
            && diagnostic.labels.len() == 2
    }));

    let bridge = output
        .program
        .modules
        .find(&"bridge".parse().unwrap())
        .unwrap()
        .module_id();
    assert_eq!(
        output
            .program
            .module_declarations
            .get(bridge)
            .unwrap()
            .public_surface()
            .map(|declaration| declaration.name.as_str())
            .collect::<Vec<_>>(),
        ["bridge_value"]
    );
}

#[test]
fn selective_import_sources_are_canonical_and_do_not_bind_modules() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import dep as source;\n",
                    "from source import value;\n",
                    "fn main() -> i64 { return value() + source::value(); }\n",
                ),
            ),
            ("dep.ska", "public fn value() -> i64 { return 2; }\n"),
            ("source.ska", "public fn value() -> i64 { return 40; }\n"),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(
        output.diagnostics.is_empty(),
        "selective sources must ignore module aliases: {:?}",
        output.diagnostics
    );
    let binding = output
        .program
        .ordinary_bindings
        .get(graph.entry())
        .unwrap()
        .get("value")
        .unwrap();
    assert_eq!(
        output
            .program
            .modules
            .get(binding.target_module)
            .unwrap()
            .module_path()
            .to_string(),
        "source"
    );

    let checked = type_check(&output.program);
    assert!(checked.diagnostics.is_empty());
    let mir = lower_hir(&checked.hir.unwrap());
    verify_mir(&mir).unwrap();
    let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn selective_imports_do_not_create_qualified_module_bindings() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "from dep import value;\nfn main() -> i64 { return dep::value(); }\n",
            ),
            ("dep.ska", "public fn value() -> i64 { return 1; }\n"),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == UNKNOWN_MODULE_BINDING));
    assert!(output
        .program
        .module_bindings
        .get(graph.entry())
        .unwrap()
        .iter()
        .next()
        .is_none());
    assert!(output
        .program
        .ordinary_bindings
        .get(graph.entry())
        .unwrap()
        .get("value")
        .is_some());
}
