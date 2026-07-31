use std::collections::HashMap;

use crate::mir::{BlockId, MirBasicBlock, MirInstruction, MirTerminator, StorageId};

use super::{
    super::super::{
        dataflow::ForwardDataflow,
        path_state::{condition_reads, PathStates},
    },
    state::SharedState,
    SharedOwnershipAnalysis,
};

impl SharedOwnershipAnalysis<'_, '_> {
    pub(super) fn analyze(&mut self) {
        let initial = SharedState::at_entry(self.function);
        let condition_reads = condition_reads(self.function);
        let activation_conditions: HashMap<_, _> = self
            .function
            .path_conditions()
            .iter()
            .map(|condition| (condition.activation, condition.id))
            .collect();
        let mut flow =
            ForwardDataflow::new(self.function.callable(), self.function.body().blocks.len());
        flow.seed(
            self.function.body().entry,
            PathStates::initial(initial.clone()),
        );

        loop {
            while let Some((block_id, mut states)) = flow.pop() {
                let Some(block) = self.function.block(block_id) else {
                    continue;
                };
                states.update_states(|state| {
                    self.apply_block(block, state);
                });
                self.end_conditions_at_storage_death(block, &activation_conditions, &mut states);
                match &block.terminator {
                    Some(MirTerminator::Goto { target, .. }) => {
                        self.merge(block.id, *target, &states, &mut flow)
                    }
                    Some(MirTerminator::Branch {
                        condition,
                        true_target,
                        false_target,
                        ..
                    }) => {
                        if let Some(path_condition) = condition_reads.get(condition).copied() {
                            for (target, active) in [(*true_target, true), (*false_target, false)] {
                                let (selected, _) = states.select(path_condition, active);
                                self.merge(block.id, target, &selected, &mut flow);
                            }
                        } else {
                            self.merge(block.id, *true_target, &states, &mut flow);
                            self.merge(block.id, *false_target, &states, &mut flow);
                        }
                    }
                    Some(MirTerminator::CheckedCast {
                        binding,
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        let mut success = states.clone();
                        success.update_states(|state| {
                            self.require_live_pointee(block.id, state, &binding.view.source);
                            self.require_live_shared_origin(block.id, state, &binding.view.origin);
                            self.begin_checked_view(block.id, state, binding);
                        });
                        self.merge(block.id, *success_target, &success, &mut flow);
                        self.merge(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::SharedCast {
                        cast,
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        let mut success = states.clone();
                        success.update_states(|state| {
                            self.require_shared_cast_source(block.id, state, cast);
                            self.apply_shared_cast(block.id, state, cast);
                        });
                        self.merge(block.id, *success_target, &success, &mut flow);
                        self.merge(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::OptionalUnwrap {
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        self.merge(block.id, *success_target, &states, &mut flow);
                        self.merge(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::OptionalSharedUnwrap {
                        unwrap,
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        let mut success = states.clone();
                        success.update_states(|state| {
                            if state.live_owners.contains(&unwrap.destination)
                                || state.released_owners.contains(&unwrap.destination)
                            {
                                self.error(
                                    block.id,
                                    "optional shared unwrap destination is already initialized",
                                );
                            } else {
                                state.live_owners.insert(unwrap.destination);
                                state
                                    .owner_origins
                                    .insert(unwrap.destination, unwrap.destination);
                                state.pending_full_expression_boundary = true;
                            }
                        });
                        self.merge(block.id, *success_target, &success, &mut flow);
                        self.merge(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::BeginOptionalView {
                        success_target,
                        absent_target,
                        overflow_target,
                        ..
                    }) => {
                        self.merge(block.id, *success_target, &states, &mut flow);
                        self.merge(block.id, *absent_target, &states, &mut flow);
                        self.merge(block.id, *overflow_target, &states, &mut flow);
                    }
                    Some(MirTerminator::CheckOptionalMutation {
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        self.merge(block.id, *success_target, &states, &mut flow);
                        self.merge(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::ArrayPositionCheck {
                        success_target,
                        failure_target,
                        ..
                    })
                    | Some(MirTerminator::ArrayOperationCheck {
                        success_target,
                        failure_target,
                        ..
                    }) => {
                        self.merge(block.id, *success_target, &states, &mut flow);
                        self.merge(block.id, *failure_target, &states, &mut flow);
                    }
                    Some(MirTerminator::ArrayLoop {
                        body_target,
                        complete_target,
                        ..
                    }) => {
                        self.merge(block.id, *body_target, &states, &mut flow);
                        self.merge(block.id, *complete_target, &states, &mut flow);
                    }
                    Some(MirTerminator::Return { .. }) => {
                        states.update_states(|state| {
                            self.check_return(block, state, None);
                        });
                    }
                    Some(MirTerminator::ReturnShared { owner, .. }) => {
                        states.update_states(|state| {
                            self.check_return(block, state, Some(*owner));
                        });
                    }
                    Some(MirTerminator::ReturnOptionalShared { .. }) => {
                        states.update_states(|state| {
                            self.check_return(block, state, None);
                        });
                    }
                    Some(MirTerminator::Panic { .. })
                    | Some(MirTerminator::Terminate { .. })
                    | None => {}
                }
            }
            if !flow.seed_next_component(
                &self.function.body().blocks,
                PathStates::initial(initial.clone()),
            ) {
                break;
            }
        }
    }

    fn end_conditions_at_storage_death(
        &mut self,
        block: &MirBasicBlock,
        activation_conditions: &HashMap<StorageId, crate::mir::PathConditionId>,
        states: &mut PathStates<SharedState>,
    ) {
        for instruction in &block.instructions {
            let MirInstruction::StorageDead(operation) = instruction else {
                continue;
            };
            let Some(condition) = activation_conditions.get(&operation.storage).copied() else {
                continue;
            };
            let mut incompatible = false;
            let missing = states.end_condition(condition, |existing, incoming| {
                if !existing.same_live_state(incoming) {
                    incompatible = true;
                    return;
                }
                existing
                    .released_owners
                    .extend(incoming.released_owners.iter().copied());
            });
            if incompatible {
                self.error(
                    block.id,
                    format!(
                        "conditional shared ownership state remains when path condition {condition} ends"
                    ),
                );
            }
            if missing {
                self.error(
                    block.id,
                    format!(
                        "path condition {condition} ends outside its selected shared-ownership region"
                    ),
                );
            }
        }
    }

    fn merge(
        &mut self,
        predecessor: BlockId,
        target: BlockId,
        states: &PathStates<SharedState>,
        flow: &mut ForwardDataflow<PathStates<SharedState>>,
    ) {
        if states.is_empty() {
            return;
        }
        let selected = states
            .on_edge(self.function, predecessor, target)
            .unwrap_or_else(|_| states.clone());
        flow.merge(target, &selected, |existing, incoming| {
            existing.merge(incoming, |existing, incoming| {
                if !existing.same_live_state(incoming) {
                    if self.reported_joins.insert(target) {
                        self.error(
                            target,
                            "shared ownership state differs across control-flow paths",
                        );
                    }
                    return;
                }
                existing
                    .released_owners
                    .extend(incoming.released_owners.iter().copied());
            })
        });
    }
}
