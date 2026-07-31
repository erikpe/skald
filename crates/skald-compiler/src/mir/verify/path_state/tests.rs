use crate::identity::FunctionId;

use super::*;

fn condition(index: usize) -> PathConditionId {
    PathConditionId::new(FunctionId::new(0), index)
}

fn only_state<State>(states: &PathStates<State>) -> &State {
    assert_eq!(states.alternatives.len(), 1);
    states.alternatives.values().next().unwrap()
}

#[test]
fn complementary_equal_states_compact_across_many_conditions() {
    let mut states = PathStates::initial(());

    for index in 0..256 {
        let condition = condition(index);
        let mut joined = states.begin_condition(condition, true, None).unwrap();
        let inactive = states.begin_condition(condition, false, None).unwrap();

        assert!(joined.merge(&inactive, |_, _| {
            panic!("equal states must not require a conflict merge")
        }));
        assert_eq!(joined.alternatives.len(), 1);
        assert!(joined.all_select(condition));
        states = joined;
    }

    for index in 0..256 {
        let condition = condition(index);
        for active in [false, true] {
            let (selected, missing) = states.select(condition, active);
            assert!(!missing);
            assert_eq!(selected.alternatives.len(), 1);
        }
    }

    for index in (0..256).rev() {
        assert!(!states.end_condition(condition(index), |_, _| {
            panic!("equal states must remain equal when a condition ends")
        }));
        assert_eq!(states.alternatives.len(), 1);
    }
}

#[test]
fn selected_either_remains_distinct_from_a_missing_condition() {
    let condition = condition(0);
    let initial = PathStates::initial(7_u8);
    let (missing, reported_missing) = initial.select(condition, true);
    assert!(reported_missing);
    assert!(missing.is_empty());

    let mut selected = initial.begin_condition(condition, true, None).unwrap();
    let inactive = initial.begin_condition(condition, false, None).unwrap();
    selected.merge(&inactive, |_, _| unreachable!());

    assert!(selected.all_select(condition));
    assert!(selected.any_select(condition));
    for active in [false, true] {
        let (branch, reported_missing) = selected.select(condition, active);
        assert!(!reported_missing);
        assert_eq!(*only_state(&branch), 7);
    }
}

#[test]
fn different_resource_states_stay_separate_until_the_condition_ends() {
    let condition = condition(0);
    let initial = PathStates::initial(0_u8);
    let mut active = initial.begin_condition(condition, true, None).unwrap();
    active.update_states(|state| *state = 1);
    let inactive = initial.begin_condition(condition, false, None).unwrap();
    active.merge(&inactive, |_, _| unreachable!());

    assert_eq!(active.alternatives.len(), 2);
    assert_eq!(*only_state(&active.select(condition, true).0), 1);
    assert_eq!(*only_state(&active.select(condition, false).0), 0);

    let mut conflicts = 0;
    assert!(!active.end_condition(condition, |existing, incoming| {
        conflicts += 1;
        *existing = (*existing).max(*incoming);
    }));
    assert_eq!(conflicts, 1);
    assert_eq!(*only_state(&active), 1);
}

#[test]
fn subset_updates_split_only_the_overlapping_compacted_predicate() {
    let first = condition(0);
    let second = condition(1);
    let initial = PathStates::initial(0_u8);

    let mut first_join = initial.begin_condition(first, true, None).unwrap();
    first_join.merge(
        &initial.begin_condition(first, false, None).unwrap(),
        |_, _| unreachable!(),
    );
    let mut all_equal = first_join.begin_condition(second, true, None).unwrap();
    all_equal.merge(
        &first_join.begin_condition(second, false, None).unwrap(),
        |_, _| unreachable!(),
    );
    assert_eq!(all_equal.alternatives.len(), 1);

    let mut active_subset = all_equal.select(first, true).0.select(second, true).0;
    active_subset.update_states(|state| *state = 4);
    let mut conflicts = 0;
    assert!(all_equal.merge(&active_subset, |existing, incoming| {
        conflicts += 1;
        *existing = *incoming;
    }));
    assert_eq!(conflicts, 1);
    assert_eq!(all_equal.alternatives.len(), 3);

    assert_eq!(
        *only_state(&all_equal.select(first, true).0.select(second, true).0),
        4
    );
    for (first_active, second_active) in [(false, false), (false, true), (true, false)] {
        let selected = all_equal
            .select(first, first_active)
            .0
            .select(second, second_active)
            .0;
        assert_eq!(*only_state(&selected), 0);
    }
}

#[test]
fn merge_boundary_recompacts_newly_equal_alternatives() {
    let condition = condition(0);
    let initial = PathStates::initial(0_u8);
    let mut states = initial.begin_condition(condition, true, None).unwrap();
    states.update_states(|state| *state = 1);
    states.merge(
        &initial.begin_condition(condition, false, None).unwrap(),
        |_, _| unreachable!(),
    );
    assert_eq!(states.alternatives.len(), 2);

    states.update_states(|state| *state = 9);
    states.normalize();
    assert_eq!(states.alternatives.len(), 1);
    for active in [false, true] {
        assert_eq!(*only_state(&states.select(condition, active).0), 9);
    }
}

#[test]
fn child_selection_requires_a_concrete_active_parent() {
    let parent = condition(0);
    let child = condition(1);
    let initial = PathStates::initial(());
    let mut parent_states = initial.begin_condition(parent, true, None).unwrap();
    parent_states.merge(
        &initial.begin_condition(parent, false, None).unwrap(),
        |_, _| unreachable!(),
    );

    assert_eq!(
        parent_states.begin_condition(child, true, Some(parent)),
        Err(PathEdgeError::ParentNotActive {
            condition: child,
            parent,
        })
    );

    let active_parent = parent_states.select(parent, true).0;
    let child_states = active_parent
        .begin_condition(child, true, Some(parent))
        .unwrap();
    assert!(child_states.all_select(parent));
    assert!(child_states.all_select(child));
}

#[test]
fn ended_conditions_can_begin_a_new_loop_epoch() {
    let condition = condition(0);
    let initial = PathStates::initial(());
    let mut first_epoch = initial.begin_condition(condition, true, None).unwrap();
    first_epoch.merge(
        &initial.begin_condition(condition, false, None).unwrap(),
        |_, _| unreachable!(),
    );
    assert!(!first_epoch.end_condition(condition, |_, _| unreachable!()));
    assert!(!first_epoch.any_select(condition));

    let second_epoch = first_epoch.begin_condition(condition, true, None).unwrap();
    assert!(second_epoch.all_select(condition));
}
