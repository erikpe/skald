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
    let entry_state = InitializationState::at_entry(program, function);
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
                let presence = optional_presence_refinement(program, function, block, *condition);
                if let Some(path_condition) = condition_reads.get(condition).copied() {
                    for (target, active) in [(*true_target, true), (*false_target, false)] {
                        let (mut selected, _) = states.select(path_condition, active);
                        apply_presence_refinement(&mut selected, presence.as_ref(), active);
                        merge_states(function, block.id, target, &selected, &mut flow);
                    }
                    continue;
                }
                for (target, active) in [(*true_target, true), (*false_target, false)] {
                    let mut selected = states.clone();
                    apply_presence_refinement(&mut selected, presence.as_ref(), active);
                    merge_states(function, block.id, target, &selected, &mut flow);
                }
                continue;
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

fn apply_presence_refinement(
    states: &mut PathStates<InitializationState>,
    refinement: Option<&(crate::mir::MirPlace, bool)>,
    active: bool,
) {
    let Some((payload, present_when)) = refinement else {
        return;
    };
    if active == *present_when {
        states.update_states(|state| {
            state.refine(payload.clone());
        });
    }
}

fn optional_presence_refinement(
    program: &MirProgram,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    condition: crate::mir::ValueId,
) -> Option<(crate::mir::MirPlace, bool)> {
    let assignment = block.instructions.iter().rev().find_map(|instruction| {
        let MirInstruction::Assign(assignment) = instruction else {
            return None;
        };
        (assignment.result == condition).then_some(assignment)
    })?;
    let crate::mir::MirRvalueKind::OptionalPresence { source, kind } = &assignment.rvalue.kind
    else {
        return None;
    };
    let optional = optional_at_place(program, function, source)?;
    matches!(
        program.optional_type(optional)?.storage,
        crate::mir::MirOptionalStorage::Nested(_) | crate::mir::MirOptionalStorage::InlineArray(_)
    )
    .then(|| {
        (
            source.clone().project_nested_optional_payload(optional),
            *kind == crate::mir::MirPresenceTestKind::Some,
        )
    })
}

fn optional_at_place(
    program: &MirProgram,
    function: MirDefinitionRef<'_>,
    place: &crate::mir::MirPlace,
) -> Option<crate::identity::OptionalTypeId> {
    use crate::mir::{MirPlaceBase, MirPlaceProjection, MirType};
    let mut ty = match place.base {
        MirPlaceBase::StaticField(field) | MirPlaceBase::StaticLifecycleDestination(field) => {
            program.static_field(field)?.ty
        }
        _ => function.storage(place.base.local_storage()?)?.ty,
    };
    for projection in &place.projections {
        ty = match *projection {
            MirPlaceProjection::Base(base) => (program.direct_base(match ty {
                MirType::Class(class) => class,
                _ => return None,
            }) == Some(base))
            .then_some(MirType::Class(base))?,
            MirPlaceProjection::Field(field) => program.field(field)?.ty,
            MirPlaceProjection::OptionalPayload(class) => MirType::Class(class),
            MirPlaceProjection::NestedOptionalPayload(optional) => {
                if ty != MirType::Optional(optional) {
                    return None;
                }
                program.optional_type(optional)?.payload
            }
            MirPlaceProjection::ArrayElement { array, .. } => {
                if ty != MirType::Array(array) {
                    return None;
                }
                program.array_type(array)?.element
            }
        };
    }
    match ty {
        MirType::Optional(optional) => Some(optional),
        _ => None,
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
