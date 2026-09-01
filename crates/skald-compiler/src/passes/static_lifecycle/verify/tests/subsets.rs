//! Empty and sparse active-authority verification fixtures.

use crate::{
    identity::{ClassId, StaticFieldId},
    mir::{lower_preliminary_hir, MirExecutionNode},
    test_support::type_check_source,
};

use super::{errors, verify_planned_mir, PlannedMirProgram};

const SUBSET_DECLARATIONS: &str = "class State {
       static first: i64 = 1;
       static inactive: i64 = 2;
       static last: i64 = 3;
       init() {}
     }";

fn sparse_plan(active_indices: &[usize]) -> PlannedMirProgram {
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
        .expect("sparse planning fixture must produce verified preliminary MIR");
    super::super::super::plan_static_lifetimes(preliminary)
        .expect("test active set must have an acyclic lifecycle")
}

#[test]
fn accepts_empty_one_sparse_and_complete_active_authority() {
    for active in [&[][..], &[0][..], &[0, 2][..], &[0, 1, 2][..]] {
        let planned = sparse_plan(active);
        assert_eq!(planned.activation_authority().len(), active.len());
        assert_eq!(planned.lifecycle_mir().definitions().len(), active.len());
        assert_eq!(planned.lifecycle().activation().len(), active.len());
        assert_eq!(planned.authority().roots().len(), active.len());
        assert_eq!(planned.static_fields().len(), 3);
        assert_eq!(planned.static_initializers().len(), 3);
        verify_planned_mir(planned).unwrap();
    }
}

#[test]
fn sparse_products_keep_stable_declarations_and_deterministic_derived_views() {
    let planned = sparse_plan(&[2, 0]);
    let declared = planned
        .static_fields()
        .map(|field| field.field)
        .collect::<Vec<_>>();
    assert!(planned.activation_authority().contains(declared[0]));
    assert!(!planned.activation_authority().contains(declared[1]));
    assert!(planned.activation_authority().contains(declared[2]));
    assert_eq!(
        planned.lifecycle().activation(),
        &[declared[0], declared[2]]
    );
    assert_eq!(
        planned.lifecycle().shutdown().collect::<Vec<_>>(),
        [declared[2], declared[0]]
    );
    let expected = super::super::super::dump_planned_mir(&planned);
    assert!(expected.contains("ActiveFields"), "{expected}");
    assert_eq!(
        super::super::super::dump_planned_mir(&planned.clone()),
        expected
    );
    assert_eq!(
        planned
            .lifecycle_mir()
            .definitions()
            .iter()
            .map(|definition| definition.field)
            .collect::<Vec<_>>(),
        [declared[0], declared[2]]
    );
}

#[test]
fn rejects_malformed_active_field_authority() {
    let mut duplicate = sparse_plan(&[0, 2]);
    let fields = duplicate
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .activation_mut_for_test()
        .fields_mut_for_test();
    fields.insert(1, fields[0]);
    assert!(errors(&duplicate).contains("duplicate field"));

    let mut reordered = sparse_plan(&[0, 2]);
    reordered
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .activation_mut_for_test()
        .fields_mut_for_test()
        .reverse();
    assert!(errors(&reordered).contains("not in canonical field order"));

    let mut foreign = sparse_plan(&[0]);
    foreign
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .activation_mut_for_test()
        .fields_mut_for_test()[0] = StaticFieldId::new(ClassId::new(99), 0);
    assert!(errors(&foreign).contains("names undeclared static field"));
}

#[test]
fn rejects_missing_and_extra_subset_roots_and_schema_entries() {
    let mut missing_root = sparse_plan(&[0, 2]);
    missing_root
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .authority_mut_for_test()
        .roots_mut_for_test()
        .pop();
    assert!(errors(&missing_root).contains("omits lifecycle root"));

    let full = sparse_plan(&[0, 1, 2]);
    let inactive_field = full.static_fields().nth(1).unwrap().field;
    let inactive_initializer = full.static_fields().nth(1).unwrap().initializer.unwrap();
    let extra_root = full
        .authority()
        .root(MirExecutionNode::callable(inactive_initializer.into()))
        .unwrap()
        .clone();
    let mut extra = sparse_plan(&[0, 2]);
    assert!(!extra.activation_authority().contains(inactive_field));
    let roots = extra
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .authority_mut_for_test()
        .roots_mut_for_test();
    roots.push(extra_root);
    roots.sort_by_key(|root| root.root());
    assert!(errors(&extra).contains("extra lifecycle root"));

    let mut missing_definition = sparse_plan(&[0, 2]);
    missing_definition
        .lifecycle_mut_for_test()
        .definitions_mut_for_test()
        .pop();
    assert!(errors(&missing_definition).contains("active authority"));

    let mut missing_activation = sparse_plan(&[0, 2]);
    missing_activation
        .lifecycle_mut_for_test()
        .plan_mut_for_test()
        .activation_mut_for_test()
        .pop();
    assert!(errors(&missing_activation).contains("active authority"));
}
