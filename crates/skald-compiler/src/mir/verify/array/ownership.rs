use std::collections::{HashMap, HashSet, VecDeque};

use super::super::{
    super::model::{
        MirArrayFailure, MirArrayInstruction, MirDefinitionRef, MirInstruction, MirStorageKind,
        MirTerminator, StorageId,
    },
    context::Verifier,
};

impl Verifier<'_> {
    /// Checks exactly-once publication/consumption and balanced anchors. The
    /// CFG-level ordinary verifier separately checks every edge and join.
    pub(in crate::mir::verify) fn verify_array_ownership(
        &mut self,
        function: MirDefinitionRef<'_>,
    ) {
        let mut allocated = HashSet::new();
        let mut published = HashMap::<StorageId, StorageId>::new();
        let mut consumed = HashSet::new();
        let mut anchors = HashSet::new();
        let mut slice_checks = HashSet::new();

        for block in &function.body().blocks {
            for instruction in &block.instructions {
                if let MirInstruction::Call(call) = instruction {
                    if let Some(destination) = &call.destination {
                        let storage = destination.base.storage();
                        if function
                            .storage(storage)
                            .is_some_and(|entry| entry.kind == MirStorageKind::ArrayProduced)
                        {
                            published.insert(storage, storage);
                        }
                    }
                }
                let MirInstruction::Array(instruction) = instruction else {
                    continue;
                };
                match instruction {
                    MirArrayInstruction::Allocate { backing, .. } => {
                        if !allocated.insert(*backing) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "array backing is allocated more than once",
                            );
                        }
                    }
                    MirArrayInstruction::Publish {
                        backing,
                        destination,
                        ..
                    } => {
                        if !allocated.remove(backing)
                            || published.insert(*destination, *backing).is_some()
                        {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "array publication requires one unpublished backing",
                            );
                        }
                    }
                    MirArrayInstruction::PublishShared { backing, .. } => {
                        if !allocated.remove(backing) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "shared array publication requires one unpublished backing",
                            );
                        }
                    }
                    MirArrayInstruction::SliceCopy { destination, .. } => {
                        if published.insert(*destination, *destination).is_some() {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "slice temporary is completed more than once",
                            );
                        }
                    }
                    MirArrayInstruction::Adopt { source, .. }
                    | MirArrayInstruction::Replace { source, .. } => {
                        if published.remove(source).is_none() || !consumed.insert(*source) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "produced array storage must be consumed exactly once",
                            );
                        }
                    }
                    MirArrayInstruction::AnchorBegin { anchor, .. } => {
                        if !anchors.insert(*anchor) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "array anchor begins more than once",
                            );
                        }
                    }
                    MirArrayInstruction::AnchorEnd { anchor, .. } => {
                        if !anchors.remove(anchor) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "array anchor ends without being live",
                            );
                        }
                    }
                    MirArrayInstruction::SliceLengthCheck {
                        destination_start,
                        destination_end,
                        ..
                    } => {
                        slice_checks.insert((*destination_start, *destination_end));
                    }
                    MirArrayInstruction::SliceAssignNext {
                        destination_index, ..
                    } => {
                        if !slice_checks
                            .iter()
                            .any(|(start, _)| start == destination_index)
                        {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "slice assignment writes before its length check",
                            );
                        }
                    }
                    MirArrayInstruction::Release { owner, .. } => {
                        let storage = owner.base.storage();
                        published.remove(&storage);
                        consumed.insert(storage);
                    }
                    _ => {}
                }
            }
        }
        for backing in allocated {
            self.function_error(
                function.callable(),
                format!("unpublished array backing {backing} escapes its definition"),
            );
        }
        for produced in published.keys() {
            if function.storage(*produced).is_some_and(|storage| {
                matches!(
                    storage.kind,
                    MirStorageKind::ArrayProduced | MirStorageKind::ArraySlice
                )
            }) {
                self.function_error(
                    function.callable(),
                    format!("produced array storage {produced} is never consumed"),
                );
            }
        }
        for anchor in anchors {
            self.function_error(
                function.callable(),
                format!("array anchor {anchor} is not ended"),
            );
        }
        self.verify_array_owner_joins(function);
    }

    fn verify_array_owner_joins(&mut self, function: MirDefinitionRef<'_>) {
        let mut incoming = vec![None; function.body().blocks.len()];
        if function.body().entry.index() >= incoming.len() {
            return;
        }
        incoming[function.body().entry.index()] = Some(ArrayOwnerState::default());
        let mut pending = VecDeque::from([function.body().entry]);

        while let Some(block_id) = pending.pop_front() {
            let Some(block) = function.block(block_id) else {
                continue;
            };
            let Some(mut state) = incoming[block_id.index()].clone() else {
                continue;
            };
            for instruction in &block.instructions {
                state.apply(function, instruction);
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
                        &state,
                        &mut incoming,
                        &mut pending,
                    );
                    let mut failure_state = state.clone();
                    if let Some(MirInstruction::Array(MirArrayInstruction::Allocate {
                        backing,
                        ..
                    })) = block.instructions.last()
                    {
                        failure_state.backings.remove(backing);
                        failure_state.completed_backings.remove(backing);
                    }
                    self.merge_array_owner_state(
                        function,
                        block.id,
                        *failure_target,
                        &failure_state,
                        &mut incoming,
                        &mut pending,
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
                        &state,
                        &mut incoming,
                        &mut pending,
                    );
                    let mut complete_state = state.clone();
                    complete_state.completed_backings.insert(*backing);
                    self.merge_array_owner_state(
                        function,
                        block.id,
                        *complete_target,
                        &complete_state,
                        &mut incoming,
                        &mut pending,
                    );
                }
                MirTerminator::Return { .. }
                | MirTerminator::ReturnShared { .. }
                | MirTerminator::ReturnOptionalShared { .. } => {
                    if !state.backings.is_empty()
                        || !state.completed_backings.is_empty()
                        || !state.produced.is_empty()
                        || !state.anchors.is_empty()
                    {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "array owner state must be fully consumed at normal return",
                        );
                    }
                }
                MirTerminator::Terminate { .. } => {}
                _ => {
                    for target in terminator.successors() {
                        self.merge_array_owner_state(
                            function,
                            block.id,
                            target,
                            &state,
                            &mut incoming,
                            &mut pending,
                        );
                    }
                }
            }
        }
    }

    fn merge_array_owner_state(
        &mut self,
        function: MirDefinitionRef<'_>,
        predecessor: crate::mir::BlockId,
        target: crate::mir::BlockId,
        state: &ArrayOwnerState,
        incoming: &mut [Option<ArrayOwnerState>],
        pending: &mut VecDeque<crate::mir::BlockId>,
    ) {
        let Some(slot) = incoming.get_mut(target.index()) else {
            return;
        };
        match slot {
            None => {
                *slot = Some(state.clone());
                pending.push_back(target);
            }
            Some(existing) if existing != state => self.block_error(
                function.callable(),
                predecessor,
                format!("array owner state disagrees at control-flow join {target}"),
            ),
            Some(_) => {}
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ArrayOwnerState {
    backings: HashSet<StorageId>,
    completed_backings: HashSet<StorageId>,
    produced: HashSet<StorageId>,
    anchors: HashSet<StorageId>,
}

impl ArrayOwnerState {
    fn apply(&mut self, function: MirDefinitionRef<'_>, instruction: &MirInstruction) {
        if let MirInstruction::Call(call) = instruction {
            if let Some(destination) = &call.destination {
                let storage = destination.base.storage();
                if function
                    .storage(storage)
                    .is_some_and(|entry| entry.kind == MirStorageKind::ArrayProduced)
                {
                    self.produced.insert(storage);
                }
            }
        }
        let MirInstruction::Array(instruction) = instruction else {
            return;
        };
        match instruction {
            MirArrayInstruction::Allocate { backing, .. } => {
                self.backings.insert(*backing);
                self.completed_backings.remove(backing);
            }
            MirArrayInstruction::Publish {
                backing,
                destination,
                ..
            } => {
                if self.completed_backings.remove(backing) {
                    self.backings.remove(backing);
                }
                self.produced.insert(*destination);
            }
            MirArrayInstruction::PublishShared { backing, .. } => {
                if self.completed_backings.remove(backing) {
                    self.backings.remove(backing);
                }
            }
            MirArrayInstruction::SliceCopy { destination, .. } => {
                self.produced.insert(*destination);
            }
            MirArrayInstruction::Adopt { source, .. }
            | MirArrayInstruction::Replace { source, .. } => {
                self.produced.remove(source);
            }
            MirArrayInstruction::Release { owner, .. } => {
                self.produced.remove(&owner.base.storage());
            }
            MirArrayInstruction::AnchorBegin { anchor, .. } => {
                self.anchors.insert(*anchor);
            }
            MirArrayInstruction::AnchorEnd { anchor, .. } => {
                self.anchors.remove(anchor);
            }
            _ => {}
        }
    }
}
