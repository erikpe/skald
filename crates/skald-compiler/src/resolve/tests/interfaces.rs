use super::*;
use crate::{
    identity::{
        GenericTemplateId, InterfaceId, InterfaceRequirementId, InterfaceTemplateId,
        InterfaceTemplateRequirementId, TypeParameterId,
    },
    resolve::{
        GenericCapability, GenericRequirementReason, ResolvedParameterBindingMode,
        ResolvedTemplateTypeKind,
    },
    test_support::load_module_sources,
};

#[test]
fn assigns_stable_interface_and_requirement_identities() {
    let output = resolve_text(
        "interface First { fn run(value: u64) -> unit; }\n\
         interface Second { mut fn stop() -> bool; }\n\
         class Worker implements Second, First {}",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let first = output.program.interface(InterfaceId::new(0)).unwrap();
    assert_eq!(
        first.requirements[0].id,
        InterfaceRequirementId::new(first.id, 0)
    );
    let claims = &output
        .program
        .class(ClassId::new(0))
        .unwrap()
        .implemented_interfaces;
    assert_eq!(
        claims
            .iter()
            .map(|claim| claim.interface)
            .collect::<Vec<_>>(),
        [InterfaceId::new(1), InterfaceId::new(0)]
    );
}

#[test]
fn rejects_duplicate_unknown_and_wrong_kind_claims_in_source_order() {
    let output = resolve_text(
        "interface Known {}\nfn helper() -> unit {}\n\
         class Broken implements Known, Known, Missing, helper {}",
    );
    let messages = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        [
            "duplicate interface `Known`",
            "unknown interface `Missing`",
            "`helper` does not name an interface",
        ]
    );
}

#[test]
fn rejects_interface_construction_without_claiming_interface_calls_are_unavailable() {
    let output = resolve_text(
        "interface Readable { fn read() -> i64; }\n\
         fn main() -> i64 { Readable(); return 0; }\n",
    );
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.message, "interface `Readable` is not callable");
    assert_eq!(
        diagnostic.labels[0].message,
        "interfaces describe non-owning views and cannot be constructed"
    );
}

#[test]
fn generic_interface_declarations_receive_stable_non_executable_identities() {
    let output = resolve_text(concat!(
        "interface Plain {}\n",
        "interface Marker {}\n",
        "interface Producer<T, Item> where T: Marker {\n",
        "  fn produce() -> T;\n",
        "  fn item() -> Item;\n",
        "}\n",
        "interface Other { fn run() -> unit; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(
        output.program.interface(InterfaceId::new(0)).unwrap().name,
        "Plain"
    );
    assert_eq!(
        output.program.interface(InterfaceId::new(1)).unwrap().name,
        "Marker"
    );
    assert_eq!(
        output.program.interface(InterfaceId::new(2)).unwrap().name,
        "Other"
    );
    assert!(output.program.interface(InterfaceId::new(3)).is_none());

    let template = output
        .program
        .interface_templates
        .get(InterfaceTemplateId::new(0))
        .unwrap();
    assert_eq!(template.name, "Producer");
    assert_eq!(
        template
            .requirements()
            .map(|requirement| (requirement.id, requirement.name.as_str()))
            .collect::<Vec<_>>(),
        [
            (
                InterfaceTemplateRequirementId::new(template.id, 0),
                "produce"
            ),
            (InterfaceTemplateRequirementId::new(template.id, 1), "item"),
        ]
    );
    let parameters = output
        .program
        .type_parameters
        .for_interface_template(template.id)
        .unwrap();
    assert_eq!(
        parameters
            .iter()
            .map(|parameter| (parameter.id, parameter.name.as_str()))
            .collect::<Vec<_>>(),
        [
            (TypeParameterId::new(template.id, 0), "T"),
            (TypeParameterId::new(template.id, 1), "Item"),
        ]
    );
    assert_eq!(
        parameters.iter().next().unwrap().id.owner(),
        GenericTemplateId::Interface(template.id)
    );
    assert_eq!(
        output
            .program
            .interface_templates
            .requirement(InterfaceTemplateRequirementId::new(template.id, 1))
            .unwrap()
            .name,
        "item"
    );
}

#[test]
fn resolves_complete_generic_interface_signature_trees_and_modes() {
    let output = resolve_text(concat!(
        "class Box<T> {}\n",
        "interface Inner<T> {}\n",
        "interface Complex<T, U> where T: Inner<U> {\n",
        "  fn all(value: i64, ref borrowed: T, mut ref maybe: T?) -> fn(ref U[], mut ref Box<T>) -> shared T?;\n",
        "  fn nested(value: Box<Inner<T>>) -> Inner<Box<U>>;\n",
        "}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert!(output.program.function_types.is_empty());
    assert!(output.program.array_types.is_empty());
    assert!(output.program.optional_types.is_empty());

    let semantics = output
        .program
        .interface_template_semantics
        .get(InterfaceTemplateId::new(1))
        .unwrap();
    assert_eq!(semantics.bounds.len(), 1);
    assert!(matches!(
        semantics.bounds[0].interface.kind,
        ResolvedTemplateTypeKind::InterfaceTemplate { .. }
    ));
    let all = &semantics.requirements[0];
    assert_eq!(
        all.parameters[0].binding_mode,
        ResolvedParameterBindingMode::Value
    );
    assert!(matches!(
        all.parameters[1].binding_mode,
        ResolvedParameterBindingMode::ReadOnlyAlias { .. }
    ));
    assert!(matches!(
        all.parameters[2].binding_mode,
        ResolvedParameterBindingMode::MutableAlias { .. }
    ));
    assert!(matches!(
        all.return_type.kind,
        ResolvedTemplateTypeKind::Function { .. }
    ));
    let nested = &semantics.requirements[1];
    assert!(matches!(
        nested.parameters[0].type_syntax.kind,
        ResolvedTemplateTypeKind::ClassTemplate { .. }
    ));
    assert!(matches!(
        nested.return_type.kind,
        ResolvedTemplateTypeKind::InterfaceTemplate { .. }
    ));
    assert!(semantics.contextual_requirements.iter().any(|requirement| {
        requirement.capability == GenericCapability::ValueResult
            && matches!(
                requirement.reason,
                GenericRequirementReason::InterfaceResult { .. }
            )
    }));
}

#[test]
fn validates_definition_independent_interface_signature_errors_once() {
    let output = resolve_text(concat!(
        "interface Plain {}\n",
        "interface Broken<T> {\n",
        "  fn bad(value: Plain) -> Plain;\n",
        "  fn observe(ref view: Plain) -> unit;\n",
        "  fn arrays(values: T[]) -> unit;\n",
        "}\n",
    ));
    let diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INVALID_GENERIC_INTERFACE_REQUIREMENT)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 3, "{:?}", output.diagnostics);
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.contains("invalid closed type"))
            .count(),
        2
    );
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.contains("array types"))
            .count(),
        1
    );
}

#[test]
fn retains_duplicate_requirement_identities_and_marker_parameters() {
    let output = resolve_text(
        "interface Marker<T> { fn same(value: T, value: T) -> unit; fn same() -> unit; }",
    );
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_GENERIC_INTERFACE_REQUIREMENT)
            .count(),
        2
    );
    let semantics = output
        .program
        .interface_template_semantics
        .get(InterfaceTemplateId::new(0))
        .unwrap();
    assert_eq!(semantics.requirements.len(), 2);
    assert_eq!(
        semantics.requirements[1].id,
        InterfaceTemplateRequirementId::new(InterfaceTemplateId::new(0), 1)
    );
    assert_eq!(semantics.contextual_requirements.len(), 2);

    let marker = resolve_text("interface Marker<T> {}");
    assert!(marker
        .program
        .interface_template_semantics
        .get(InterfaceTemplateId::new(0))
        .unwrap()
        .contextual_requirements
        .is_empty());
}

#[test]
fn resolves_interface_signature_names_in_the_definition_module() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "from dep import Source;\nclass Item {}\nfn main() -> i64 { return 0; }\n",
            ),
            (
                "dep.ska",
                "public class Item {}\npublic interface Source<T> { fn take(value: Item) -> T; }\n",
            ),
        ],
    );
    let output = resolve_module_graph(&graph);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let template = output
        .program
        .interface_templates
        .iter()
        .find(|template| template.name == "Source")
        .unwrap();
    let semantics = output
        .program
        .interface_template_semantics
        .get(template.id)
        .unwrap();
    let ResolvedTemplateTypeKind::Class(item) =
        semantics.requirements[0].parameters[0].type_syntax.kind
    else {
        panic!("definition-site Item must resolve to an ordinary class")
    };
    assert_eq!(output.program.class(item).unwrap().module, template.module);
}

#[test]
fn resolves_qualified_interface_signature_names_across_cyclic_modules() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import dep;\npublic class Host {}\npublic interface AppView<T> { fn node(ref value: dep::Node) -> T; }\nfn main() -> i64 { return 0; }\n",
            ),
            (
                "dep.ska",
                "import app;\npublic class Node {}\npublic interface DepView<T> { fn host(ref value: app::Host) -> T; }\n",
            ),
        ],
    );
    let output = resolve_module_graph(&graph);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    for semantics in output.program.interface_template_semantics.iter() {
        assert!(matches!(
            semantics.requirements[0].parameters[0].type_syntax.kind,
            ResolvedTemplateTypeKind::Class(_)
        ));
    }
}

#[test]
fn diagnoses_raw_and_wrong_arity_interface_terms_without_losing_requirements() {
    let output = resolve_text(concat!(
        "interface Inner<T> {}\n",
        "interface Broken<T> {\n",
        "  fn raw(value: Inner) -> unit;\n",
        "  fn arity(value: Inner<T, T>) -> unit;\n",
        "}\n",
    ));
    let codes = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    assert_eq!(codes, [RAW_GENERIC_TYPE, GENERIC_ARITY_MISMATCH]);
    let semantics = output
        .program
        .interface_template_semantics
        .get(InterfaceTemplateId::new(1))
        .unwrap();
    assert_eq!(semantics.requirements.len(), 2);
    assert_eq!(
        semantics.requirements[1].id,
        InterfaceTemplateRequirementId::new(InterfaceTemplateId::new(1), 1)
    );
}

#[test]
fn duplicate_interface_type_parameters_preserve_dense_owner_scoped_lookup() {
    let output = resolve_text("interface Pair<T, T> { fn first() -> T; }");

    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == DUPLICATE_TYPE_PARAMETER)
            .count(),
        1
    );
    let template = InterfaceTemplateId::new(0);
    let parameters = output
        .program
        .type_parameters
        .for_interface_template(template)
        .unwrap();
    assert_eq!(parameters.len(), 2);
    assert_eq!(
        output
            .program
            .type_parameters
            .get(TypeParameterId::new(template, 1))
            .unwrap()
            .name,
        "T"
    );
    assert!(output
        .program
        .type_parameters
        .get(TypeParameterId::new(InterfaceTemplateId::new(1), 0))
        .is_none());
}

#[test]
fn interface_template_module_declarations_and_imports_preserve_their_kind() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "from dep import Public as Imported;\nimport dep;\nfn raw(value: Imported) -> unit {}\nfn qualified(value: dep::Public) -> unit {}\nfn hidden(value: dep::Hidden) -> unit {}\nfn main() -> i64 { return 0; }\n",
            ),
            (
                "dep.ska",
                "public interface Public<T> { fn get() -> T; }\ninterface Hidden<T> {}\n",
            ),
        ],
    );

    let output = resolve_module_graph(&graph);
    let public = output
        .program
        .interface_templates
        .iter()
        .find(|template| template.name == "Public")
        .unwrap();
    let binding = output
        .program
        .ordinary_bindings
        .iter()
        .flat_map(|bindings| bindings.iter())
        .find(|binding| binding.local_name == "Imported")
        .unwrap();
    assert_eq!(
        binding.target,
        ResolvedTopLevelId::InterfaceTemplate(public.id)
    );
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == RAW_GENERIC_TYPE)
            .count(),
        2
    );
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == PRIVATE_DECLARATION)
            .count(),
        1
    );
}

#[test]
fn interface_template_ids_follow_canonical_module_and_declaration_order() {
    fn identities(sources: &[(&str, &str)]) -> Vec<(usize, usize, String)> {
        let (_workspace, graph) = load_module_sources("app", sources);
        resolve_module_graph(&graph)
            .program
            .interface_templates
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

    let app = "import a;\nimport z;\ninterface App<T> {}\nfn main() -> i64 { return 0; }\n";
    let a = "public interface First<T> {}\npublic interface Second<T> {}\n";
    let z = "public interface Last<T> {}\n";
    let first = identities(&[("z.ska", z), ("app.ska", app), ("a.ska", a)]);
    let second = identities(&[("a.ska", a), ("z.ska", z), ("app.ska", app)]);
    assert_eq!(first, second);
}

#[test]
fn interface_templates_participate_in_cross_kind_top_level_collisions() {
    let output = resolve_text(concat!(
        "interface Shared<T> {}\n",
        "class Shared {}\n",
        "class Other<T> {}\n",
        "interface Other<U> {}\n",
    ));

    let collisions = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DUPLICATE_TOP_LEVEL)
        .collect::<Vec<_>>();
    assert_eq!(collisions.len(), 2, "{:?}", output.diagnostics);
    assert_eq!(output.program.interface_templates.len(), 1);
    assert_eq!(output.program.class_templates.len(), 1);
    assert!(output.program.classes.is_empty());
    assert!(output.program.interfaces.iter().next().is_none());
}

#[test]
fn resolved_dump_exposes_interface_template_parameter_and_requirement_identities() {
    let output =
        resolve_text("interface Pair<Left, Right> { fn first() -> Left; fn second() -> Right; }");

    let dump = dump_resolved(&output.program);
    assert!(
        dump.contains("private interface-template0 \"Pair\""),
        "{dump}"
    );
    assert!(dump.contains(
        "Template interface-template0 module m0 \"Pair\" parameters interface-template0:type0=\"Left\" interface-template0:type1=\"Right\""
    ), "{dump}");
    assert!(
        dump.contains("Requirement interface-template0:requirement0 \"first\""),
        "{dump}"
    );
    assert!(
        dump.contains("Requirement interface-template0:requirement1 \"second\""),
        "{dump}"
    );
    assert!(dump.contains("InterfaceTemplateSemantics"), "{dump}");
    assert!(
        dump.contains("TypeUse interface-template0:requirement0:result interface-template0:type0"),
        "{dump}"
    );
    assert!(dump.contains(
        "ContextualRequirement value-result interface-template0:type0 reason interface-template0:requirement0:result"
    ), "{dump}");
}

#[test]
fn generic_interface_claims_and_bounds_are_gated_without_name_only_resolution() {
    let output = resolve_text(concat!(
        "interface Plain {}\n",
        "class Ordinary implements Plain<i64> {}\n",
        "class Generic<T> implements Plain<T> where T: Plain<T> {}\n",
    ));

    let diagnostics = output.diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 3, "{:?}", output.diagnostics);
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == UNSUPPORTED_GENERIC_INTERFACE));
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.message == "generic interface application `Plain` is not yet supported"
    }));
    assert!(output
        .program
        .class(ClassId::new(0))
        .unwrap()
        .implemented_interfaces
        .is_empty());
}
