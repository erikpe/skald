//! Reusable path-sensitive state carried by MIR dataflow verifiers.

use std::collections::{BTreeMap, HashMap};

use crate::mir::{
    BlockId, MirDefinitionRef, MirInstruction, MirRvalueKind, PathConditionId, ValueId,
};

/// One condition's selected values represented by a predicate cube.
///
/// `Either` means both concrete selections reach the same verifier state. An
/// absent map entry remains a distinct "not selected in this epoch" state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SelectedValue {
    Inactive,
    Active,
    Either,
}

impl SelectedValue {
    const fn exact(active: bool) -> Self {
        if active {
            Self::Active
        } else {
            Self::Inactive
        }
    }

    const fn matches(self, active: bool) -> bool {
        matches!(
            (self, active),
            (Self::Active, true) | (Self::Inactive, false) | (Self::Either, _)
        )
    }
}

/// A conjunction over the path conditions selected in one active epoch.
///
/// Predicates stored together in `PathStates` are disjoint. Complementary
/// predicates with equal resource state are canonicalized to `Either`.
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct PathPredicate(BTreeMap<PathConditionId, SelectedValue>);

impl PathPredicate {
    fn intersection(&self, other: &Self) -> Option<Self> {
        if self.0.len() != other.0.len() {
            return None;
        }
        let mut intersection = BTreeMap::new();
        for (condition, left) in &self.0 {
            let right = other.0.get(condition)?;
            let selected = match (*left, *right) {
                (SelectedValue::Either, selected) | (selected, SelectedValue::Either) => selected,
                (left, right) if left == right => left,
                _ => return None,
            };
            intersection.insert(*condition, selected);
        }
        Some(Self(intersection))
    }

    /// Returns a disjoint cube cover for `self` with `other` removed.
    fn difference(&self, other: &Self) -> Vec<Self> {
        if self.intersection(other).is_none() {
            return vec![self.clone()];
        }

        let mut remaining = self.clone();
        let mut difference = Vec::new();
        for (condition, other_selected) in &other.0 {
            let selected = remaining
                .0
                .get(condition)
                .copied()
                .expect("intersecting predicates have the same selected conditions");
            let (SelectedValue::Either, concrete) = (selected, *other_selected) else {
                continue;
            };
            if concrete == SelectedValue::Either {
                continue;
            }

            let opposite = match concrete {
                SelectedValue::Inactive => SelectedValue::Active,
                SelectedValue::Active => SelectedValue::Inactive,
                SelectedValue::Either => unreachable!("either selection was handled above"),
            };
            let mut outside = remaining.clone();
            outside.0.insert(*condition, opposite);
            difference.push(outside);
            remaining.0.insert(*condition, concrete);
        }
        difference
    }

    fn merge_complement(&self, other: &Self) -> Option<Self> {
        if self.0.len() != other.0.len() {
            return None;
        }
        let mut merged = self.clone();
        let mut complement = None;
        for (condition, left) in &self.0 {
            let right = other.0.get(condition)?;
            if left == right {
                continue;
            }
            if complement.is_some()
                || !matches!(
                    (*left, *right),
                    (SelectedValue::Inactive, SelectedValue::Active)
                        | (SelectedValue::Active, SelectedValue::Inactive)
                )
            {
                return None;
            }
            complement = Some(*condition);
        }
        let condition = complement?;
        merged.0.insert(condition, SelectedValue::Either);
        Some(merged)
    }

    fn specialize(&self, condition: PathConditionId, active: bool) -> Option<Self> {
        let selected = self.0.get(&condition).copied()?;
        if !selected.matches(active) {
            return None;
        }
        let mut specialized = self.clone();
        specialized
            .0
            .insert(condition, SelectedValue::exact(active));
        Some(specialized)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PathStates<State> {
    /// Canonical, non-overlapping predicate cubes and their resource states.
    alternatives: BTreeMap<PathPredicate, State>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PathEdgeError {
    ParentNotActive {
        condition: PathConditionId,
        parent: PathConditionId,
    },
    ConditionAlreadySelected(PathConditionId),
}

impl<State> PathStates<State> {
    pub(super) fn initial(state: State) -> Self {
        Self {
            alternatives: BTreeMap::from([(PathPredicate::default(), state)]),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.alternatives.is_empty()
    }

    pub(super) fn all_select(&self, condition: PathConditionId) -> bool {
        self.alternatives
            .keys()
            .all(|predicate| predicate.0.contains_key(&condition))
    }

    pub(super) fn any_select(&self, condition: PathConditionId) -> bool {
        self.alternatives
            .keys()
            .any(|predicate| predicate.0.contains_key(&condition))
    }
}

impl<State: Clone> PathStates<State> {
    pub(super) fn on_edge(
        &self,
        function: MirDefinitionRef<'_>,
        predecessor: BlockId,
        target: BlockId,
    ) -> Result<Self, PathEdgeError> {
        let Some((condition, active)) = function.path_conditions().iter().find_map(|condition| {
            if condition.merge != target {
                return None;
            }
            if condition.active_predecessor == predecessor {
                Some((condition, true))
            } else if condition.inactive_predecessor == predecessor {
                Some((condition, false))
            } else {
                None
            }
        }) else {
            return Ok(self.clone());
        };

        self.begin_condition(condition.id, active, condition.parent)
    }

    pub(super) fn select(&self, condition: PathConditionId, active: bool) -> (Self, bool) {
        let mut missing = false;
        let alternatives = self
            .alternatives
            .iter()
            .filter_map(|(predicate, state)| {
                if !predicate.0.contains_key(&condition) {
                    missing = true;
                    return None;
                }
                predicate
                    .specialize(condition, active)
                    .map(|predicate| (predicate, state.clone()))
            })
            .collect();
        (Self { alternatives }, missing)
    }

    /// Applies one block-local transfer to every distinct resource state.
    ///
    /// Normalization is deliberately deferred to `merge` or `end_condition`:
    /// transfers often run once per instruction, and selected branch states
    /// immediately feed a successor merge.
    pub(super) fn update_states(&mut self, mut update: impl FnMut(&mut State)) {
        for state in self.alternatives.values_mut() {
            update(state);
        }
    }

    pub(super) fn merge(
        &mut self,
        incoming: &Self,
        mut merge_conflict: impl FnMut(&mut State, &State),
    ) -> bool
    where
        State: Eq,
    {
        let before = self.clone();
        for (predicate, incoming_state) in &incoming.alternatives {
            Self::overlay(
                &mut self.alternatives,
                predicate.clone(),
                incoming_state.clone(),
                &mut merge_conflict,
            );
        }
        self.normalize();
        *self != before
    }

    pub(super) fn end_condition(
        &mut self,
        condition: PathConditionId,
        mut merge_conflict: impl FnMut(&mut State, &State),
    ) -> bool
    where
        State: Eq,
    {
        let mut missing = false;
        let mut collapsed = Self {
            alternatives: BTreeMap::new(),
        };
        for (mut predicate, state) in std::mem::take(&mut self.alternatives) {
            missing |= predicate.0.remove(&condition).is_none();
            Self::overlay(
                &mut collapsed.alternatives,
                predicate,
                state,
                &mut merge_conflict,
            );
        }
        collapsed.normalize();
        *self = collapsed;
        missing
    }

    fn begin_condition(
        &self,
        condition: PathConditionId,
        active: bool,
        parent: Option<PathConditionId>,
    ) -> Result<Self, PathEdgeError> {
        let mut alternatives = BTreeMap::new();
        for (predicate, state) in &self.alternatives {
            if let Some(parent) = parent {
                if predicate.0.get(&parent) != Some(&SelectedValue::Active) {
                    return Err(PathEdgeError::ParentNotActive { condition, parent });
                }
            }
            let mut selected = predicate.clone();
            if selected
                .0
                .insert(condition, SelectedValue::exact(active))
                .is_some()
            {
                return Err(PathEdgeError::ConditionAlreadySelected(condition));
            }
            alternatives.insert(selected, state.clone());
        }
        Ok(Self { alternatives })
    }

    fn overlay(
        alternatives: &mut BTreeMap<PathPredicate, State>,
        incoming_predicate: PathPredicate,
        incoming_state: State,
        merge_conflict: &mut impl FnMut(&mut State, &State),
    ) where
        State: Eq,
    {
        // A later loop edge may update only one concrete subset of an `Either`
        // cube. Partition both cubes around their intersection so the domain's
        // conflict merge touches exactly that subset.
        let Some(existing_predicate) = alternatives
            .keys()
            .find(|existing| existing.intersection(&incoming_predicate).is_some())
            .cloned()
        else {
            assert!(
                alternatives
                    .insert(incoming_predicate, incoming_state)
                    .is_none(),
                "non-overlapping path predicate must be unique"
            );
            return;
        };
        let existing_state = alternatives
            .remove(&existing_predicate)
            .expect("overlapping path predicate must still exist");
        let intersection = existing_predicate
            .intersection(&incoming_predicate)
            .expect("selected path predicates must intersect");

        for remainder in existing_predicate.difference(&incoming_predicate) {
            assert!(
                alternatives
                    .insert(remainder, existing_state.clone())
                    .is_none(),
                "existing predicate remainder must stay disjoint"
            );
        }

        let mut intersection_state = existing_state;
        if intersection_state != incoming_state {
            merge_conflict(&mut intersection_state, &incoming_state);
        }
        assert!(
            alternatives
                .insert(intersection, intersection_state)
                .is_none(),
            "predicate intersection must stay disjoint"
        );

        for remainder in incoming_predicate.difference(&existing_predicate) {
            Self::overlay(
                alternatives,
                remainder,
                incoming_state.clone(),
                merge_conflict,
            );
        }
    }

    fn normalize(&mut self)
    where
        State: Eq,
    {
        // Lookup complementary cubes directly instead of comparing all pairs.
        // Repeating is necessary because one merge can expose a complement in
        // another dimension.
        loop {
            let mut merge = None;
            'predicates: for (left, state) in &self.alternatives {
                for (condition, selected) in &left.0 {
                    let opposite = match selected {
                        SelectedValue::Inactive => SelectedValue::Active,
                        SelectedValue::Active => SelectedValue::Inactive,
                        SelectedValue::Either => continue,
                    };
                    let mut right = left.clone();
                    right.0.insert(*condition, opposite);
                    if self.alternatives.get(&right) != Some(state) {
                        continue;
                    }
                    let predicate = left
                        .merge_complement(&right)
                        .expect("complementary predicates must compact");
                    merge = Some((left.clone(), right, predicate));
                    break 'predicates;
                }
            }
            let Some((left, right, merged)) = merge else {
                break;
            };
            let state = self
                .alternatives
                .remove(&left)
                .expect("normalization source must exist");
            self.alternatives
                .remove(&right)
                .expect("normalization complement must exist");
            assert!(
                self.alternatives.insert(merged, state).is_none(),
                "compacted predicate must not duplicate an existing cube"
            );
        }
    }
}

pub(super) fn condition_reads(function: MirDefinitionRef<'_>) -> HashMap<ValueId, PathConditionId> {
    function
        .body()
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) => match assignment.rvalue.kind {
                MirRvalueKind::PathCondition(condition) => {
                    Some((assignment.result, condition.condition))
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests;
