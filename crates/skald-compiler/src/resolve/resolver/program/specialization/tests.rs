use super::*;
use crate::{
    identity::{ClassId, ClassTemplateId, InterfaceId, InterfaceTemplateId},
    resolve::{
        dump_resolved, NON_TERMINATING_GENERIC_SPECIALIZATION, PRIVATE_DECLARATION, UNKNOWN_TYPE,
    },
    test_support::{load_module_sources, resolve_source},
};

#[test]
fn interface_grouping_and_optional_shared_shorthand_share_one_canonical_key() {
    let output = resolve_source(
        "class Item {}\n\
         interface View<T> {}\n\
         fn canonical(ref value: View<(shared Item)?>) -> unit {}\n\
         fn shorthand(ref value: View<shared? Item>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let entries = output
        .program
        .generic_interface_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1, "{}", dump_resolved(&output.program));
    assert_eq!(entries[0].key.template, InterfaceTemplateId::new(0));
    assert_eq!(entries[0].provenance.origins.len(), 2);
    assert_eq!(
        entries[0].state,
        GenericInterfaceSpecializationState::Complete(InterfaceId::new(0)),
    );
}

#[test]
fn interface_keys_distinguish_templates_argument_order_and_nested_closed_types() {
    let output = resolve_source(
        "class Item {}\n\
         interface One<T> {}\n\
         interface Two<T> {}\n\
         interface Pair<Left, Right> {}\n\
         fn use_types(ref one: One<i64>, ref two: Two<i64>, ref forward: Pair<i64, bool>, ref reverse: Pair<bool, i64>, ref owner: One<shared Item>, ref nested: Two<One<i64>>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let entries = output
        .program
        .generic_interface_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 6, "{}", dump_resolved(&output.program));
    assert_ne!(entries[0].key.template, entries[1].key.template);
    assert_eq!(entries[2].key.template, entries[3].key.template);
    assert_ne!(entries[2].key.arguments, entries[3].key.arguments);
    assert_ne!(entries[0].key.arguments, entries[4].key.arguments);
    assert!(entries.iter().all(|entry| matches!(
        entry.state,
        GenericInterfaceSpecializationState::Complete(_)
    )));
}

#[test]
fn identical_recursive_interface_requests_reuse_the_reserved_identity() {
    let output = resolve_source(
        "interface Node<T> { fn next() -> Node<T>; }\n\
         fn use(ref value: Node<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert_eq!(
        diagnostic_count(&output, NON_TERMINATING_GENERIC_SPECIALIZATION),
        0
    );
    let entry = output
        .program
        .generic_interface_specializations
        .iter()
        .next()
        .unwrap();
    assert_eq!(
        entry.state,
        GenericInterfaceSpecializationState::Complete(InterfaceId::new(0))
    );
    assert_eq!(
        entry.transitions,
        [
            GenericInterfaceSpecializationTransition::Requested,
            GenericInterfaceSpecializationTransition::InProgress(InterfaceId::new(0)),
            GenericInterfaceSpecializationTransition::Complete(InterfaceId::new(0)),
        ]
    );
    assert!(entry.provenance.recursion_path.is_empty());
}

#[test]
fn mutually_recursive_interfaces_complete_in_one_cross_kind_worklist() {
    let output = resolve_source(
        "interface Left<T> { fn right() -> Right<T>; }\n\
         interface Right<T> { fn left() -> Left<T>; }\n\
         fn use(ref value: Left<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert_eq!(
        diagnostic_count(&output, NON_TERMINATING_GENERIC_SPECIALIZATION),
        0
    );
    let entries = output
        .program
        .generic_interface_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2, "{}", dump_resolved(&output.program));
    assert!(entries.iter().all(|entry| matches!(
        entry.state,
        GenericInterfaceSpecializationState::Complete(_)
    )));
}

#[test]
fn transformed_interface_recursion_is_cached_and_diagnosed_once() {
    let output = resolve_source(
        "interface Expand<T> { fn next() -> Expand<T[]>; }\n\
         fn first(ref value: Expand<i64>) -> unit {}\n\
         fn second(ref value: Expand<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert_eq!(
        diagnostic_count(&output, NON_TERMINATING_GENERIC_SPECIALIZATION),
        1
    );
    let entries = output
        .program
        .generic_interface_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2, "{}", dump_resolved(&output.program));
    assert_eq!(entries[0].provenance.origins.len(), 2);
    assert!(entries.iter().all(|entry| matches!(
        entry.state,
        GenericInterfaceSpecializationState::Failed { .. }
    )));
    assert!(!entries[0].provenance.recursion_path.is_empty());
}

#[test]
fn mixed_interface_and_class_recursion_terminates_without_publishing_the_interface() {
    let output = resolve_source(
        "interface View<T> { fn box_value() -> Box<T>; }\n\
         class Box<T> { view: View<T>; }\n\
         fn use(ref value: View<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert_eq!(
        diagnostic_count(&output, NON_TERMINATING_GENERIC_SPECIALIZATION),
        0
    );
    let interface = output
        .program
        .generic_interface_specializations
        .iter()
        .next()
        .unwrap();
    assert!(matches!(
        interface.state,
        GenericInterfaceSpecializationState::Complete(_)
    ));
    let class = output
        .program
        .generic_specializations
        .iter()
        .next()
        .unwrap();
    assert!(matches!(
        class.state,
        GenericSpecializationState::Failed { .. }
    ));
    assert_eq!(output.program.interfaces.len(), 0);
}

#[test]
fn mixed_transformed_recursion_is_rejected_by_the_shared_active_path() {
    let output = resolve_source(
        "interface Grow<T> { fn box_value() -> Box<T>; }\n\
         class Box<T> { next: Grow<T[]>; }\n\
         fn use(ref value: Grow<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert_eq!(
        diagnostic_count(&output, NON_TERMINATING_GENERIC_SPECIALIZATION),
        1
    );
    assert!(output
        .program
        .generic_interface_specializations
        .iter()
        .all(|entry| matches!(
            entry.state,
            GenericInterfaceSpecializationState::Failed { .. }
        )));
    assert!(output
        .program
        .generic_specializations
        .iter()
        .all(|entry| matches!(entry.state, GenericSpecializationState::Failed { .. })));
}

#[test]
fn grouping_and_optional_shared_shorthand_share_one_canonical_key() {
    let output = resolve_source(
        "class Item {}\n\
         class Box<T> { value: T; }\n\
         fn canonical(value: Box<(shared Item)?>) -> unit {}\n\
         fn shorthand(value: Box<shared? Item>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let entries = output
        .program
        .generic_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key.template, ClassTemplateId::new(0));
    assert_eq!(entries[0].provenance.origins.len(), 2);
    assert_eq!(
        entries[0].state,
        GenericSpecializationState::Complete(ClassId::new(1))
    );
}

#[test]
fn nested_source_applications_close_inside_out_and_reuse_repeated_keys() {
    let output = resolve_source(
        "class Inner<T> { value: T; }\n\
         class Outer<T> { value: T; }\n\
         fn first(value: Outer<Inner<i64>>) -> unit {}\n\
         fn second(value: Outer<Inner<i64>>) -> unit {}\n\
         fn distinct(value: Outer<Inner<i64>?>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let entries = output
        .program
        .generic_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 3, "{}", dump_resolved(&output.program));
    assert_eq!(entries[0].key.template, ClassTemplateId::new(0));
    assert_eq!(
        entries[0].state,
        GenericSpecializationState::Complete(ClassId::new(0))
    );
    assert_eq!(entries[1].key.template, ClassTemplateId::new(1));
    assert_eq!(
        entries[1].state,
        GenericSpecializationState::Complete(ClassId::new(1))
    );
    assert_eq!(entries[0].provenance.origins.len(), 3);
    assert_eq!(entries[1].provenance.origins.len(), 2);
    assert_eq!(entries[2].key.template, ClassTemplateId::new(1));
    assert_eq!(
        entries[2].state,
        GenericSpecializationState::Complete(ClassId::new(2))
    );
}

#[test]
fn templates_argument_order_and_closed_type_shapes_remain_distinct() {
    let output = resolve_source(
        "interface View { fn inspect() -> unit; }\n\
         class Item {}\n\
         class One<T> { value: T; }\n\
         class Two<T> { value: T; }\n\
         class Pair<Left, Right> { left: Left; right: Right; }\n\
         fn shapes(\n\
           one: One<i64>,\n\
           two: Two<i64>,\n\
           forward: Pair<i64, bool>,\n\
           reverse: Pair<bool, i64>,\n\
           array: One<i64?[]>,\n\
           owner: One<shared Item>,\n\
           view: One<shared View>\n\
         ) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let entries = output
        .program
        .generic_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 7, "{}", dump_resolved(&output.program));
    assert_ne!(entries[0].key.template, entries[1].key.template);
    assert_eq!(entries[2].key.template, entries[3].key.template);
    assert_ne!(entries[2].key.arguments, entries[3].key.arguments);
    assert!(entries
        .iter()
        .all(|entry| matches!(entry.state, GenericSpecializationState::Complete(_))));
}

#[test]
fn identical_recursive_requests_reuse_the_in_progress_class() {
    let output = resolve_source(
        "class Node<T> { next: shared Node<T>; }\n\
         fn use(value: Node<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert_eq!(
        diagnostic_count(&output, NON_TERMINATING_GENERIC_SPECIALIZATION),
        0
    );
    let specialization = output
        .program
        .generic_specializations
        .iter()
        .next()
        .unwrap();
    assert_eq!(
        specialization.state,
        GenericSpecializationState::Complete(ClassId::new(0))
    );
    assert_eq!(
        specialization.transitions,
        [
            GenericSpecializationTransition::Requested,
            GenericSpecializationTransition::InProgress(ClassId::new(0)),
            GenericSpecializationTransition::Complete(ClassId::new(0)),
        ]
    );
    assert!(specialization.provenance.recursion_path.is_empty());
    assert_eq!(output.program.classes.len(), 1);
    assert_eq!(
        output.program.classes.get(ClassId::new(0)).unwrap().name,
        "Node<i64>"
    );
}

#[test]
fn contextual_requirement_failures_reject_declaration_publication_after_identity_discovery() {
    let output = resolve_source(
        "class Nested<T> { value: T; }\n\
         class Owner<T> { invalid_for_i64: shared T; nested: Nested<T>; }\n\
         fn use(value: Owner<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert_eq!(
        diagnostic_count(&output, NON_TERMINATING_GENERIC_SPECIALIZATION),
        0
    );
    let entries = output
        .program
        .generic_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2, "{}", dump_resolved(&output.program));
    assert!(entries
        .iter()
        .any(|entry| matches!(entry.state, GenericSpecializationState::Failed { .. })));
    assert_eq!(
        diagnostic_count(
            &output,
            super::super::super::UNSATISFIED_GENERIC_REQUIREMENT
        ),
        1
    );
    assert!(output.program.classes.is_empty());
}

#[test]
fn repeated_failed_keys_emit_once_and_restore_coherent_specialization_products() {
    let output = resolve_source(
        "class Plain { init() {} }\n\
         class Owner<T> { invalid: shared T; }\n\
         fn first(ref value: Owner<i64>) -> unit {}\n\
         fn second(ref value: Owner<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let failures = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == UNSATISFIED_GENERIC_REQUIREMENT)
        .collect::<Vec<_>>();
    assert_eq!(failures.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(
        failures[0]
            .labels
            .iter()
            .filter(|label| label.message.contains("also used here"))
            .count(),
        1
    );
    assert!(output
        .program
        .generic_specializations
        .iter()
        .all(|entry| matches!(entry.state, GenericSpecializationState::Failed { .. })));
    assert_eq!(output.program.classes.len(), 1);
    assert_eq!(
        output.program.classes.get(ClassId::new(0)).unwrap().name,
        "Plain"
    );
    assert!(output.program.class_definitions.is_empty());
    assert!(output.program.definitions.is_empty());
    assert!(output.program.virtual_families.is_empty());
}

#[test]
fn lifecycle_requirement_diagnostics_include_the_existing_field_path() {
    let output = resolve_source(
        "class Node {\n\
           next: Node;\n\
           init(ref next: Node) { self.next = next; }\n\
         }\n\
         class Box<T> {\n\
           value: T;\n\
           init(ref value: T) { self.value = value; }\n\
         }\n\
         fn use(ref value: Box<Node>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let failure = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == UNSATISFIED_GENERIC_REQUIREMENT)
        .expect("copying a recursively stored argument must fail its inferred contract");
    assert!(failure
        .labels
        .iter()
        .any(|label| label.message.contains("lifecycle path enters this field")));
    assert!(failure
        .notes
        .iter()
        .any(|note| note.contains("field `next`")));
}

#[test]
fn substitution_preserves_optional_layers_and_shared_arguments_literally() {
    let output = resolve_source(
        "class Item {}\n\
         class Vec<T> { storage: T?[]; }\n\
         fn exact(value: Vec<Item>) -> unit {}\n\
         fn optional(value: Vec<Item?>) -> unit {}\n\
         fn owner(value: Vec<shared Item>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert_eq!(output.program.generic_specializations.iter().len(), 3);
    let optionals = output.program.optional_types.iter().collect::<Vec<_>>();
    assert_eq!(optionals.len(), 3);
    assert_eq!(
        optionals[0].payload.kind,
        ResolvedTypeKind::Class(ClassId::new(0))
    );
    assert_eq!(
        optionals[1].payload.kind,
        ResolvedTypeKind::Optional(optionals[0].id)
    );
    assert!(matches!(
        optionals[2].payload.kind,
        ResolvedTypeKind::Shared(_)
    ));

    let arrays = output.program.array_types.iter().collect::<Vec<_>>();
    assert_eq!(arrays.len(), 3);
    for (array, optional) in arrays.iter().zip(optionals) {
        assert_eq!(array.element.kind, ResolvedTypeKind::Optional(optional.id));
    }
}

#[test]
fn transformed_recursion_is_failed_once_and_later_uses_reuse_the_failure() {
    let output = resolve_source(
        "class Expand<T> { next: shared Expand<T[]>; }\n\
         fn first(value: Expand<i64>) -> unit {}\n\
         fn second(value: Expand<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert_eq!(
        diagnostic_count(&output, NON_TERMINATING_GENERIC_SPECIALIZATION),
        1
    );
    let entries = output
        .program
        .generic_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].state,
        GenericSpecializationState::Failed {
            reserved_class: Some(ClassId::new(0)),
        }
    );
    assert_eq!(entries[0].provenance.origins.len(), 2);
    assert_eq!(entries[0].provenance.recursion_path.len(), 2);
    assert!(output.program.classes.is_empty());
}

#[test]
fn transformed_edges_do_not_poison_a_separately_finite_key() {
    fn states(source: &str) -> (GenericSpecializationState, GenericSpecializationState) {
        let output = resolve_source(source);
        assert_eq!(
            diagnostic_count(&output, NON_TERMINATING_GENERIC_SPECIALIZATION),
            1
        );
        let mut i64_state = None;
        let mut bool_state = None;
        for specialization in output.program.generic_specializations.iter() {
            match specialization.key.arguments.as_slice() {
                [ResolvedTypeKind::I64] => i64_state = Some(specialization.state),
                [ResolvedTypeKind::Bool] => bool_state = Some(specialization.state),
                arguments => panic!("unexpected specialization arguments: {arguments:?}"),
            }
        }
        (i64_state.unwrap(), bool_state.unwrap())
    }

    let finite_first = states(
        "class Switch<T> { next: shared Switch<i64>; }\n\
         fn finite(value: Switch<i64>) -> unit {}\n\
         fn transformed(value: Switch<bool>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    let transformed_first = states(
        "class Switch<T> { next: shared Switch<i64>; }\n\
         fn transformed(value: Switch<bool>) -> unit {}\n\
         fn finite(value: Switch<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(matches!(
        finite_first.0,
        GenericSpecializationState::Complete(_)
    ));
    assert!(matches!(
        finite_first.1,
        GenericSpecializationState::Failed { .. }
    ));
    assert!(matches!(
        transformed_first.0,
        GenericSpecializationState::Complete(_)
    ));
    assert!(matches!(
        transformed_first.1,
        GenericSpecializationState::Failed { .. }
    ));
}

#[test]
fn cross_module_reuse_and_source_permutation_have_identical_dumps() {
    let app = "from dep import Box;\nfn first(value: Box<i64>) -> unit {}\nfn second(value: Box<i64>) -> unit {}\nfn main() -> i64 { return 0; }\n";
    let dep = "public class Box<T> { value: T; }\n";

    let first = resolve_modules(&[("dep.ska", dep), ("app.ska", app)]);
    let second = resolve_modules(&[("app.ska", app), ("dep.ska", dep)]);

    assert_eq!(
        dump_resolved(&first.program),
        dump_resolved(&second.program)
    );
    let specialization = first.program.generic_specializations.iter().next().unwrap();
    assert_eq!(specialization.provenance.origins.len(), 2);
    assert_eq!(
        specialization.state,
        GenericSpecializationState::Complete(ClassId::new(0))
    );
}

#[test]
fn application_arguments_obey_module_visibility_before_requesting_a_key() {
    let app = "import dep;\nfn good(value: dep::Box<dep::Visible>) -> unit {}\nfn bad(value: dep::Box<dep::Hidden>) -> unit {}\nfn main() -> i64 { return 0; }\n";
    let dep = "public class Visible {}\nclass Hidden {}\npublic class Box<T> { value: T; }\n";
    let output = resolve_modules(&[("app.ska", app), ("dep.ska", dep)]);

    assert_eq!(diagnostic_count(&output, PRIVATE_DECLARATION), 1);
    let entries = output
        .program
        .generic_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1, "{}", dump_resolved(&output.program));
    assert!(matches!(
        entries[0].state,
        GenericSpecializationState::Complete(_)
    ));
}

#[test]
fn invalid_closed_argument_spellings_do_not_create_specialization_keys() {
    let output = resolve_source(
        "class Box<T> { value: T; }\n\
         fn bad(value: Box<shared i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert_eq!(diagnostic_count(&output, UNKNOWN_TYPE), 1);
    assert!(output.program.generic_specializations.is_empty());
}

#[test]
fn dump_exposes_stable_keys_transitions_origins_and_recursion_paths() {
    let output = resolve_source(
        "class Expand<T> { next: shared Expand<T[]>; }\n\
         fn use(value: Expand<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    for fragment in [
        "GenericSpecializations",
        "Specialization Expand<i64> class c0 state failed c0",
        "Transition requested",
        "Transition in-progress c0",
        "Origin module m0",
        "RecursionPath",
        "Expand<i64[]>",
    ] {
        assert!(dump.contains(fragment), "missing `{fragment}` in:\n{dump}");
    }
}

#[test]
fn cyclic_module_applications_use_canonical_names_and_stable_graph_order() {
    let first = [
        (
            "app.ska",
            "from left import Box;\n\
             from right import Item as RightItem, Wrap;\n\
             fn accept(ref direct: Box<RightItem>, ref nested: Wrap<RightItem>) -> unit {}\n\
             fn main() -> i64 { return 0; }\n",
        ),
        (
            "left.ska",
            "import right;\n\
             public class Item {}\n\
             public class Box<T> { value: T; marker: right::Marker; }\n",
        ),
        (
            "right.ska",
            "import left;\n\
             public class Marker {}\n\
             public class Item {}\n\
             public class Wrap<T> { value: left::Box<T>; }\n",
        ),
    ];
    let second = [first[2], first[0], first[1]];

    let first = resolve_modules(&first);
    let second = resolve_modules(&second);
    assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
    assert!(second.diagnostics.is_empty(), "{:?}", second.diagnostics);

    let first_names = first
        .program
        .classes
        .iter()
        .map(|class| class.name.as_str())
        .filter(|name| name.contains('<'))
        .collect::<Vec<_>>();
    let second_names = second
        .program
        .classes
        .iter()
        .map(|class| class.name.as_str())
        .filter(|name| name.contains('<'))
        .collect::<Vec<_>>();
    assert_eq!(first_names, second_names);
    assert!(first_names.contains(&"left::Box<right::Item>"));
    assert!(first_names.contains(&"right::Wrap<right::Item>"));

    assert_eq!(
        dump_resolved(&first.program),
        dump_resolved(&second.program)
    );
}

#[test]
fn interface_aliases_and_qualification_reuse_one_cross_module_key() {
    let first = [
        (
            "app.ska",
            "from dep import View as Alias, Item;\n\
             import dep;\n\
             fn first(ref value: Alias<Item>) -> unit {}\n\
             fn second(ref value: dep::View<dep::Item>) -> unit {}\n\
             fn main() -> i64 { return 0; }\n",
        ),
        (
            "dep.ska",
            "public class Item {}\n\
             public interface View<T> {}\n",
        ),
    ];
    let second = [first[1], first[0]];

    let first = resolve_modules(&first);
    let second = resolve_modules(&second);
    let first_entries = first
        .program
        .generic_interface_specializations
        .iter()
        .collect::<Vec<_>>();
    let second_entries = second
        .program
        .generic_interface_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(first_entries.len(), 1, "{}", dump_resolved(&first.program));
    assert_eq!(first_entries[0].provenance.origins.len(), 2);
    assert_eq!(first_entries, second_entries);
    assert_eq!(
        dump_resolved(&first.program),
        dump_resolved(&second.program)
    );
}

fn resolve_modules(sources: &[(&str, &str)]) -> crate::resolve::ResolveOutput {
    let (_workspace, graph) = load_module_sources("app", sources);
    crate::resolve::resolve_module_graph(&graph)
}

fn diagnostic_count(output: &crate::resolve::ResolveOutput, code: &str) -> usize {
    output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == code)
        .count()
}
