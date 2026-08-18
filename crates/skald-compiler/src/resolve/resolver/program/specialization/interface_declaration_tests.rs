use super::*;
use crate::{
    identity::{InterfaceId, InterfaceRequirementId, InterfaceTemplateRequirementId},
    resolve::{dump_resolved, INVALID_GENERIC_INTERFACE_REQUIREMENT},
    test_support::resolve_source,
};

#[test]
fn materializes_complete_closed_signatures_and_requirement_mappings() {
    let output = resolve_source(
        "interface Plain {}\n\
         class Item { init() {} }\n\
         interface Nested<T> { fn inspect(ref value: T) -> unit; }\n\
         interface Complete<T> {\n\
           fn scalar(value: u64) -> bool;\n\
           fn item(value: T) -> T?;\n\
           fn owner(value: shared Plain) -> shared Plain;\n\
           fn callback(value: fn(T[]) -> T?) -> fn(ref T) -> unit;\n\
           fn nested(ref value: Nested<T>) -> unit;\n\
         }\n\
         fn use(ref value: Complete<Item>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let generated = output
        .program
        .interfaces
        .iter()
        .filter(|interface| interface.id.index() > 0)
        .collect::<Vec<_>>();
    assert_eq!(generated.len(), 2, "{}", dump_resolved(&output.program));
    let complete = generated
        .iter()
        .find(|interface| interface.name.starts_with("Complete<"))
        .unwrap();
    assert_eq!(complete.requirements.len(), 5);
    assert!(matches!(
        complete.requirements[1].parameters[0].type_syntax.kind,
        ResolvedTypeKind::Class(_)
    ));
    assert!(matches!(
        complete.requirements[1].return_type.kind,
        ResolvedTypeKind::Optional(_)
    ));
    assert!(matches!(
        complete.requirements[2].return_type.kind,
        ResolvedTypeKind::Shared(ResolvedSharedTarget::Interface(interface))
            if interface == InterfaceId::new(0)
    ));
    assert!(matches!(
        complete.requirements[3].parameters[0].type_syntax.kind,
        ResolvedTypeKind::Function(_)
    ));
    assert!(matches!(
        complete.requirements[4].parameters[0].type_syntax.kind,
        ResolvedTypeKind::Interface(_)
    ));

    let specialization = output
        .program
        .generic_interface_specializations
        .for_interface(complete.id)
        .unwrap();
    assert_eq!(specialization.requirement_mappings.len(), 5);
    for (index, mapping) in specialization.requirement_mappings.iter().enumerate() {
        assert_eq!(
            mapping.template,
            InterfaceTemplateRequirementId::new(specialization.key.template, index)
        );
        assert_eq!(
            mapping.closed,
            InterfaceRequirementId::new(complete.id, index)
        );
    }

    let checked = crate::typeck::type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    assert!(checked.hir.is_some());
}

#[test]
fn marker_accepts_an_unused_bare_interface_argument() {
    let output = resolve_source(
        "interface Plain {}\n\
         interface Marker<T> {}\n\
         fn use(ref value: Marker<Plain>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let marker = output.program.interface(InterfaceId::new(1)).unwrap();
    assert_eq!(marker.name, "Marker<Plain>");
    assert!(marker.requirements.is_empty());
}

#[test]
fn invalid_result_fails_the_complete_application_once_with_all_origins() {
    let output = resolve_source(
        "interface View {}\n\
         interface Producer<T> { fn produce() -> T; }\n\
         fn first(ref value: Producer<View>) -> unit {}\n\
         fn second(ref value: Producer<View>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INVALID_GENERIC_INTERFACE_REQUIREMENT)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(diagnostics[0]
        .message
        .contains("do not produce a valid interface signature"));
    let specialization = output
        .program
        .generic_interface_specializations
        .iter()
        .next()
        .unwrap();
    assert_eq!(specialization.provenance.origins.len(), 2);
    assert!(matches!(
        specialization.state,
        GenericInterfaceSpecializationState::Failed { .. }
    ));
    assert_eq!(output.program.interfaces.len(), 1);
}

#[test]
fn invalid_value_parameter_fails_the_complete_application() {
    let output = resolve_source(
        "interface View {}\n\
         interface Consumer<T> { fn consume(value: T) -> unit; }\n\
         fn use(ref value: Consumer<View>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == INVALID_GENERIC_INTERFACE_REQUIREMENT)
        .expect("invalid closed value parameter must be diagnosed");
    assert!(diagnostic
        .labels
        .iter()
        .any(|label| label.message.contains("non-storable value parameter")));
    assert_eq!(output.program.interfaces.len(), 1);
    assert!(matches!(
        output
            .program
            .generic_interface_specializations
            .iter()
            .next()
            .unwrap()
            .state,
        GenericInterfaceSpecializationState::Failed { .. }
    ));
}

#[test]
fn invalid_shared_target_and_nested_invalid_application_publish_no_suffix() {
    let output = resolve_source(
        "interface View {}\n\
         interface Bad<T> { fn produce() -> T; }\n\
         interface Outer<T> { fn inspect(ref value: Bad<T>) -> unit; }\n\
         interface Owner<T> { fn keep(value: shared T) -> unit; }\n\
         fn nested(ref value: Outer<View>) -> unit {}\n\
         fn owner(ref value: Owner<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_GENERIC_INTERFACE_REQUIREMENT));
    assert_eq!(output.program.interfaces.len(), 1);
    assert!(output.program.generic_interface_specializations.iter().all(
        |specialization| matches!(
            specialization.state,
            GenericInterfaceSpecializationState::Failed { .. }
        )
    ));
}

#[test]
fn resolved_dump_exposes_arguments_mappings_and_generated_requirements() {
    let output = resolve_source(
        "interface Source<T> { fn get() -> T; }\n\
         fn use(ref value: Source<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    let dump = dump_resolved(&output.program);

    for expected in [
        "TypeArgument interface-template0:type0 = i64",
        "RequirementMapping interface-template0:requirement0 -> i0:requirement0",
        "Interface i0 module m0 \"Source<i64>\"",
        "Requirement i0:requirement0 readonly \"get\"",
    ] {
        assert!(dump.contains(expected), "missing `{expected}` in:\n{dump}");
    }
    assert_eq!(dump, dump_resolved(&output.program));
}
