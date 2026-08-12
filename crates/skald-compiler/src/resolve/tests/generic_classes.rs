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
