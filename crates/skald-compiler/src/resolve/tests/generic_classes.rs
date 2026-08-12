use super::*;
use crate::test_support::load_module_sources;

#[test]
fn generic_declarations_receive_non_executable_template_identities() {
    let output = resolve_text(
        "class Box<T> { value: T; }\n\
         class Pair<Left, Right> { left: Left; right: Right; }\n\
         class Ordinary {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.program.classes.len(), 1);
    assert_eq!(
        output.program.classes.get(ClassId::new(0)).unwrap().name,
        "Ordinary"
    );
    assert_eq!(output.program.class_templates.len(), 2);

    let box_template = output
        .program
        .class_templates
        .get(ClassTemplateId::new(0))
        .unwrap();
    assert_eq!(box_template.name, "Box");
    let pair_parameters = output
        .program
        .type_parameters
        .for_template(ClassTemplateId::new(1))
        .unwrap();
    assert_eq!(
        pair_parameters
            .iter()
            .map(|parameter| (parameter.id, parameter.name.as_str()))
            .collect::<Vec<_>>(),
        [
            (TypeParameterId::new(ClassTemplateId::new(1), 0), "Left"),
            (TypeParameterId::new(ClassTemplateId::new(1), 1), "Right"),
        ]
    );
}

#[test]
fn duplicate_type_parameters_are_diagnosed_without_losing_source_order() {
    let output = resolve_text(
        "class Pair<T, T> { value: T; }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.has_errors());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == DUPLICATE_TYPE_PARAMETER)
            .count(),
        1
    );
    assert_eq!(
        output
            .program
            .type_parameters
            .for_template(ClassTemplateId::new(0))
            .unwrap()
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<Vec<_>>(),
        ["T", "T"]
    );
}

#[test]
fn generic_type_uses_distinguish_raw_wrong_kind_arity_and_valid_application() {
    let output = resolve_text(
        "class Box<T> { value: T; }\n\
         class Ordinary {}\n\
         fn raw(value: Box) -> unit {}\n\
         fn wrong(value: Ordinary<i64>) -> unit {}\n\
         fn arity(value: Box<i64, bool>) -> unit {}\n\
         fn valid(value: Box<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let codes = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    assert!(codes.contains(&RAW_GENERIC_TYPE));
    assert!(codes.contains(&INVALID_GENERIC_APPLICATION));
    assert!(codes.contains(&GENERIC_ARITY_MISMATCH));
    assert!(codes.contains(&UNSUPPORTED_GENERIC_SYNTAX));
}

#[test]
fn type_parameters_shadow_unqualified_templates_but_not_qualified_ones() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import dep;\nclass Holder<T> { local: T; external: dep::T; }\nfn main() -> i64 { return 0; }\n",
            ),
            ("dep.ska", "public class T<Value> { value: Value; }\n"),
        ],
    );

    let output = resolve_module_graph(&graph);
    let raw = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == RAW_GENERIC_TYPE)
        .collect::<Vec<_>>();
    assert_eq!(raw.len(), 1, "{:?}", output.diagnostics);
    assert!(raw[0].message.contains("dep::T"));
}

#[test]
fn public_templates_support_selective_import_without_instantiation() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "from dep import Box;\nfn main() -> i64 { return 0; }\n",
            ),
            ("dep.ska", "public class Box<T> { value: T; }\n"),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let binding = output
        .program
        .ordinary_bindings
        .iter()
        .flat_map(|module| module.iter())
        .find(|binding| binding.local_name == "Box")
        .unwrap();
    assert_eq!(
        binding.target,
        ResolvedTopLevelId::ClassTemplate(ClassTemplateId::new(0))
    );
    assert!(output.program.classes.is_empty());
}

#[test]
fn private_templates_remain_inaccessible_through_qualification() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import dep;\nclass Use<T> { value: dep::Hidden<T>; }\nfn main() -> i64 { return 0; }\n",
            ),
            ("dep.ska", "class Hidden<T> { value: T; }\n"),
        ],
    );

    let output = resolve_module_graph(&graph);
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == PRIVATE_DECLARATION));
    assert!(output.program.classes.is_empty());
}

#[test]
fn template_ids_follow_canonical_module_and_declaration_order() {
    fn identities(sources: &[(&str, &str)]) -> Vec<(usize, usize, String)> {
        let (_workspace, graph) = load_module_sources("app", sources);
        let output = resolve_module_graph(&graph);
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        output
            .program
            .class_templates
            .iter()
            .map(|template| {
                (
                    template.id.index(),
                    template.module.index(),
                    template.name.clone(),
                )
            })
            .collect()
    }

    let app = "import a;\nimport z;\nclass App<T> { value: T; }\nfn main() -> i64 { return 0; }\n";
    let a = "public class First<T> { value: T; }\npublic class Second<T> { value: T; }\n";
    let z = "public class Last<T> { value: T; }\n";
    let first = identities(&[("z.ska", z), ("app.ska", app), ("a.ska", a)]);
    let second = identities(&[("a.ska", a), ("z.ska", z), ("app.ska", app)]);
    assert_eq!(first, second);
}

#[test]
fn resolved_dump_includes_template_and_parameter_identities() {
    let output = resolve_text(
        "class Pair<Left, Right> { left: Left; right: Right; }\n\
         fn main() -> i64 { return 0; }\n",
    );

    let dump = dump_resolved(&output.program);
    assert!(dump.contains("private template0 \"Pair\""), "{dump}");
    assert!(dump.contains("Template template0 module m0 \"Pair\" parameters template0:type0=\"Left\" template0:type1=\"Right\""), "{dump}");
}

#[test]
fn template_types_preserve_every_constructor_and_definition_site_identity() {
    let output = resolve_text(
        "class Inner<Value> { value: Value; }\n\
         class Shape<T> {\n\
           value: T;\n\
           maybe: T?;\n\
           nested: T??[];\n\
           owner: shared T;\n\
           concrete: Inner<T?[]>;\n\
           fn transform(input: T) -> Inner<T> {\n\
             var local: T?[] = T?[]();\n\
             return Inner<T>(input);\n\
           }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    assert!(
        dump.contains("TypeUse member0:field template1:type0"),
        "{dump}"
    );
    assert!(
        dump.contains("TypeUse member1:field optional (template1:type0)"),
        "{dump}"
    );
    assert!(
        dump.contains("array (optional (optional (template1:type0)))"),
        "{dump}"
    );
    assert!(dump.contains("shared (template1:type0)"), "{dump}");
    assert!(
        dump.contains("template0<array (optional (template1:type0))>"),
        "{dump}"
    );
    assert!(
        dump.contains(
            "Selection argument-dependent inline-construction template0<template1:type0>"
        ),
        "{dump}"
    );
    assert!(output.program.array_types.is_empty());
    assert!(output.program.optional_types.is_empty());
}

#[test]
fn generic_direct_bases_resolve_structurally_but_bare_parameters_do_not() {
    let valid = resolve_text(
        "class Base<T> { value: T; }\n\
         class Derived<T> extends Base<T> { init() { super(); } }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(valid.diagnostics.is_empty(), "{:?}", valid.diagnostics);
    assert!(dump_resolved(&valid.program).contains("DirectBase template0<template1:type0>"));

    let invalid = resolve_text(
        "class Derived<T> extends T { init() { super(); } }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(invalid
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_GENERIC_BASE));
}

#[test]
fn nominal_bounds_resolve_interface_and_requirement_identities() {
    let output = resolve_text(
        "interface Ranked { fn rank() -> i64; }\n\
         class Sorted<T> where T: Ranked {\n\
           fn inspect(ref value: T) -> i64 { return value.rank(); }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    assert!(
        dump.contains("Bound template0:type0 interface i0"),
        "{dump}"
    );
    assert!(dump.contains(
        "Selection bound-member template0:type0 interface i0 requirement i0:requirement0 member rank"
    ), "{dump}");
}

#[test]
fn invalid_unknown_duplicate_and_inaccessible_bounds_are_diagnosed() {
    let local = resolve_text(
        "interface Marker { fn mark() -> unit; }\n\
         class NotInterface {}\n\
         class Invalid<T> where T: Marker, T: Marker, T: NotInterface, Missing: Marker, T: Unknown {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(local
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DUPLICATE_GENERIC_BOUND));
    assert!(
        local
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_GENERIC_BOUND)
            .count()
            >= 3,
        "{:?}",
        local.diagnostics
    );

    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import dep;\nclass Use<T> where T: dep::Hidden {}\nfn main() -> i64 { return 0; }\n",
            ),
            ("dep.ska", "interface Hidden { fn hidden() -> unit; }\n"),
        ],
    );
    let inaccessible = resolve_module_graph(&graph);
    assert!(inaccessible
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == PRIVATE_DECLARATION));
}

#[test]
fn unconstrained_and_ambiguous_parameter_members_are_definition_errors() {
    let unconstrained = resolve_text(
        "class Box<T> { fn inspect(ref value: T) -> unit { value.inspect(); } }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(unconstrained
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == UNCONSTRAINED_TYPE_PARAMETER_MEMBER));

    let ambiguous = resolve_text(
        "interface Left { fn inspect() -> unit; }\n\
         interface Right { fn inspect() -> unit; }\n\
         class Box<T> where T: Left, T: Right {\n\
           fn use(ref value: T) -> unit { value.inspect(); }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(ambiguous
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == AMBIGUOUS_GENERIC_BOUND_MEMBER));
}

#[test]
fn construction_directly_through_a_parameter_is_rejected_in_both_modes() {
    let output = resolve_text(
        "class Factory<T> {\n\
           fn make() -> unit { T(); new T(); }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == UNSUPPORTED_PARAMETER_CONSTRUCTION)
            .count(),
        2,
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn body_type_positions_are_retained_as_argument_dependent_selections() {
    let output = resolve_text(
        "class View<T> {\n\
           fn inspect(ref value: T) -> bool {\n\
             var casted: T = (T) value;\n\
             return casted is T;\n\
           }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    assert!(
        dump.contains("TypeUse member0:cast-target template0:type0"),
        "{dump}"
    );
    assert!(
        dump.contains("TypeUse member0:type-test-target template0:type0"),
        "{dump}"
    );
    assert!(
        dump.contains("Selection argument-dependent cast template0:type0"),
        "{dump}"
    );
    assert!(
        dump.contains("Selection argument-dependent type-test template0:type0"),
        "{dump}"
    );
}

#[test]
fn lifecycle_static_and_remaining_body_type_positions_are_retained() {
    let output = resolve_text(
        "class Helper<T> { value: T; static fn make() -> unit {} }\n\
         class Complete<T> {\n\
           static cached: T?;\n\
           init(value: T) { var box: shared T? = new T?(); }\n\
           copy(ref source: Complete<T>) {}\n\
           assign(ref source: Complete<T>) {}\n\
           fn inspect(ref value: T) -> Helper<T> {\n\
             var values: T?[] = T?[]();\n\
             Helper<T>::make();\n\
             return Helper<T>(value);\n\
           }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    for expected in [
        "member0:static-field optional (template1:type0)",
        "member1:initializer-parameter0 template1:type0",
        "member2:copy-parameter0 template1<template1:type0>",
        "member3:assignment-parameter0 template1<template1:type0>",
        "member4:method-parameter0 template1:type0",
        "member4:method-result template0<template1:type0>",
        "member1:optional-box-target optional (template1:type0)",
        "member4:array-construction-target array (optional (template1:type0))",
        "member4:static-selection-target template0<template1:type0>",
    ] {
        assert!(dump.contains(expected), "missing `{expected}` in:\n{dump}");
    }
    assert!(
        dump.contains(
            "Selection argument-dependent static-member template0<template1:type0> member make"
        ),
        "{dump}"
    );
}

#[test]
fn definition_site_lookup_preserves_parameter_shadowing_and_qualified_identity() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import dep;\nclass Holder<T> { local: T; external: dep::T; }\nfn main() -> i64 { return 0; }\n",
            ),
            ("dep.ska", "public class T {}\n"),
        ],
    );
    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    assert!(
        dump.contains("TypeUse member0:field template0:type0"),
        "{dump}"
    );
    assert!(dump.contains("TypeUse member1:field class c0"), "{dump}");
}

#[test]
fn cyclic_module_templates_resolve_definition_site_names_without_instantiation() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            ("app.ska", "import a;\nfn main() -> i64 { return 0; }\n"),
            (
                "a.ska",
                "import b;\npublic class A<T> { value: b::B<T>; }\n",
            ),
            (
                "b.ska",
                "import a;\npublic class B<T> { value: a::A<T>; }\n",
            ),
        ],
    );
    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.program.class_templates.len(), 2);
    assert!(output.program.classes.is_empty());
    let dump = dump_resolved(&output.program);
    assert!(
        dump.contains("template0<template1:type0>") || dump.contains("template1<template0:type0>"),
        "{dump}"
    );
}

#[test]
fn nondependent_body_names_are_resolved_in_the_definition_module() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import dep;\nclass Box<T> { fn run() -> unit { dep::helper(); } }\nfn main() -> i64 { return 0; }\n",
            ),
            ("dep.ska", "public fn helper() -> unit {}\n"),
        ],
    );
    let output = resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    assert!(
        dump.contains("Selection definition-site top-level f1"),
        "{dump}"
    );

    let unknown = resolve_text(
        "class Box<T> { fn run() -> unit { missing(); } }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(unknown
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == UNKNOWN_NAME));
}

#[test]
fn template_semantic_dump_is_stable_across_source_registration_order() {
    fn semantic_dump(sources: &[(&str, &str)]) -> String {
        let (_workspace, graph) = load_module_sources("app", sources);
        let output = resolve_module_graph(&graph);
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let dump = dump_resolved(&output.program);
        let start = dump.find("  TemplateSemantics\n").unwrap();
        let end = dump[start..].find("  Entry ").unwrap() + start;
        dump[start..end].to_owned()
    }

    let app = "import model;\nclass Holder<T> { value: model::Pair<T>; }\nfn main() -> i64 { return 0; }\n";
    let model = "public class Pair<T> { value: T?[]; }\n";
    assert_eq!(
        semantic_dump(&[("model.ska", model), ("app.ska", app)]),
        semantic_dump(&[("app.ska", app), ("model.ska", model)])
    );
}
