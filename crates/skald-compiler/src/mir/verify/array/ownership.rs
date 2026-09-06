use std::collections::{HashMap, HashSet};

use super::super::{
    super::model::{
        BlockId, MirArgument, MirArrayFailure, MirArrayInstruction, MirArrayLoopKind,
        MirArrayPositionKind, MirCall, MirCallReceiver, MirDefinitionRef, MirInstruction,
        MirIoBuffer, MirIoOperation, MirPlace, MirPlaceProjection, MirStorageKind, MirTerminator,
        MirType, StorageId,
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
                        if let MirInstruction::Io(io) = instruction {
                            self.verify_io_array_dependencies(
                                function,
                                block.id,
                                &io.operation,
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
                    MirTerminator::ArrayPositionCheck {
                        position,
                        kind: MirArrayPositionKind::RangeOffset,
                        success_target,
                        failure_target,
                        ..
                    } => {
                        let mut success_states = states.clone();
                        success_states.update_states(|state| {
                            state.checked_range_offsets.insert(*position);
                        });
                        self.merge_array_owner_state(
                            function,
                            block.id,
                            *success_target,
                            &success_states,
                            &mut flow,
                            &mut reported_joins,
                        );
                        self.merge_array_owner_state(
                            function,
                            block.id,
                            *failure_target,
                            &states,
                            &mut flow,
                            &mut reported_joins,
                        );
                    }
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
                        if let Some(MirInstruction::Array(operation)) = block.instructions.last() {
                            let backing = match operation {
                                MirArrayInstruction::Allocate { backing, .. }
                                | MirArrayInstruction::AllocateElements { backing, .. } => {
                                    Some(*backing)
                                }
                                _ => None,
                            };
                            if let Some(backing) = backing {
                                failure_states.update_states(|state| {
                                    state.backings.remove(&backing);
                                    state.completed_backings.remove(&backing);
                                    state.element_lists.remove(&backing);
                                    state.indexed.remove(&backing);
                                });
                            }
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
                        index,
                        length,
                        kind,
                        body_target,
                        complete_target,
                        ..
                    } => {
                        let mut body_states = states.clone();
                        let mut complete_states = states.clone();
                        if matches!(kind, MirArrayLoopKind::Indexed { .. }) {
                            body_states.update_states(|state| {
                                state.enter_indexed_element(
                                    self, function, block.id, *backing, *index, *length,
                                );
                            });
                            complete_states.update_states(|state| {
                                state.exit_indexed_loop(
                                    self, function, block.id, *backing, *index, *length,
                                );
                            });
                        } else {
                            complete_states.update_states(|state| {
                                state.completed_backings.insert(*backing);
                            });
                        }
                        self.merge_array_owner_state(
                            function,
                            block.id,
                            *body_target,
                            &body_states,
                            &mut flow,
                            &mut reported_joins,
                        );
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
                                || !state.element_lists.is_empty()
                                || !state.indexed.is_empty()
                                || !state.produced.is_empty()
                                || !state.anchors.is_empty()
                                || !state.aliases.is_empty()
                                || !state.slice_checks.is_empty()
                                || !state.checked_range_offsets.is_empty()
                                || !state.range_offset_owners.is_empty()
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
                _ if array_borrow_requires_anchor(self.program, function, borrowed) => {
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

    fn verify_io_array_dependencies(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: crate::mir::BlockId,
        operation: &MirIoOperation,
        state: &ArrayOwnerState,
        anchor_owners: &HashMap<StorageId, MirPlace>,
    ) {
        let (buffer, offset) = match operation {
            MirIoOperation::Open { path, .. } => (Some(path), None),
            MirIoOperation::Read {
                destination,
                offset,
                ..
            } => (Some(destination), Some(*offset)),
            MirIoOperation::Write { source, offset, .. } => (Some(source), Some(*offset)),
            MirIoOperation::StandardHandle { .. } | MirIoOperation::Close { .. } => (None, None),
        };
        let Some(buffer) = buffer else {
            return;
        };
        if !io_buffer_is_anchored(buffer, state, anchor_owners) {
            self.block_error(
                function.callable(),
                block,
                "standard-I/O buffer requires its exact compatible backing anchor to be live",
            );
        }
        if offset.is_some_and(|offset| {
            !state.checked_range_offsets.contains(&offset)
                || state.range_offset_owners.get(&offset) != Some(&buffer.place)
        }) {
            self.block_error(
                function.callable(),
                block,
                "standard-I/O byte range must be dominated by its successful offset bounds check",
            );
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

fn array_borrow_requires_anchor(
    program: &crate::mir::MirProgram,
    function: MirDefinitionRef<'_>,
    place: &MirPlace,
) -> bool {
    place.base.static_field().is_some_and(|field| {
        program
            .static_field(field)
            .is_some_and(|field| matches!(field.ty, MirType::Array(_)))
    }) || place
        .base
        .local_storage()
        .and_then(|storage| function.storage(storage))
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
    element_lists: HashMap<StorageId, ElementListState>,
    indexed: HashMap<StorageId, IndexedConstructionState>,
    produced: HashSet<StorageId>,
    consumed: HashSet<StorageId>,
    anchors: HashSet<StorageId>,
    aliases: HashMap<StorageId, StorageId>,
    slice_checks: HashSet<(StorageId, StorageId)>,
    checked_range_offsets: HashSet<StorageId>,
    range_offset_owners: HashMap<StorageId, MirPlace>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ElementListState {
    array: crate::identity::ArrayTypeId,
    prefix: StorageId,
    length: u64,
    next: u64,
    element_state: ElementInitializationState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IndexedConstructionState {
    array: crate::identity::ArrayTypeId,
    prefix: StorageId,
    length: StorageId,
    phase: IndexedConstructionPhase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndexedConstructionPhase {
    Header,
    Element,
    Bound,
    ValueReady,
    Initialized,
    Exit,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ElementInitializationState {
    #[default]
    Uninitialized,
    Ready,
    ClassOptionalPayloadReady,
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
                if let Some(storage) = destination.base.local_storage() {
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
        }
        if let Some(destination) = completed_class_destination(instruction) {
            self.record_completed_class_destination(verifier, function, block, destination);
        }
        if let Some(destination) = completed_optional_destination(instruction) {
            self.record_completed_optional_destination(verifier, function, block, destination);
        }
        if let MirInstruction::SharedFieldInitialize(initialize) = instruction {
            self.record_completed_shared_destination(
                verifier,
                function,
                block,
                &initialize.destination,
            );
        }
        if let MirInstruction::Array(MirArrayInstruction::Adopt { destination, .. }) = instruction {
            self.record_completed_array_destination(verifier, function, block, destination);
        }
        if let MirInstruction::ClassOptionalPublish(publish) = instruction {
            self.record_published_class_optional_destination(
                verifier,
                function,
                block,
                &publish.destination,
            );
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
        if let MirInstruction::Array(MirArrayInstruction::Offset {
            destination, owner, ..
        }) = instruction
        {
            self.checked_range_offsets.remove(destination);
            self.range_offset_owners.insert(*destination, owner.clone());
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
            MirArrayInstruction::AllocateElements {
                backing,
                prefix,
                array,
                length,
                ..
            } => {
                let duplicate = !self.backings.insert(*backing)
                    || self.completed_backings.contains(backing)
                    || self.element_lists.contains_key(backing)
                    || self
                        .element_lists
                        .values()
                        .any(|state| state.prefix == *prefix);
                if duplicate {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "array element-list backing or prefix is allocated more than once",
                    );
                }
                self.completed_backings.remove(backing);
                self.element_lists.insert(
                    *backing,
                    ElementListState {
                        array: *array,
                        prefix: *prefix,
                        length: *length,
                        next: 0,
                        element_state: ElementInitializationState::Uninitialized,
                    },
                );
                if *length == 0 {
                    self.completed_backings.insert(*backing);
                }
            }
            MirArrayInstruction::BeginIndexed {
                backing,
                prefix,
                length,
                ..
            } => {
                let Some(array) = function
                    .storage(*backing)
                    .and_then(|storage| match storage.ty {
                        MirType::Array(array) => Some(array),
                        _ => None,
                    })
                else {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "indexed array construction requires array backing storage",
                    );
                    return;
                };
                let valid = self.backings.contains(backing)
                    && !self.completed_backings.contains(backing)
                    && !self.element_lists.contains_key(backing)
                    && self
                        .indexed
                        .values()
                        .all(|state| state.prefix != *prefix && state.length != *length)
                    && self
                        .indexed
                        .insert(
                            *backing,
                            IndexedConstructionState {
                                array,
                                prefix: *prefix,
                                length: *length,
                                phase: IndexedConstructionPhase::Header,
                            },
                        )
                        .is_none();
                if !valid {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "indexed array construction must begin once on a live unpublished backing",
                    );
                }
            }
            MirArrayInstruction::BindIndexed {
                backing,
                prefix,
                length,
                ..
            } => {
                let valid = self.indexed.get_mut(backing).is_some_and(|state| {
                    state.prefix == *prefix
                        && state.length == *length
                        && state.phase == IndexedConstructionPhase::Element
                        && {
                            state.phase = IndexedConstructionPhase::Bound;
                            true
                        }
                });
                if !valid {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "indexed array binding requires the current canonical element epoch",
                    );
                }
            }
            MirArrayInstruction::InitializeIndexedElement {
                backing, prefix, ..
            } => {
                let valid = self.indexed.get_mut(backing).is_some_and(|state| {
                    state.prefix == *prefix && state.phase == IndexedConstructionPhase::Bound && {
                        state.phase = IndexedConstructionPhase::Initialized;
                        true
                    }
                });
                if !valid {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "indexed array element must initialize and advance the exact current slot once",
                    );
                }
            }
            MirArrayInstruction::AdvanceIndexedElement {
                backing, prefix, ..
            } => {
                let valid = self.indexed.get_mut(backing).is_some_and(|state| {
                    state.prefix == *prefix
                        && state.phase == IndexedConstructionPhase::ValueReady
                        && {
                            state.phase = IndexedConstructionPhase::Initialized;
                            true
                        }
                });
                if !valid {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "indexed array prefix may advance only after the current lifecycle-bearing slot is complete",
                    );
                }
            }
            MirArrayInstruction::EndIndexedElement {
                backing,
                prefix,
                length,
                ..
            } => {
                let valid = self.indexed.get_mut(backing).is_some_and(|state| {
                    state.prefix == *prefix
                        && state.length == *length
                        && state.phase == IndexedConstructionPhase::Initialized
                        && {
                            state.phase = IndexedConstructionPhase::Header;
                            true
                        }
                });
                if !valid {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "indexed array element epoch must end after initialization and cleanup",
                    );
                }
            }
            MirArrayInstruction::CompleteIndexed {
                backing,
                prefix,
                length,
                ..
            } => {
                let valid = self.indexed.get(backing).is_some_and(|state| {
                    state.prefix == *prefix
                        && state.length == *length
                        && state.phase == IndexedConstructionPhase::Exit
                });
                if valid {
                    self.indexed.remove(backing);
                    self.completed_backings.insert(*backing);
                } else {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "indexed array completion requires the canonical prefix-equals-length exit",
                    );
                }
            }
            MirArrayInstruction::InitializeElement {
                backing,
                prefix,
                position,
                ..
            } => {
                let Some(state) = self.element_lists.get_mut(backing) else {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "array element initialization requires a live unpublished element-list backing",
                    );
                    return;
                };
                if state.prefix != *prefix
                    || state.next != *position
                    || state.next >= state.length
                    || state.element_state != ElementInitializationState::Uninitialized
                {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "array element initialization must advance the exact source-ordered prefix",
                    );
                    return;
                }
                state.next += 1;
                if state.next == state.length {
                    self.completed_backings.insert(*backing);
                }
            }
            MirArrayInstruction::CompleteElement {
                backing,
                prefix,
                position,
                ..
            } => {
                let Some(state) = self.element_lists.get_mut(backing) else {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "lifecycle-bearing array element completion requires a live unpublished element-list backing",
                    );
                    return;
                };
                if state.prefix != *prefix
                    || state.next != *position
                    || state.next >= state.length
                    || state.element_state != ElementInitializationState::Ready
                {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "lifecycle-bearing array element completion must advance the exact constructed source-ordered prefix",
                    );
                    return;
                }
                state.element_state = ElementInitializationState::Uninitialized;
                state.next += 1;
                if state.next == state.length {
                    self.completed_backings.insert(*backing);
                }
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
                if !self.backings.contains(backing) {
                    self.element_lists.remove(backing);
                    self.indexed.remove(backing);
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
                if !self.backings.contains(backing) {
                    self.element_lists.remove(backing);
                    self.indexed.remove(backing);
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
            MirArrayInstruction::Adopt {
                destination,
                source,
                ..
            } => {
                if !self.produced.remove(source) || !self.consumed.insert(*source) {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "produced array storage must be consumed exactly once",
                    );
                }
                if let Some(storage) = destination.base.local_storage() {
                    if function
                        .storage(storage)
                        .is_some_and(|entry| entry.kind == MirStorageKind::ArrayProduced)
                        && (!self.produced.insert(storage) || self.consumed.contains(&storage))
                    {
                        verifier.block_error(
                            function.callable(),
                            block,
                            "produced array adoption destination is initialized more than once",
                        );
                    }
                }
            }
            MirArrayInstruction::Replace { source, .. } => {
                if !self.produced.remove(source) || !self.consumed.insert(*source) {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "produced array storage must be consumed exactly once",
                    );
                }
            }
            MirArrayInstruction::Release { owner, .. } => {
                let Some(storage) = owner.base.local_storage() else {
                    return;
                };
                if function.storage(storage).is_some_and(|storage| {
                    matches!(
                        storage.kind,
                        MirStorageKind::ArrayProduced | MirStorageKind::ArraySlice
                    )
                }) && (!self.produced.remove(&storage) || !self.consumed.insert(storage))
                {
                    verifier.block_error(
                        function.callable(),
                        block,
                        "produced array storage must be released exactly once",
                    );
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
            || self.element_lists.contains_key(&storage)
            || self
                .element_lists
                .values()
                .any(|state| state.prefix == storage)
            || self.indexed.contains_key(&storage)
            || self
                .indexed
                .values()
                .any(|state| state.prefix == storage || state.length == storage)
            || self.produced.contains(&storage)
            || self.anchors.contains(&storage)
    }

    fn enter_indexed_element(
        &mut self,
        verifier: &mut Verifier<'_>,
        function: MirDefinitionRef<'_>,
        block: BlockId,
        backing: StorageId,
        prefix: StorageId,
        length: StorageId,
    ) {
        let valid = self.indexed.get_mut(&backing).is_some_and(|state| {
            state.prefix == prefix
                && state.length == length
                && state.phase == IndexedConstructionPhase::Header
                && {
                    state.phase = IndexedConstructionPhase::Element;
                    true
                }
        });
        if !valid {
            verifier.block_error(
                function.callable(),
                block,
                "indexed array loop body requires one active header-ready construction",
            );
        }
    }

    fn exit_indexed_loop(
        &mut self,
        verifier: &mut Verifier<'_>,
        function: MirDefinitionRef<'_>,
        block: BlockId,
        backing: StorageId,
        prefix: StorageId,
        length: StorageId,
    ) {
        let valid = self.indexed.get_mut(&backing).is_some_and(|state| {
            state.prefix == prefix
                && state.length == length
                && state.phase == IndexedConstructionPhase::Header
                && {
                    state.phase = IndexedConstructionPhase::Exit;
                    true
                }
        });
        if !valid {
            verifier.block_error(
                function.callable(),
                block,
                "indexed array loop exit requires one active header-ready construction",
            );
        }
    }

    fn record_completed_class_destination(
        &mut self,
        verifier: &mut Verifier<'_>,
        function: MirDefinitionRef<'_>,
        block: BlockId,
        destination: &MirPlace,
    ) {
        if destination
            .projections
            .iter()
            .any(|projection| matches!(projection, MirPlaceProjection::AggregateOptionalPayload(_)))
        {
            return;
        }
        let crate::mir::MirPlaceBase::Storage(backing) = destination.base else {
            return;
        };
        if let Some(state) = self.indexed.get_mut(&backing) {
            let exact_slot = matches!(
                destination.projections.as_slice(),
                [MirPlaceProjection::ArrayElement {
                    array,
                    normalized_index,
                }] if *array == state.array && *normalized_index == state.prefix
            );
            if !exact_slot || state.phase != IndexedConstructionPhase::Bound {
                verifier.block_error(
                    function.callable(),
                    block,
                    "indexed class construction must complete exactly once in the current prefix slot",
                );
                return;
            }
            state.phase = IndexedConstructionPhase::ValueReady;
            return;
        }
        let Some(state) = self.element_lists.get_mut(&backing) else {
            if function
                .storage(backing)
                .is_some_and(|storage| storage.kind == MirStorageKind::ArrayBacking)
            {
                verifier.block_error(
                    function.callable(),
                    block,
                    "class array element construction requires a live unpublished element-list backing",
                );
            }
            return;
        };
        let exact_slot = matches!(
            destination.projections.as_slice(),
            [MirPlaceProjection::ArrayElement {
                array,
                normalized_index,
            }] if *array == state.array && *normalized_index == state.prefix
        );
        if exact_slot {
            if state.next >= state.length
                || state.element_state != ElementInitializationState::Uninitialized
            {
                verifier.block_error(
                    function.callable(),
                    block,
                    "class array element construction must complete exactly once in the current prefix slot",
                );
                return;
            }
            state.element_state = ElementInitializationState::Ready;
            return;
        }

        let optional_payload = matches!(
            destination.projections.as_slice(),
            [
                MirPlaceProjection::ArrayElement {
                    array,
                    normalized_index,
                },
                MirPlaceProjection::OptionalPayload(_),
            ] if *array == state.array && *normalized_index == state.prefix
        );
        if !optional_payload
            || state.next >= state.length
            || state.element_state != ElementInitializationState::Ready
        {
            verifier.block_error(
                function.callable(),
                block,
                "class array element construction must complete exactly once in the current prefix slot",
            );
            return;
        }
        state.element_state = ElementInitializationState::ClassOptionalPayloadReady;
    }

    fn record_completed_optional_destination(
        &mut self,
        verifier: &mut Verifier<'_>,
        function: MirDefinitionRef<'_>,
        block: BlockId,
        destination: &MirPlace,
    ) {
        if destination
            .projections
            .iter()
            .any(|projection| matches!(projection, MirPlaceProjection::AggregateOptionalPayload(_)))
        {
            return;
        }
        let crate::mir::MirPlaceBase::Storage(backing) = destination.base else {
            return;
        };
        let Some(state) = self.element_lists.get_mut(&backing) else {
            if function
                .storage(backing)
                .is_some_and(|storage| storage.kind == MirStorageKind::ArrayBacking)
            {
                verifier.block_error(
                    function.callable(),
                    block,
                    "optional array element initialization requires a live unpublished element-list backing",
                );
            }
            return;
        };
        let exact_slot = matches!(
            destination.projections.as_slice(),
            [MirPlaceProjection::ArrayElement {
                array,
                normalized_index,
            }] if *array == state.array && *normalized_index == state.prefix
        );
        if !exact_slot
            || state.next >= state.length
            || state.element_state != ElementInitializationState::Uninitialized
        {
            verifier.block_error(
                function.callable(),
                block,
                "optional array element initialization must complete exactly once in the current prefix slot",
            );
            return;
        }
        state.element_state = ElementInitializationState::Ready;
    }

    fn record_completed_shared_destination(
        &mut self,
        verifier: &mut Verifier<'_>,
        function: MirDefinitionRef<'_>,
        block: BlockId,
        destination: &MirPlace,
    ) {
        if destination
            .projections
            .iter()
            .any(|projection| matches!(projection, MirPlaceProjection::AggregateOptionalPayload(_)))
        {
            return;
        }
        let crate::mir::MirPlaceBase::Storage(backing) = destination.base else {
            return;
        };
        let Some(state) = self.element_lists.get_mut(&backing) else {
            if function
                .storage(backing)
                .is_some_and(|storage| storage.kind == MirStorageKind::ArrayBacking)
            {
                verifier.block_error(
                    function.callable(),
                    block,
                    "shared-owner array element initialization requires a live unpublished element-list backing",
                );
            }
            return;
        };
        let exact_slot = matches!(
            destination.projections.as_slice(),
            [MirPlaceProjection::ArrayElement {
                array,
                normalized_index,
            }] if *array == state.array && *normalized_index == state.prefix
        );
        if !exact_slot
            || state.next >= state.length
            || state.element_state != ElementInitializationState::Uninitialized
        {
            verifier.block_error(
                function.callable(),
                block,
                "shared-owner array element initialization must complete exactly once in the current prefix slot",
            );
            return;
        }
        state.element_state = ElementInitializationState::Ready;
    }

    fn record_completed_array_destination(
        &mut self,
        verifier: &mut Verifier<'_>,
        function: MirDefinitionRef<'_>,
        block: BlockId,
        destination: &MirPlace,
    ) {
        if destination
            .projections
            .iter()
            .any(|projection| matches!(projection, MirPlaceProjection::AggregateOptionalPayload(_)))
        {
            return;
        }
        let crate::mir::MirPlaceBase::Storage(backing) = destination.base else {
            return;
        };
        let Some(state) = self.element_lists.get_mut(&backing) else {
            if function
                .storage(backing)
                .is_some_and(|storage| storage.kind == MirStorageKind::ArrayBacking)
            {
                verifier.block_error(
                    function.callable(),
                    block,
                    "nested array element transfer requires a live unpublished element-list backing",
                );
            }
            return;
        };
        let exact_slot = matches!(
            destination.projections.as_slice(),
            [MirPlaceProjection::ArrayElement {
                array,
                normalized_index,
            }] if *array == state.array && *normalized_index == state.prefix
        );
        if !exact_slot
            || state.next >= state.length
            || state.element_state != ElementInitializationState::Uninitialized
        {
            verifier.block_error(
                function.callable(),
                block,
                "nested array element transfer must complete exactly once in the current prefix slot",
            );
            return;
        }
        state.element_state = ElementInitializationState::Ready;
    }

    fn record_published_class_optional_destination(
        &mut self,
        verifier: &mut Verifier<'_>,
        function: MirDefinitionRef<'_>,
        block: BlockId,
        destination: &MirPlace,
    ) {
        let crate::mir::MirPlaceBase::Storage(backing) = destination.base else {
            return;
        };
        let Some(state) = self.element_lists.get_mut(&backing) else {
            if function
                .storage(backing)
                .is_some_and(|storage| storage.kind == MirStorageKind::ArrayBacking)
            {
                verifier.block_error(
                    function.callable(),
                    block,
                    "class optional array element publication requires a live unpublished element-list backing",
                );
            }
            return;
        };
        let exact_slot = matches!(
            destination.projections.as_slice(),
            [MirPlaceProjection::ArrayElement {
                array,
                normalized_index,
            }] if *array == state.array && *normalized_index == state.prefix
        );
        if !exact_slot
            || state.next >= state.length
            || state.element_state != ElementInitializationState::ClassOptionalPayloadReady
        {
            verifier.block_error(
                function.callable(),
                block,
                "class optional array element publication requires a completed current payload",
            );
            return;
        }
        state.element_state = ElementInitializationState::Ready;
    }

    fn reset_storage(&mut self, storage: StorageId) {
        self.backings.remove(&storage);
        self.completed_backings.remove(&storage);
        self.element_lists
            .retain(|backing, state| *backing != storage && state.prefix != storage);
        self.indexed.retain(|backing, state| {
            *backing != storage && state.prefix != storage && state.length != storage
        });
        self.produced.remove(&storage);
        self.consumed.remove(&storage);
        self.anchors.remove(&storage);
        self.aliases
            .retain(|alias, anchor| *alias != storage && *anchor != storage);
        self.slice_checks
            .retain(|(start, end)| *start != storage && *end != storage);
        self.checked_range_offsets.remove(&storage);
        self.range_offset_owners.remove(&storage);
    }
}

fn completed_class_destination(instruction: &MirInstruction) -> Option<&MirPlace> {
    match instruction {
        MirInstruction::Initialize(initialize) => Some(&initialize.destination),
        MirInstruction::CopyConstruct(copy) => Some(&copy.destination),
        MirInstruction::Call(call) => call.destination.as_ref(),
        MirInstruction::StringInitialize(initialize) => Some(&initialize.destination),
        _ => None,
    }
}

fn completed_optional_destination(instruction: &MirInstruction) -> Option<&MirPlace> {
    match instruction {
        MirInstruction::OptionalInitialize(initialize) => Some(&initialize.destination),
        MirInstruction::ClassOptionalInitialize(initialize) => Some(&initialize.destination),
        MirInstruction::OptionalSharedInitialize(initialize) => Some(&initialize.destination),
        MirInstruction::AggregateOptionalInitialize(initialize)
            if !matches!(
                initialize.source,
                crate::mir::MirAggregateOptionalSource::Unpublished
            ) =>
        {
            Some(&initialize.destination)
        }
        MirInstruction::AggregateOptionalPublish(publish) => Some(&publish.destination),
        _ => None,
    }
}

fn io_buffer_is_anchored(
    buffer: &MirIoBuffer,
    state: &ArrayOwnerState,
    anchor_owners: &HashMap<StorageId, MirPlace>,
) -> bool {
    if !state.anchors.contains(&buffer.anchor) {
        return false;
    }
    match buffer.place.base {
        crate::mir::MirPlaceBase::ArrayAlias(alias) => {
            state.aliases.get(&alias) == Some(&buffer.anchor)
        }
        _ => anchor_owners
            .get(&buffer.anchor)
            .is_some_and(|owner| array_anchor_covers(buffer.anchor, owner, &buffer.place)),
    }
}
