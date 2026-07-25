//! Primitive optional structure and definite-initialization verification.

use std::collections::{HashSet, VecDeque};

use super::{
    super::model::{
        MirBasicBlock, MirDefinitionRef, MirInstruction, MirOptionalSource, MirPrimitiveType,
        MirRvalueKind, MirStorageKind, MirTerminator, MirType, StorageId, ValueId,
    },
    context::Verifier,
};

impl Verifier<'_> {
    pub(super) fn verify_optional_initialize(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: StorageId,
        source: MirOptionalSource,
        defined: &HashSet<ValueId>,
    ) {
        let payload = self.verify_optional_local(function, block, destination);
        self.verify_optional_source(function, block, source, payload, defined);
    }

    pub(super) fn verify_optional_assign(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: StorageId,
        source: MirOptionalSource,
        defined: &HashSet<ValueId>,
    ) {
        let payload = self.verify_optional_local(function, block, destination);
        self.verify_optional_source(function, block, source, payload, defined);
    }

    pub(super) fn verify_optional_presence(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        source: StorageId,
        result_type: MirType,
    ) {
        self.verify_optional_storage(function, block, source);
        if result_type != MirType::Bool {
            self.block_error(
                function.callable(),
                block.id,
                "optional presence test result is not `bool`",
            );
        }
    }

    pub(super) fn verify_optional_unwrap_terminator(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        source: StorageId,
        destination: StorageId,
        success_target: crate::mir::BlockId,
        failure_target: crate::mir::BlockId,
    ) {
        let payload = self.verify_optional_storage(function, block, source);
        let destination_valid = function.storage(destination).is_some_and(|storage| {
            storage.kind == MirStorageKind::OptionalUnwrap
                && Some(storage.ty) == payload.map(MirPrimitiveType::payload_type)
                && storage.source.is_none()
        });
        if !destination_valid {
            self.block_error(
                function.callable(),
                block.id,
                "optional unwrap destination must be matching compiler-owned scalar storage",
            );
        }
        self.verify_block_target(function, block, success_target);
        self.verify_block_target(function, block, failure_target);
        if success_target == failure_target {
            self.block_error(
                function.callable(),
                block.id,
                "optional unwrap success and failure edges must be distinct",
            );
        }
        if function.block(failure_target).is_some_and(|failure| {
            !failure.instructions.is_empty()
                || !matches!(
                    failure.terminator,
                    Some(MirTerminator::Terminate {
                        reason: crate::mir::MirTerminationReason::OptionalAccessFailure,
                        ..
                    })
                )
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "optional unwrap failure edge must terminate with optional-access failure",
            );
        }
    }

    pub(super) fn verify_optional_initialization(&mut self, function: MirDefinitionRef<'_>) {
        if function.body().entry.index() >= function.body().blocks.len() {
            return;
        }
        let mut incoming: Vec<Option<HashSet<StorageId>>> =
            vec![None; function.body().blocks.len()];
        incoming[function.body().entry.index()] = Some(HashSet::new());
        let mut pending = VecDeque::from([function.body().entry]);

        while let Some(block_id) = pending.pop_front() {
            let Some(block) = function.block(block_id) else {
                continue;
            };
            let Some(mut state) = incoming[block_id.index()].clone() else {
                continue;
            };
            apply_initializations(block, &mut state);
            for successor in block.terminator.iter().flat_map(MirTerminator::successors) {
                let Some(slot) = incoming.get_mut(successor.index()) else {
                    continue;
                };
                let changed = match slot {
                    Some(existing) => {
                        let merged: HashSet<_> = existing.intersection(&state).copied().collect();
                        if *existing == merged {
                            false
                        } else {
                            *existing = merged;
                            true
                        }
                    }
                    None => {
                        *slot = Some(state.clone());
                        true
                    }
                };
                if changed {
                    pending.push_back(successor);
                }
            }
        }

        for block in &function.body().blocks {
            let Some(Some(mut state)) = incoming.get(block.id.index()).cloned() else {
                continue;
            };
            for instruction in &block.instructions {
                match instruction {
                    MirInstruction::OptionalInitialize(initialize) => {
                        require_initialized_source(
                            self,
                            function,
                            block,
                            initialize.source,
                            &state,
                        );
                        if !state.insert(initialize.destination) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "optional local is initialized more than once",
                            );
                        }
                    }
                    MirInstruction::OptionalAssign(assignment) => {
                        require_initialized(
                            self,
                            function,
                            block,
                            assignment.destination,
                            &state,
                            "optional assignment destination",
                        );
                        require_initialized_source(
                            self,
                            function,
                            block,
                            assignment.source,
                            &state,
                        );
                    }
                    MirInstruction::Assign(assignment) => {
                        if let MirRvalueKind::OptionalPresence { source, .. } =
                            assignment.rvalue.kind
                        {
                            require_initialized(
                                self,
                                function,
                                block,
                                source,
                                &state,
                                "optional presence-test source",
                            );
                        }
                    }
                    _ => {}
                }
            }
            if let Some(MirTerminator::OptionalUnwrap { source, .. }) = &block.terminator {
                require_initialized(
                    self,
                    function,
                    block,
                    *source,
                    &state,
                    "optional unwrap source",
                );
            }
        }
    }

    fn verify_optional_local(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        storage: StorageId,
    ) -> Option<MirPrimitiveType> {
        let payload = self.verify_optional_storage(function, block, storage);
        if function
            .storage(storage)
            .is_some_and(|storage| storage.kind != MirStorageKind::Local)
        {
            self.block_error(
                function.callable(),
                block.id,
                "primitive optional operation destination must be local storage",
            );
        }
        payload
    }

    fn verify_optional_storage(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        storage: StorageId,
    ) -> Option<MirPrimitiveType> {
        let Some(storage) = function.storage(storage) else {
            self.block_error(
                function.callable(),
                block.id,
                "optional operation references undeclared storage",
            );
            return None;
        };
        let MirType::OptionalPrimitive(payload) = storage.ty else {
            self.block_error(
                function.callable(),
                block.id,
                "optional operation storage is not primitive optional storage",
            );
            return None;
        };
        Some(payload)
    }

    fn verify_optional_source(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        source: MirOptionalSource,
        expected: Option<MirPrimitiveType>,
        defined: &HashSet<ValueId>,
    ) {
        let actual = match source {
            MirOptionalSource::Absent => expected,
            MirOptionalSource::Present(value) => self
                .verify_value_use(function, block, value, defined)
                .and_then(primitive_from_type),
            MirOptionalSource::Copy(storage) => {
                self.verify_optional_storage(function, block, storage)
            }
        };
        if expected.is_some() && actual.is_some() && expected != actual {
            self.block_error(
                function.callable(),
                block.id,
                "optional source payload type does not match its destination",
            );
        }
    }
}

fn apply_initializations(block: &MirBasicBlock, state: &mut HashSet<StorageId>) {
    for instruction in &block.instructions {
        if let MirInstruction::OptionalInitialize(initialize) = instruction {
            state.insert(initialize.destination);
        }
    }
}

fn require_initialized_source(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    source: MirOptionalSource,
    state: &HashSet<StorageId>,
) {
    if let MirOptionalSource::Copy(storage) = source {
        require_initialized(
            verifier,
            function,
            block,
            storage,
            state,
            "optional copy source",
        );
    }
}

fn require_initialized(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    block: &MirBasicBlock,
    storage: StorageId,
    state: &HashSet<StorageId>,
    context: &'static str,
) {
    if !state.contains(&storage) {
        verifier.block_error(
            function.callable(),
            block.id,
            format!("{context} is not definitely initialized"),
        );
    }
}

fn primitive_from_type(ty: MirType) -> Option<MirPrimitiveType> {
    match ty {
        MirType::I64 => Some(MirPrimitiveType::I64),
        MirType::U64 => Some(MirPrimitiveType::U64),
        MirType::U8 => Some(MirPrimitiveType::U8),
        MirType::F64 => Some(MirPrimitiveType::F64),
        MirType::Bool => Some(MirPrimitiveType::Bool),
        _ => None,
    }
}
