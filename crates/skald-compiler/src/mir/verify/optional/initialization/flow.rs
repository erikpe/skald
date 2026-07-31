use std::collections::HashMap;

use crate::mir::{
    BlockId, MirBasicBlock, MirDefinitionRef, MirInstruction, MirProgram, MirTerminator,
    PathConditionId, StorageId,
};

use super::{
    super::super::{
        dataflow::ForwardDataflow,
        path_state::{condition_reads, PathStates},
    },
    state::InitializationState,
};

pub(super) struct Analysis {
    activation_conditions: HashMap<StorageId, PathConditionId>,
    flow: ForwardDataflow<PathStates<InitializationState>>,
}

impl Analysis {
    pub(super) fn state(&self, block: BlockId) -> Option<&PathStates<InitializationState>> {
        self.flow.state(block)
    }

    pub(super) fn activation_condition(&self, storage: StorageId) -> Option<PathConditionId> {
        self.activation_conditions.get(&storage).copied()
    }
}

pub(super) fn analyze(program: &MirProgram, function: MirDefinitionRef<'_>) -> Analysis {
    let entry_state = InitializationState::at_entry(function);
    let activation_conditions = function
        .path_conditions()
        .iter()
        .map(|condition| (condition.activation, condition.id))
        .collect::<HashMap<_, _>>();
    let condition_reads = condition_reads(function);
    let mut flow = ForwardDataflow::new(function.callable(), function.body().blocks.len());
    flow.seed(
        function.body().entry,
        PathStates::initial(entry_state.clone()),
    );

    loop {
        while let Some((block_id, mut states)) = flow.pop() {
            let Some(block) = function.block(block_id) else {
                continue;
            };
            states.update_states(|state| state.apply_block(program, function, block));
            collapse_conditions_at_storage_death(&mut states, block, &activation_conditions);

            if let Some(MirTerminator::Branch {
                condition,
                true_target,
                false_target,
                ..
            }) = &block.terminator
            {
                if let Some(path_condition) = condition_reads.get(condition).copied() {
                    for (target, active) in [(*true_target, true), (*false_target, false)] {
                        let (selected, _) = states.select(path_condition, active);
                        merge_states(function, block.id, target, &selected, &mut flow);
                    }
                    continue;
                }
            }
            for successor in block.terminator.iter().flat_map(MirTerminator::successors) {
                merge_states(function, block.id, successor, &states, &mut flow);
            }
        }

        if !flow.seed_next_component(
            &function.body().blocks,
            PathStates::initial(entry_state.clone()),
        ) {
            break;
        }
    }

    Analysis {
        activation_conditions,
        flow,
    }
}

fn merge_states(
    function: MirDefinitionRef<'_>,
    predecessor: BlockId,
    target: BlockId,
    states: &PathStates<InitializationState>,
    flow: &mut ForwardDataflow<PathStates<InitializationState>>,
) {
    if states.is_empty() {
        return;
    }
    let selected = states
        .on_edge(function, predecessor, target)
        .unwrap_or_else(|_| states.clone());
    flow.merge(target, &selected, |existing, incoming| {
        existing.merge(incoming, InitializationState::merge)
    });
}

fn collapse_conditions_at_storage_death(
    states: &mut PathStates<InitializationState>,
    block: &MirBasicBlock,
    activation_conditions: &HashMap<StorageId, PathConditionId>,
) {
    for instruction in &block.instructions {
        let MirInstruction::StorageDead(operation) = instruction else {
            continue;
        };
        let Some(condition) = activation_conditions.get(&operation.storage).copied() else {
            continue;
        };
        states.end_condition(condition, InitializationState::merge);
    }
}
