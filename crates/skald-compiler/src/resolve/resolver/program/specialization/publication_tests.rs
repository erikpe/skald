//! Behavioral contracts for partial specialization publication.

use super::*;
use crate::{
    identity::ClassId,
    resolve::{dump_resolved, resolve_module_graph, INVALID_GENERIC_INTERFACE_REQUIREMENT},
    test_support::{load_module_sources, resolve_source},
};

#[test]
fn class_rejection_clears_populated_dispatch_and_bodies_but_retains_evidence() {
    let source = concat!(
        "class Base { virtual fn read() -> i64 { return 1; } }\n",
        "class Derived extends Base { override fn read() -> i64 { return 2; } }\n",
        "class Owner<T> { value: T; }\n",
        "fn inspect(ref value: Owner<i64>?[]) -> unit {}\n",
        "fn target(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { var callback: fn(i64) -> i64 = target; return 0; }\n",
    );
    let valid = resolve_source(source);
    assert!(valid.diagnostics.is_empty(), "{:?}", valid.diagnostics);
    assert!(!valid.program.virtual_families.is_empty());
    assert!(!valid.program.definitions.is_empty());
    assert!(!valid.program.class_definitions.is_empty());
    assert!(!valid.program.address_taken_callables.is_empty());
    let rejected = resolve_source(source.replace("value: T;", "value: shared T;"));
    assert_eq!(rejected.diagnostics.len(), 1, "{:?}", rejected.diagnostics);
    assert_eq!(
        rejected.diagnostics.iter().next().unwrap().code,
        UNSATISFIED_GENERIC_REQUIREMENT
    );
    let program = &rejected.program;
    assert_eq!(program.classes.len(), 2);
    assert_eq!(
        program.hierarchy.direct_base(ClassId::new(1)),
        Some(ClassId::new(0))
    );
    assert!(program.virtual_families.is_empty());
    assert!(program.definitions.is_empty());
    assert!(program.class_definitions.is_empty());
    assert_eq!(program.entry_function, valid.program.entry_function);
    assert_eq!(program.declarations.len(), valid.program.declarations.len());
    assert!(!program.function_types.is_empty());
    assert!(!program.array_types.is_empty());
    assert!(!program.optional_types.is_empty());
    let signature = program
        .declarations
        .iter()
        .find(|entry| entry.name == "inspect")
        .unwrap();
    let ResolvedTypeKind::Array(array) = signature.parameters[0].type_syntax.kind else {
        panic!("retained signature must preserve its array type");
    };
    let ResolvedTypeKind::Optional(optional) = program.array_types.get(array).unwrap().element.kind
    else {
        panic!("retained array must preserve its optional element");
    };
    let ResolvedTypeKind::Class(class) = program.optional_types.get(optional).unwrap().payload.kind
    else {
        panic!("retained optional must preserve its class identity");
    };
    assert!(
        program.class(class).is_none(),
        "rejected type identity is diagnostic evidence, not a published class"
    );
    // Materialization failure prevents body analysis; no address-taken facts
    // may be collected against missing generated declarations.
    assert!(program.address_taken_callables.is_empty());
    assert!(!dump_resolved(program).is_empty());
}

#[test]
fn contextual_class_rejection_rejects_dependent_interface_family_without_cascade() {
    let source = concat!(
        "class Owner<T> { value: shared T; }\n",
        "interface View<T> { fn read(ref value: T) -> unit; }\n",
        "fn use(ref bad: Owner<i64>, ref dependent: View<Owner<i64>>, ref independent: View<i64>) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = resolve_source(source);
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        UNSATISFIED_GENERIC_REQUIREMENT
    );
    assert_eq!(
        output
            .program
            .generic_interface_specializations
            .iter()
            .len(),
        2
    );
    assert!(output
        .program
        .generic_interface_specializations
        .iter()
        .all(|entry| matches!(
            entry.state,
            GenericInterfaceSpecializationState::Failed { .. }
        )));
    assert!(output.program.interfaces.is_empty());
    assert!(!dump_resolved(&output.program).is_empty());
}

#[test]
fn interface_rejection_preserves_class_dispatch_and_module_ordered_diagnostics() {
    let model = concat!(
        "public interface View {}\n",
        "public interface Consumer<T> { fn consume(value: T) -> unit; }\n",
        "public class Dynamic { virtual fn read() -> i64 { return 1; } }\n",
    );
    let app = concat!(
        "from model import View, Consumer, Dynamic;\n",
        "fn use(ref value: Consumer<View>) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let mut dumps = Vec::new();
    let mut diagnostics = Vec::new();
    for sources in [
        vec![("app.ska", app), ("model.ska", model)],
        vec![("model.ska", model), ("app.ska", app)],
    ] {
        let (_workspace, graph) = load_module_sources("app", &sources);
        let output = resolve_module_graph(&graph);
        assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
        assert_eq!(
            output.diagnostics.iter().next().unwrap().code,
            INVALID_GENERIC_INTERFACE_REQUIREMENT
        );
        assert!(!output.program.virtual_families.is_empty());
        assert!(!output.program.class_definitions.is_empty());
        assert!(!output.program.definitions.is_empty());
        dumps.push(dump_resolved(&output.program));
        diagnostics.push(format!("{:?}", output.diagnostics));
    }
    assert_eq!(dumps[0], dumps[1]);
    assert_eq!(diagnostics[0], diagnostics[1]);
}

#[test]
fn class_rejection_preserves_canonical_string_fields_and_literal_bytes() {
    let source = concat!(
        "class Owner<T> { value: shared T; }\n",
        "fn use(ref value: Owner<i64>) -> unit {}\n",
        "fn main() -> i64 { \"publication evidence\"; return 0; }\n",
    );
    let valid_source = source.replace("value: shared T;", "value: T;");
    let (_valid_workspace, valid_graph) =
        crate::test_support::load_module_sources_with_standard_library(
            "app",
            &[("app.ska", valid_source.as_str())],
        );
    let valid = resolve_module_graph(&valid_graph);
    assert!(valid.diagnostics.is_empty(), "{:?}", valid.diagnostics);
    assert!(valid.measurements.semantic_range_rounds() > 0);

    let (_workspace, graph) = crate::test_support::load_module_sources_with_standard_library(
        "app",
        &[("app.ska", source)],
    );
    let output = resolve_module_graph(&graph);
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        UNSATISFIED_GENERIC_REQUIREMENT
    );
    assert_eq!(output.measurements.semantic_range_rounds(), 0);
    assert_eq!(output.measurements.semantic_range_interner_copies(), 0);
    assert!(output.program.definitions.is_empty());
    assert!(output.program.class_definitions.is_empty());
    let string = output.program.string_language_item.as_ref().unwrap();
    let class = output.program.class(string.class).unwrap();
    for field in [
        string.storage_field,
        string.start_field,
        string.length_field,
        string.hash_code_field,
    ] {
        assert!(class.fields.iter().any(|entry| entry.id == field));
    }
    assert!(output
        .program
        .literal_data
        .iter()
        .any(|literal| literal.bytes == b"publication evidence"));
    assert!(!dump_resolved(&output.program).is_empty());
}

#[test]
fn rejected_interface_claims_remain_dumpable_partial_class_evidence() {
    let output = resolve_source(concat!(
        "interface View {}\n",
        "interface Consumer<T> { fn consume(value: T) -> unit; }\n",
        "class Implementation implements Consumer<View> {\n",
        "  fn consume(value: View) -> unit {}\n",
        "  virtual fn read() -> i64 { return 1; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        INVALID_GENERIC_INTERFACE_REQUIREMENT
    );
    let rejected = output
        .program
        .generic_interface_specializations
        .iter()
        .next()
        .unwrap();
    let interface = rejected.interface().unwrap();
    assert!(output.program.interface(interface).is_none());
    assert_eq!(output.program.classes.len(), 1);
    let class = output.program.classes.iter().next().unwrap();
    assert!(class
        .implemented_interfaces
        .iter()
        .any(|claim| { claim.interface == ResolvedInterfaceType::Ordinary(interface) }));
    assert!(!output.program.virtual_families.is_empty());
    assert!(!output.program.class_definitions.is_empty());
    assert!(!dump_resolved(&output.program).is_empty());
}

#[test]
fn dependent_rejection_diagnostics_and_products_ignore_module_creation_order() {
    let model = concat!(
        "public class Owner<T> { value: shared T; }\n",
        "public interface View<T> { fn read(ref value: T) -> unit; }\n",
    );
    let app = concat!(
        "from model import Owner, View;\n",
        "fn use(ref bad: Owner<i64>, ref dependent: View<Owner<i64>>, ref independent: View<i64>) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let mut evidence = Vec::new();
    for sources in [
        [("app.ska", app), ("model.ska", model)],
        [("model.ska", model), ("app.ska", app)],
    ] {
        let (_workspace, graph) = load_module_sources("app", &sources);
        let output = resolve_module_graph(&graph);
        assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
        assert_eq!(
            output.diagnostics.iter().next().unwrap().code,
            UNSATISFIED_GENERIC_REQUIREMENT
        );
        assert!(output.program.interfaces.is_empty());
        assert!(output.program.classes.is_empty());
        assert!(output
            .program
            .generic_interface_specializations
            .iter()
            .all(|entry| matches!(
                entry.state,
                GenericInterfaceSpecializationState::Failed { .. }
            )));
        evidence.push((
            dump_resolved(&output.program),
            format!("{:?}", output.diagnostics),
        ));
    }
    assert_eq!(evidence[0], evidence[1]);
}
