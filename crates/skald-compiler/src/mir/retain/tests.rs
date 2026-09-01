//! Focused atomic executable-definition retention tests.

use crate::{
    identity::{CallableId, ClassId},
    passes::{verify_final_mir, VerifiedFinalMirProgram},
    test_support::lower_generic_source_to_final_mir,
};

use super::{
    prepare, prepare_reachable_definition_retention, MirDefinitionRetention,
    MirDefinitionRetentionError,
};
use crate::mir::MirProgram;

fn class(program: &MirProgram, name: &str) -> ClassId {
    program
        .classes
        .iter()
        .find(|class| class.name == name)
        .unwrap_or_else(|| panic!("missing class `{name}`"))
        .id
}

fn function(program: &MirProgram, name: &str) -> CallableId {
    program
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .unwrap_or_else(|| panic!("missing function `{name}`"))
        .id
        .into()
}

fn prepare_verified(verified: &VerifiedFinalMirProgram) -> MirDefinitionRetention {
    prepare_reachable_definition_retention(verified.program(), verified.reachability()).unwrap()
}

fn apply_changed(
    verified: &VerifiedFinalMirProgram,
) -> (MirProgram, super::MirDefinitionRetentionSummary) {
    let MirDefinitionRetention::Changed(prepared) = prepare_verified(verified) else {
        panic!("fixture must contain unreachable retained definitions")
    };
    let change = prepared.apply(verified.program().clone());
    (change.program, change.summary)
}

#[test]
fn removes_unreachable_functions_and_every_member_kind_in_canonical_order() {
    let verified = verify_final_mir(lower_generic_source_to_final_mir(
        "class Dormant {
           value: i64;
           init(value: i64) { self.value = value; }
           copy(ref other: Dormant) { self.value = other.value; }
           assign(ref other: Dormant) { self.value = other.value; }
           destroy {}
           fn read() -> i64 { return self.value; }
           static fn answer() -> i64 { return 42; }
         }
         fn dead() -> i64 { return 9; }
         fn main() -> i64 { return 0; }",
    ))
    .unwrap();
    let program = verified.program();
    let dormant = program.class(class(program, "Dormant")).unwrap();
    let expected_removed = [
        function(program, "dead"),
        dormant.initializers[0].id.into(),
        dormant
            .copy_constructor_declaration
            .as_ref()
            .unwrap()
            .id
            .into(),
        dormant
            .copy_assignment_declaration
            .as_ref()
            .unwrap()
            .id
            .into(),
        dormant.destruction.destructor.as_ref().unwrap().id.into(),
        dormant.methods[0].id.into(),
        dormant.methods[1].id.into(),
    ];

    let (retained_program, summary) = apply_changed(&verified);

    assert_eq!(summary.removed_callables(), expected_removed);
    assert_eq!(summary.examined().functions(), 2);
    assert_eq!(summary.examined().initializers(), 1);
    assert_eq!(summary.examined().copy_constructors(), 1);
    assert_eq!(summary.examined().copy_assignments(), 1);
    assert_eq!(summary.examined().destructors(), 1);
    assert_eq!(summary.examined().methods(), 2);
    assert_eq!(summary.examined().static_initializers(), 0);
    assert_eq!(summary.examined().total(), 8);
    assert_eq!(summary.retained().total(), 1);
    assert_eq!(summary.removed().total(), 7);
    assert_eq!(retained_program.definitions.len(), 1);
    assert!(retained_program.member_definitions.is_empty());
    verify_final_mir(retained_program).expect("retained output must reseal");
}

#[test]
fn preserves_global_metadata_static_authority_and_retained_bodies_exactly() {
    let verified = verify_final_mir(lower_generic_source_to_final_mir(
        "class State { static value: i64 = 7; init() {} }
         fn used() -> i64 { return State.value; }
         fn dead() -> i64 { return 9; }
         fn main() -> i64 { return used(); }",
    ))
    .unwrap();
    let before = verified.program().clone();
    let used = function(&before, "used").as_function().unwrap();
    let main = before.entry_function;
    let coordinator = before.static_lifecycle.clone().unwrap();

    let (after, summary) = apply_changed(&verified);

    assert_eq!(summary.examined().static_initializers(), 1);
    assert_eq!(summary.retained().static_initializers(), 1);
    assert_eq!(summary.removed().static_initializers(), 0);
    assert_eq!(after.modules, before.modules);
    assert_eq!(after.external_links, before.external_links);
    assert_eq!(after.function_types, before.function_types);
    assert_eq!(after.array_types, before.array_types);
    assert_eq!(after.optional_types, before.optional_types);
    assert_eq!(after.optional_box_types, before.optional_box_types);
    assert_eq!(after.literal_data, before.literal_data);
    assert_eq!(after.classes, before.classes);
    assert_eq!(after.interfaces, before.interfaces);
    assert_eq!(after.virtual_families, before.virtual_families);
    assert_eq!(after.declarations, before.declarations);
    assert_eq!(after.entry_function, before.entry_function);
    assert_eq!(after.span, before.span);
    assert_eq!(after.static_lifecycle, Some(coordinator));
    assert_eq!(after.definitions.get(used), before.definitions.get(used));
    assert_eq!(after.definitions.get(main), before.definitions.get(main));
}

#[test]
fn preserves_existing_function_holes_and_member_identity_order() {
    let mut program = lower_generic_source_to_final_mir(
        "class Live {
           init() {}
           fn read() -> i64 { return 3; }
         }
         class Dormant {
           init() {}
           fn first() -> i64 { return 1; }
           fn second() -> i64 { return 2; }
         }
         fn first_dead() -> i64 { return 1; }
         fn second_dead() -> i64 { return 2; }
         fn main() -> i64 { var live: Live = Live(); return live.read(); }",
    );
    let first_dead = function(&program, "first_dead");
    let second_dead = function(&program, "second_dead");
    program.remove_executable_definition_for_test(first_dead);
    let verified = verify_final_mir(program).unwrap();
    let member_order = verified
        .program()
        .member_definitions
        .indexed_entries()
        .map(|(callable, _)| callable)
        .collect::<Vec<_>>();
    let expected_member_order = member_order
        .iter()
        .copied()
        .filter(|callable| {
            verified
                .reachability()
                .reachable_callables()
                .contains(callable)
        })
        .collect::<Vec<_>>();

    let (after, summary) = apply_changed(&verified);

    assert_eq!(summary.removed_callables()[0], second_dead);
    let first_dead = first_dead.as_function().unwrap();
    let second_dead = second_dead.as_function().unwrap();
    assert!(after
        .definitions
        .indexed_slots()
        .nth(first_dead.index())
        .unwrap()
        .1
        .is_none());
    assert!(after
        .definitions
        .indexed_slots()
        .nth(second_dead.index())
        .unwrap()
        .1
        .is_none());
    assert_eq!(
        after
            .member_definitions
            .indexed_entries()
            .map(|(callable, _)| callable)
            .collect::<Vec<_>>(),
        expected_member_order
    );
    assert!(!after.member_definitions.is_empty());
    assert!(member_order.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn unchanged_and_repeated_retention_are_exact_and_idempotent() {
    let verified = verify_final_mir(lower_generic_source_to_final_mir(
        "fn dead() -> i64 { return 1; }
         fn main() -> i64 { return 0; }",
    ))
    .unwrap();
    let (retained, first_summary) = apply_changed(&verified);
    assert_eq!(first_summary.removed().functions(), 1);
    let resealed = verify_final_mir(retained.clone()).unwrap();

    let MirDefinitionRetention::Unchanged(second_summary) = prepare_verified(&resealed) else {
        panic!("retaining an already retained product must be unchanged")
    };
    assert_eq!(second_summary.removed().total(), 0);
    assert_eq!(second_summary.examined(), second_summary.retained());
    assert_eq!(resealed.program(), &retained);
}

#[test]
fn rejects_an_unreachable_static_initializer_before_consuming_any_container() {
    let program = lower_generic_source_to_final_mir(
        "class State { static value: i64 = 7; init() {} }
         fn main() -> i64 { return State.value; }",
    );
    let before = program.clone();
    let initializer = program.static_lifecycle.as_ref().unwrap().initializers()[0].id;

    let error = match prepare(&program, |_| false) {
        Err(error) => error,
        Ok(_) => panic!("unreachable static initialization must prevent retention"),
    };

    assert_eq!(
        error,
        MirDefinitionRetentionError::UnreachableStaticInitializer(initializer)
    );
    assert_eq!(program, before, "failed preparation must not mutate MIR");
}
