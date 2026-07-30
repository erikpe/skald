//! Reusable path-sensitive state carried by MIR dataflow verifiers.

use std::collections::{BTreeMap, HashMap};

use crate::mir::{
    BlockId, MirDefinitionRef, MirInstruction, MirRvalueKind, PathConditionId, ValueId,
};

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct PathPredicate(BTreeMap<PathConditionId, bool>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PathStates<State> {
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

    pub(super) fn states_mut(&mut self) -> impl Iterator<Item = &mut State> {
        self.alternatives.values_mut()
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

        let mut alternatives = BTreeMap::new();
        for (predicate, state) in &self.alternatives {
            if let Some(parent) = condition.parent {
                if predicate.0.get(&parent) != Some(&true) {
                    return Err(PathEdgeError::ParentNotActive {
                        condition: condition.id,
                        parent,
                    });
                }
            }
            let mut selected = predicate.clone();
            if selected.0.insert(condition.id, active).is_some() {
                return Err(PathEdgeError::ConditionAlreadySelected(condition.id));
            }
            alternatives.insert(selected, state.clone());
        }
        Ok(Self { alternatives })
    }

    pub(super) fn select(&self, condition: PathConditionId, active: bool) -> (Self, bool) {
        let mut missing = false;
        let alternatives = self
            .alternatives
            .iter()
            .filter_map(|(predicate, state)| match predicate.0.get(&condition) {
                Some(selected) if *selected == active => Some((predicate.clone(), state.clone())),
                Some(_) => None,
                None => {
                    missing = true;
                    None
                }
            })
            .collect();
        (Self { alternatives }, missing)
    }

    pub(super) fn merge(
        &mut self,
        incoming: &Self,
        mut merge_conflict: impl FnMut(&mut State, &State),
    ) -> bool
    where
        State: Eq,
    {
        let mut changed = false;
        for (predicate, incoming_state) in &incoming.alternatives {
            match self.alternatives.get_mut(predicate) {
                Some(existing) if existing != incoming_state => {
                    let before = existing.clone();
                    merge_conflict(existing, incoming_state);
                    changed |= *existing != before;
                }
                Some(_) => {}
                None => {
                    self.alternatives
                        .insert(predicate.clone(), incoming_state.clone());
                    changed = true;
                }
            }
        }
        changed
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
        let mut collapsed = BTreeMap::new();
        for (mut predicate, state) in std::mem::take(&mut self.alternatives) {
            missing |= predicate.0.remove(&condition).is_none();
            match collapsed.get_mut(&predicate) {
                Some(existing) if existing != &state => merge_conflict(existing, &state),
                Some(_) => {}
                None => {
                    collapsed.insert(predicate, state);
                }
            }
        }
        self.alternatives = collapsed;
        missing
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
