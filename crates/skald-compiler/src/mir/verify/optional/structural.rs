//! Instruction-local optional structure, types, and failure-edge verification.

use std::collections::HashSet;

use super::super::{
    super::model::{
        MirAliasAccess, MirBasicBlock, MirDefinitionRef, MirOptionalSharedSource,
        MirOptionalSource, MirPlace, MirPrimitiveType, MirSharedTarget, MirStorageKind,
        MirTerminationReason, MirTerminator, MirType, StorageId, ValueId,
    },
    context::Verifier,
};

impl Verifier<'_> {
    pub(in crate::mir::verify) fn verify_optional_shared_operation(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: &MirPlace,
        source: &MirOptionalSharedSource,
        optional: crate::identity::OptionalTypeId,
        target: MirSharedTarget,
    ) {
        self.verify_shared_target_declared(function.callable(), target);
        if self
            .verify_place(function, block, destination)
            .map(|place| place.ty)
            != Some(MirType::Optional(optional))
            || self
                .program
                .optional_type(optional)
                .and_then(crate::mir::MirOptionalType::shared_owner)
                != Some(target)
        {
            self.block_error(
                function.callable(),
                block.id,
                "optional shared destination has the wrong exact target type",
            );
        }
        let actual = match source {
            MirOptionalSharedSource::Absent => return,
            MirOptionalSharedSource::Present(owner) => {
                function
                    .storage(*owner)
                    .and_then(|storage| match storage.ty {
                        MirType::Shared(target) => Some(target),
                        _ => None,
                    })
            }
            MirOptionalSharedSource::Move(owner) => function
                .storage(*owner)
                .and_then(|storage| self.optional_shared(storage.ty)),
            MirOptionalSharedSource::Copy(place) => self
                .verify_place(function, block, place)
                .and_then(|place| self.optional_shared(place.ty)),
        };
        if !actual.is_some_and(|actual| self.shared_target_accepts(target, actual)) {
            self.block_error(
                function.callable(),
                block.id,
                "optional shared source is not a compatible owner",
            );
        }
    }

    pub(in crate::mir::verify) fn verify_optional_shared_cleanup(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        cleanup: &crate::mir::MirOptionalSharedCleanup,
    ) {
        if self
            .verify_place(function, block, &cleanup.destination)
            .map(|place| place.ty)
            != Some(MirType::Optional(cleanup.optional))
            || self.optional_shared(MirType::Optional(cleanup.optional)) != Some(cleanup.target)
        {
            self.block_error(
                function.callable(),
                block.id,
                "optional shared cleanup has the wrong exact target type",
            );
        }
    }

    pub(in crate::mir::verify) fn verify_optional_shared_unwrap_terminator(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        unwrap: &crate::mir::MirOptionalSharedUnwrap,
        success_target: crate::mir::BlockId,
        failure_target: crate::mir::BlockId,
    ) {
        let source_valid = self
            .verify_place(function, block, &unwrap.source)
            .is_some_and(|place| place.ty == MirType::Optional(unwrap.optional))
            && self.optional_shared(MirType::Optional(unwrap.optional)) == Some(unwrap.target);
        let destination_valid = function.storage(unwrap.destination).is_some_and(|storage| {
            matches!(
                storage.kind,
                MirStorageKind::Temporary
                    | MirStorageKind::SharedAnchor
                    | MirStorageKind::Argument
                    | MirStorageKind::Return
            ) && storage.ty == MirType::Shared(unwrap.target)
                && storage.source.is_none()
        });
        if !source_valid || !destination_valid {
            self.block_error(
                function.callable(),
                block.id,
                "optional shared unwrap requires matching optional source and fresh shared owner",
            );
        }
        self.verify_block_target(function, block, success_target);
        self.verify_block_target(function, block, failure_target);
        if success_target == failure_target
            || !function.block(failure_target).is_some_and(|failure| {
                failure.instructions.is_empty()
                    && matches!(
                        failure.terminator,
                        Some(MirTerminator::Terminate {
                            reason: MirTerminationReason::OptionalAccessFailure,
                            ..
                        })
                    )
            })
        {
            self.block_error(
                function.callable(),
                block.id,
                "optional shared unwrap failure edge must be distinct and terminate with optional-access failure",
            );
        }
    }

    pub(in crate::mir::verify) fn verify_optional_view_begin(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        begin: &crate::mir::MirOptionalViewBegin,
        success_target: crate::mir::BlockId,
        absent_target: crate::mir::BlockId,
        overflow_target: crate::mir::BlockId,
    ) {
        if begin.guard.callable() != function.callable() {
            self.block_error(
                function.callable(),
                block.id,
                "optional guard belongs to another callable",
            );
        }
        if self
            .verify_place(function, block, &begin.source)
            .map(|place| place.ty)
            != Some(MirType::Optional(begin.optional))
            || self.optional_class(MirType::Optional(begin.optional)) != Some(begin.class)
        {
            self.block_error(
                function.callable(),
                block.id,
                "optional-view source has the wrong exact class type",
            );
        }
        for target in [success_target, absent_target, overflow_target] {
            self.verify_block_target(function, block, target);
        }
        if success_target == absent_target
            || success_target == overflow_target
            || absent_target == overflow_target
        {
            self.block_error(
                function.callable(),
                block.id,
                "optional-view success, absence, and overflow edges must be distinct",
            );
        }
        self.require_failure_edge(
            function,
            block,
            absent_target,
            MirTerminationReason::OptionalAccessFailure,
            "optional-view absence edge must terminate with optional-access failure",
        );
        self.require_failure_edge(
            function,
            block,
            overflow_target,
            MirTerminationReason::OptionalGuardOverflow,
            "optional-view overflow edge must terminate with optional-guard overflow",
        );
    }

    pub(in crate::mir::verify) fn verify_optional_mutation_check(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        source: &MirPlace,
        success_target: crate::mir::BlockId,
        failure_target: crate::mir::BlockId,
    ) {
        if !self
            .verify_place(function, block, source)
            .is_some_and(|place| self.optional_class(place.ty).is_some())
        {
            self.block_error(
                function.callable(),
                block.id,
                "optional mutation check requires exact-class optional storage",
            );
        }
        self.verify_block_target(function, block, success_target);
        self.verify_block_target(function, block, failure_target);
        if success_target == failure_target {
            self.block_error(
                function.callable(),
                block.id,
                "optional mutation success and failure edges must be distinct",
            );
        }
        self.require_failure_edge(
            function,
            block,
            failure_target,
            MirTerminationReason::OptionalPinnedMutation,
            "optional mutation failure edge must terminate with optional-pinned-mutation",
        );
    }

    pub(in crate::mir::verify) fn verify_optional_view_end(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        end: &crate::mir::MirOptionalViewEnd,
    ) {
        if end.guard.callable() != function.callable()
            || self
                .verify_place(function, block, &end.source)
                .map(|place| place.ty)
                != Some(MirType::Optional(end.optional))
            || self.optional_class(MirType::Optional(end.optional)) != Some(end.class)
        {
            self.block_error(
                function.callable(),
                block.id,
                "optional-view end has an incompatible guard root",
            );
        }
    }

    fn require_failure_edge(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        target: crate::mir::BlockId,
        reason: MirTerminationReason,
        message: &'static str,
    ) {
        if function.block(target).is_some_and(|failure| {
            !failure.instructions.is_empty()
                || !matches!(
                    failure.terminator,
                    Some(MirTerminator::Terminate { reason: actual, .. }) if actual == reason
                )
        }) {
            self.block_error(function.callable(), block.id, message);
        }
    }

    pub(in crate::mir::verify) fn verify_optional_initialize(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: &MirPlace,
        source: &MirOptionalSource,
        defined: &HashSet<ValueId>,
    ) {
        let payload = self.verify_optional_place(function, block, destination);
        self.verify_optional_source(function, block, source, payload, defined);
    }

    pub(in crate::mir::verify) fn verify_optional_assign(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: &MirPlace,
        source: &MirOptionalSource,
        defined: &HashSet<ValueId>,
    ) {
        let checked = self.verify_place(function, block, destination);
        if checked.is_some_and(|place| place.access != MirAliasAccess::Mutable) {
            self.block_error(
                function.callable(),
                block.id,
                "optional assignment destination requires mutable access",
            );
        }
        let payload = self.verify_optional_place(function, block, destination);
        self.verify_optional_source(function, block, source, payload, defined);
    }

    pub(in crate::mir::verify) fn verify_optional_presence(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        source: &MirPlace,
        result_type: MirType,
    ) {
        if !matches!(
            self.verify_place(function, block, source)
                .map(|place| place.ty),
            Some(MirType::Optional(_))
        ) {
            self.block_error(
                function.callable(),
                block.id,
                "optional presence-test source is not optional storage",
            );
        }
        if result_type != MirType::Bool {
            self.block_error(
                function.callable(),
                block.id,
                "optional presence test result is not `bool`",
            );
        }
    }

    pub(in crate::mir::verify) fn verify_optional_unwrap_terminator(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        source: &MirPlace,
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

    fn verify_optional_place(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        place: &MirPlace,
    ) -> Option<MirPrimitiveType> {
        self.verify_optional_storage(function, block, place)
    }

    fn verify_optional_storage(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        place: &MirPlace,
    ) -> Option<MirPrimitiveType> {
        let verified = self.verify_place(function, block, place);
        let Some(payload) = verified.and_then(|place| self.optional_primitive(place.ty)) else {
            self.block_error(
                function.callable(),
                block.id,
                "optional operation place is not primitive optional storage",
            );
            return None;
        };
        Some(payload)
    }

    fn verify_optional_source(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        source: &MirOptionalSource,
        expected: Option<MirPrimitiveType>,
        defined: &HashSet<ValueId>,
    ) {
        let actual = match source {
            MirOptionalSource::Absent => expected,
            MirOptionalSource::Present(value) => self
                .verify_value_use(function, block, *value, defined)
                .and_then(primitive_from_type),
            MirOptionalSource::Copy(place) => self.verify_optional_storage(function, block, place),
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
