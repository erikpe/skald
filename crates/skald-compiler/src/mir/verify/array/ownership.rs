use std::collections::{HashMap, HashSet};

use super::super::{
    super::model::{
        BlockId, MirArgument, MirArrayFailure, MirArrayInstruction, MirCall, MirCallReceiver,
        MirDefinitionRef, MirInstruction, MirPlace, MirPlaceProjection, MirStorageKind,
        MirTerminator, MirType, StorageId,
    },
    context::Verifier,
    dataflow::ForwardDataflow,
    path_state::{condition_reads, PathStates},
};

impl Verifier<'_> {
    /// Checks exactly-once publication/consumption and balanced anchors. The
    /// CFG-level ordinary verifier separately checks every edge and join.
    pub(in crate::mir::verify) fn verify_array_ownership(
        &mut self,
        function: MirDefinitionRef<'_>,
    ) {
        self.verify_array_owner_cfg(function);
    }

    fn verify_array_owner_cfg(&mut self, function: MirDefinitionRef<'_>) {
        let anchor_owners: HashMap<_, _> = function
            .body()
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match instruction {
                MirInstruction::Array(MirArrayInstruction::AnchorBegin {
                    anchor, owner, ..
                }) => Some((*anchor, owner.clone())),
                _ => None,
            })
            .collect();
        let condition_reads = condition_reads(function);
        let activation_conditions: HashMap<_, _> = function
            .path_conditions()
            .iter()
            .map(|condition| (condition.activation, condition.id))
            .collect();
        let mut flow = ForwardDataflow::new(function.callable(), function.body().blocks.len());
        flow.seed(
            function.body().entry,
            PathStates::initial(ArrayOwnerState::default()),
        );
        let mut reported_joins = HashSet::new();

        loop {
            while let Some((block_id, mut states)) = flow.pop() {
                let Some(block) = function.block(block_id) else {
                    continue;
                };
                for instruction in &block.instructions {
                    states.update_states(|state| {
                        if let MirInstruction::Call(call) = instruction {
                            self.verify_array_alias_dependencies(
                                function,
                                block.id,
                                call,
                                state,
                                &anchor_owners,
                            );
                        }
                        if let MirInstruction::Array(MirArrayInstruction::AliasBind {
                            anchor,
                            source,
                            ..
                        }) = instruction
                        {
                            let compatible = state.anchors.contains(anchor)
                                && anchor_owners.get(anchor).is_some_and(|owner| {
                                    array_anchor_covers(*anchor, owner, source)
                                });
                            if !compatible {
                                self.block_error(
                                    function.callable(),
                                    block.id,
                                    "array alias binding requires one compatible live backing or owner anchor",
                                );
                            }
                        }
                        state.apply(self, function, block.id, instruction);
                    });
                    self.end_condition_at_storage_death(
                        function,
                        block.id,
                        instruction,
                        &activation_conditions,
                        &mut states,
                    );
                }

                let Some(terminator) = &block.terminator else {
                    continue;
                };
                match terminator {
                    MirTerminator::ArrayOperationCheck {
                        failure: MirArrayFailure::AllocationSize,
                        success_target,
                        failure_target,
                        ..
                    } => {
                        self.merge_array_owner_state(
                            function,
                            block.id,
                            *success_target,
                            &states,
                            &mut flow,
                            &mut reported_joins,
                        );
                        let mut failure_states = states.clone();
                        if let Some(MirInstruction::Array(MirArrayInstruction::Allocate {
                            backing,
                            ..
                        })) = block.instructions.last()
                        {
                            failure_states.update_states(|state| {
                                state.backings.remove(backing);
                                state.completed_backings.remove(backing);
                            });
                        }
                        self.merge_array_owner_state(
                            function,
                            block.id,
                            *failure_target,
                            &failure_states,
                            &mut flow,
                            &mut reported_joins,
                        );
                    }
                    MirTerminator::ArrayLoop {
                        backing,
                        body_target,
                        complete_target,
                        ..
                    } => {
                        self.merge_array_owner_state(
                            function,
                            block.id,
                            *body_target,
                            &states,
                            &mut flow,
                            &mut reported_joins,
                        );
                        let mut complete_states = states.clone();
                        complete_states.update_states(|state| {
                            state.completed_backings.insert(*backing);
                        });
                        self.merge_array_owner_state(
                            function,
                            block.id,
                            *complete_target,
                            &complete_states,
                            &mut flow,
                            &mut reported_joins,
                        );
                    }
                    MirTerminator::Return { .. }
                    | MirTerminator::ReturnShared { .. }
                    | MirTerminator::ReturnOptionalShared { .. } => {
                        states.update_states(|state| {
                            if !state.backings.is_empty()
                                || !state.completed_backings.is_empty()
                                || !state.produced.is_empty()
                                || !state.anchors.is_empty()
                                || !state.aliases.is_empty()
                                || !state.slice_checks.is_empty()
                            {
                                self.block_error(
                                    function.callable(),
                                    block.id,
                                    "array owner state must be fully consumed at normal return",
                                );
                            }
                        });
                    }
                    MirTerminator::Terminate { .. } => {}
                    MirTerminator::Branch {
                        condition,
                        true_target,
                        false_target,
                        ..
                    } if condition_reads.contains_key(condition) => {
                        let path_condition = condition_reads[condition];
                        for (target, active) in [(*true_target, true), (*false_target, false)] {
                            let (selected, _) = states.select(path_condition, active);
                            self.merge_array_owner_state(
                                function,
                                block.id,
                                target,
                                &selected,
                                &mut flow,
                                &mut reported_joins,
                            );
                        }
                    }
                    _ => {
                        for target in terminator.successors() {
                            self.merge_array_owner_state(
                                function,
                                block.id,
                                target,
                                &states,
                                &mut flow,
                                &mut reported_joins,
                            );
                        }
                    }
                }
            }
            if !flow.seed_next_component(
                &function.body().blocks,
                PathStates::initial(ArrayOwnerState::default()),
            ) {
                break;
            }
        }
    }

    fn verify_array_alias_dependencies(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: crate::mir::BlockId,
        call: &MirCall,
        state: &ArrayOwnerState,
        anchor_owners: &HashMap<StorageId, MirPlace>,
    ) {
        let receiver = call.receiver.as_ref().map(|receiver| match receiver {
            MirCallReceiver::Method(receiver) => &receiver.place,
            MirCallReceiver::Interface(view) => &view.source,
        });
        for borrowed in receiver
            .into_iter()
            .chain(call.arguments.iter().filter_map(|argument| match argument {
                MirArgument::Place(place) => Some(place),
                _ => None,
            }))
        {
            let covered = match borrowed.base {
                crate::mir::MirPlaceBase::ArrayAlias(alias) => state
                    .aliases
                    .get(&alias)
                    .is_some_and(|anchor| state.anchors.contains(anchor)),
                _ if array_borrow_requires_anchor(function, borrowed) => {
                    state.anchors.iter().any(|anchor| {
                        anchor_owners
                            .get(anchor)
                            .is_some_and(|owner| array_anchor_covers(*anchor, owner, borrowed))
                    })
                }
                _ => true,
            };
            if !covered {
                self.block_error(
                    function.callable(),
                    block,
                    "array alias call requires one compatible live descriptor, backing, or owner anchor",
                );
            }
        }
    }

    fn merge_array_owner_state(
        &mut self,
        function: MirDefinitionRef<'_>,
        predecessor: BlockId,
        target: BlockId,
        states: &PathStates<ArrayOwnerState>,
        flow: &mut ForwardDataflow<PathStates<ArrayOwnerState>>,
        reported_joins: &mut HashSet<BlockId>,
    ) {
        if states.is_empty() {
            return;
        }
        let selected = states
            .on_edge(function, predecessor, target)
            .unwrap_or_else(|_| states.clone());
        flow.merge(target, &selected, |existing, incoming| {
            existing.merge(incoming, |_existing, _incoming| {
                if reported_joins.insert(target) {
                    self.block_error(
                        function.callable(),
                        predecessor,
                        format!("array owner state disagrees at control-flow join {target}"),
                    );
                }
            })
        });
    }

    fn end_condition_at_storage_death(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: BlockId,
        instruction: &MirInstruction,
        activation_conditions: &HashMap<StorageId, crate::mir::PathConditionId>,
        states: &mut PathStates<ArrayOwnerState>,
    ) {
        let MirInstruction::StorageDead(operation) = instruction else {
            return;
        };
        let Some(condition) = activation_conditions.get(&operation.storage).copied() else {
            return;
        };
        let mut incompatible = false;
        let missing = states.end_condition(condition, |_existing, _incoming| {
            incompatible = true;
        });
        if incompatible {
            self.block_error(
                function.callable(),
                block,
                format!(
                    "conditional array owner state remains when path condition {condition} ends"
                ),
            );
        }
        if missing {
            self.block_error(
                function.callable(),
                block,
                format!(
                    "path condition {condition} ends outside its selected array-ownership region"
                ),
            );
        }
    }
}

fn array_borrow_requires_anchor(function: MirDefinitionRef<'_>, place: &MirPlace) -> bool {
    function
        .storage(place.base.storage())
        .is_some_and(|storage| matches!(storage.ty, MirType::Array(_)))
        || place
            .projections
            .iter()
            .any(|projection| matches!(projection, MirPlaceProjection::ArrayElement { .. }))
}

fn array_anchor_covers(anchor: StorageId, owner: &MirPlace, borrowed: &MirPlace) -> bool {
    if owner.base == borrowed.base
        && owner.projections.len() <= borrowed.projections.len()
        && borrowed.projections[..owner.projections.len()] == owner.projections
    {
        return true;
    }
    borrowed.projections.is_empty()
        && matches!(
            borrowed.base,
            crate::mir::MirPlaceBase::Storage(storage)
                if storage == anchor
        )
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ArrayOwnerState {
    backings: HashSet<StorageId>,
    completed_backings: HashSet<StorageId>,
    produced: HashSet<StorageId>,
    consumed: HashSet<StorageId>,
    anchors: HashSet<StorageId>,
    aliases: HashMap<StorageId, StorageId>,
    slice_checks: HashSet<(StorageId, StorageId)>,
}

impl ArrayOwnerState {
    fn apply(
        &mut self,
        verifier: &mut Verifier<'_>,
        function: MirDefinitionRef<'_>,
        block: crate::mir::BlockId,
        instruction: &MirInstruction,
    ) {
        if let MirInstruction::Call(call) = instruction {
            if let Some(destination) = &call.destination {
                let storage = destination.base.storage();
                if function
                    .storage(storage)
                    .is_some_and(|entry| entry.kind == MirStorageKind::ArrayProduced)
                    && (!self.produced.insert(storage) || self.consumed.contains(&storage))
                {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "produced array call destination is initialized more than once",
                    );
                }
            }
        }
        match instruction {
            MirInstruction::StorageLive(operation) => {
                self.reset_storage(operation.storage);
                return;
            }
            MirInstruction::StorageDead(operation) => {
                if self.has_active_storage(operation.storage) {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "array owner state remains active at storage-dead",
                    );
                }
                if self.produced.contains(&operation.storage)
                    && !self.consumed.contains(&operation.storage)
                {
                    verifier.block_error(
                        function.callable(),
                        block,
                        format!(
                            "produced array storage {} is never consumed",
                            operation.storage
                        ),
                    );
                }
                if self.anchors.contains(&operation.storage) {
                    verifier.block_error(
                        function.callable(),
                        block,
                        format!("array anchor {} is not ended", operation.storage),
                    );
                }
                self.reset_storage(operation.storage);
                return;
            }
            _ => {}
        }
        let MirInstruction::Array(instruction) = instruction else {
            return;
        };
        match instruction {
            MirArrayInstruction::Allocate { backing, .. } => {
                if !self.backings.insert(*backing) || self.completed_backings.contains(backing) {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "array backing is allocated more than once",
                    );
                }
                self.completed_backings.remove(backing);
            }
            MirArrayInstruction::Publish {
                backing,
                destination,
                ..
            } => {
                if !self.completed_backings.remove(backing)
                    || !self.backings.remove(backing)
                    || !self.produced.insert(*destination)
                    || self.consumed.contains(destination)
                {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "array publication requires one completed unpublished backing",
                    );
                }
            }
            MirArrayInstruction::PublishShared { backing, .. } => {
                if !self.completed_backings.remove(backing) || !self.backings.remove(backing) {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "shared array publication requires one completed unpublished backing",
                    );
                }
            }
            MirArrayInstruction::SliceCopy { destination, .. } => {
                if !self.produced.insert(*destination) || self.consumed.contains(destination) {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "slice temporary is completed more than once",
                    );
                }
            }
            MirArrayInstruction::Adopt { source, .. }
            | MirArrayInstruction::Replace { source, .. } => {
                if !self.produced.remove(source) || !self.consumed.insert(*source) {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "produced array storage must be consumed exactly once",
                    );
                }
            }
            MirArrayInstruction::Release { owner, .. } => {
                let storage = owner.base.storage();
                self.produced.remove(&storage);
                if function.storage(storage).is_some_and(|storage| {
                    matches!(
                        storage.kind,
                        MirStorageKind::ArrayProduced | MirStorageKind::ArraySlice
                    )
                }) {
                    self.consumed.insert(storage);
                }
            }
            MirArrayInstruction::AnchorBegin { anchor, .. } => {
                if !self.anchors.insert(*anchor) {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "array anchor begins more than once",
                    );
                }
            }
            MirArrayInstruction::AnchorEnd { anchor, .. } => {
                if !self.anchors.remove(anchor) {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "array anchor ends without being live",
                    );
                }
            }
            MirArrayInstruction::AliasBind { alias, anchor, .. } => {
                self.aliases.insert(*alias, *anchor);
            }
            MirArrayInstruction::SliceLengthCheck {
                destination_start,
                destination_end,
                ..
            } => {
                self.slice_checks
                    .insert((*destination_start, *destination_end));
            }
            MirArrayInstruction::SliceAssignNext {
                destination_index, ..
            } if !self
                .slice_checks
                .iter()
                .any(|(start, _)| start == destination_index) =>
            {
                verifier.block_error(
                    function.callable(),
                    block,
                    "slice assignment writes before its length check",
                );
            }
            _ => {}
        }
    }

    fn has_active_storage(&self, storage: StorageId) -> bool {
        self.backings.contains(&storage)
            || self.completed_backings.contains(&storage)
            || self.produced.contains(&storage)
            || self.anchors.contains(&storage)
    }

    fn reset_storage(&mut self, storage: StorageId) {
        self.backings.remove(&storage);
        self.completed_backings.remove(&storage);
        self.produced.remove(&storage);
        self.consumed.remove(&storage);
        self.anchors.remove(&storage);
        self.aliases
            .retain(|alias, anchor| *alias != storage && *anchor != storage);
        self.slice_checks
            .retain(|(start, end)| *start != storage && *end != storage);
    }
}
