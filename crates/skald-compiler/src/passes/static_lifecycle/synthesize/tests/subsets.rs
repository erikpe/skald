//! Sparse coordinator synthesis and malformed-component fixtures.

use crate::{
    mir::{dump_mir, lower_preliminary_hir},
    test_support::type_check_source,
};

use super::{errors, synthesize_static_lifecycle, verify_planned_mir, verify_synthesized_mir};

const SUBSET_DECLARATIONS: &str = "class State {
      static first: i64 = 1;
      static inactive: i64 = 2;
      static last: i64 = 3;
      init() {}
    }";

fn sparse_synthesized(active_indices: &[usize]) -> crate::mir::MirProgram {
    let names = ["first", "inactive", "last"];
    let accesses = active_indices
        .iter()
        .map(|index| format!("State.{0} = State.{0};", names[*index]))
        .collect::<Vec<_>>()
        .join(" ");
    let source = format!("{SUBSET_DECLARATIONS} fn main() -> i64 {{ {accesses} return 0; }}");
    let checked = type_check_source(&source);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let preliminary = lower_preliminary_hir(&checked.hir.unwrap());
    let preliminary = crate::mir::verify_preliminary_mir(preliminary)
        .expect("sparse synthesis fixture must produce verified preliminary MIR");
    let planned = super::super::super::plan_static_lifetimes(preliminary)
        .expect("test active set must have an acyclic lifecycle");
    synthesize_static_lifecycle(verify_planned_mir(planned).unwrap())
}

#[test]
fn moves_only_active_bodies_without_compacting_declarations() {
    let program = sparse_synthesized(&[2, 0]);
    let coordinator = program.static_lifecycle.as_ref().unwrap();
    let declared = program
        .classes
        .iter()
        .flat_map(|class| class.static_fields.iter().map(|field| field.id))
        .collect::<Vec<_>>();

    assert_eq!(declared.len(), 3);
    assert_eq!(coordinator.lifecycle().proof().activation().len(), 2);
    assert!(coordinator
        .lifecycle()
        .proof()
        .activation()
        .contains(declared[0]));
    assert!(!coordinator
        .lifecycle()
        .proof()
        .activation()
        .contains(declared[1]));
    assert!(coordinator
        .lifecycle()
        .proof()
        .activation()
        .contains(declared[2]));
    assert_eq!(coordinator.initializers().len(), 2);
    assert!(coordinator
        .initializers()
        .iter()
        .all(|initializer| initializer.field != declared[1]));
    assert_eq!(coordinator.activation().len(), 2);
    assert_eq!(coordinator.shutdown().len(), 2);
    verify_synthesized_mir(&program).unwrap();
    assert_eq!(dump_mir(&program.clone()), dump_mir(&program));
}

#[test]
fn rejects_missing_and_extra_coordinator_components() {
    let valid = sparse_synthesized(&[0, 2]);

    let mut missing_coordinator = valid.clone();
    missing_coordinator.static_lifecycle = None;
    assert!(errors(&missing_coordinator).contains("has no lifecycle coordinator"));

    let mut missing_initializer = valid.clone();
    missing_initializer
        .static_lifecycle
        .as_mut()
        .unwrap()
        .initializers_mut_for_test()
        .pop();
    assert!(errors(&missing_initializer).contains("do not exactly cover"));

    let complete = sparse_synthesized(&[0, 1, 2]);
    let inactive_region = complete.static_lifecycle.as_ref().unwrap().activation()[1].clone();
    let mut extra_region = valid;
    extra_region
        .static_lifecycle
        .as_mut()
        .unwrap()
        .activation_mut_for_test()
        .push(inactive_region);
    assert!(errors(&extra_region).contains("do not cover every active field"));
}
