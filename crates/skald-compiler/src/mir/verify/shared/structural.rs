//! Instruction-local structural verification for shared-owner MIR.

use std::collections::HashSet;

use crate::identity::CallableId;

use super::super::{
    super::model::{
        BlockId, MirAliasAccess, MirBasicBlock, MirDefinitionRef, MirPlace, MirPlaceBase,
        MirPlaceProjection, MirSharedAdopt, MirSharedAllocate, MirSharedAllocationMode,
        MirSharedAllocationOrigin, MirSharedCast, MirSharedCastSource, MirSharedCastTransfer,
        MirSharedCopy, MirSharedFieldCopy, MirSharedFieldInitialize, MirSharedFieldReplace,
        MirSharedInitialize, MirSharedMove, MirSharedPublish, MirSharedRelease, MirSharedTarget,
        MirStorageKind, MirTerminationReason, MirTerminator, MirType, MirViewTarget, StorageId,
        ValueId,
    },
    context::Verifier,
    type_operations::TypeRelation,
};

impl<'mir> Verifier<'mir> {
    pub(in crate::mir::verify) fn shared_target_accepts(
        &self,
        expected: MirSharedTarget,
        actual: MirSharedTarget,
    ) -> bool {
        match expected {
            MirSharedTarget::Obj => true,
            MirSharedTarget::Class(expected) => matches!(
                actual,
                MirSharedTarget::Class(actual)
                    if actual == expected || self.program.is_ancestor(expected, actual)
            ),
            MirSharedTarget::Interface(expected) => match actual {
                MirSharedTarget::Class(actual) => {
                    self.program.conformance(actual, expected).is_some()
                }
                MirSharedTarget::Interface(actual) => actual == expected,
                MirSharedTarget::Obj | MirSharedTarget::Array(_) => false,
            },
            MirSharedTarget::Array(expected) => {
                matches!(actual, MirSharedTarget::Array(actual) if actual == expected)
            }
        }
    }

    pub(in crate::mir::verify) fn verify_shared_target_declared(
        &mut self,
        callable: CallableId,
        target: MirSharedTarget,
    ) {
        let declared = match target {
            MirSharedTarget::Obj => true,
            MirSharedTarget::Class(class) => self.program.class(class).is_some(),
            MirSharedTarget::Interface(interface) => self.program.interface(interface).is_some(),
            MirSharedTarget::Array(array) => self.program.array_type(array).is_some(),
        };
        if !declared {
            self.function_error(callable, format!("shared target {target} is not declared"));
        }
    }

    pub(in crate::mir::verify) fn verify_shared_allocate(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        allocation: &MirSharedAllocate,
    ) {
        if allocation.origin != MirSharedAllocationOrigin::New {
            self.block_error(
                function.callable(),
                block.id,
                "shared allocation does not originate from `new`",
            );
        }
        if self.program.class(allocation.class).is_none() {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "shared allocation class {} is not declared",
                    allocation.class
                ),
            );
        }
        self.verify_allocation_storage(
            function,
            block,
            allocation.allocation,
            Some(allocation.class),
        );
        if let MirSharedAllocationMode::Copy { source } = &allocation.mode {
            let source = self.verify_place(function, block, source);
            if source.map(|source| source.ty) != Some(MirType::Class(allocation.class)) {
                self.block_error(
                    function.callable(),
                    block.id,
                    "shared copy-allocation source must have the exact allocation class",
                );
            }
        }
    }

    pub(in crate::mir::verify) fn verify_shared_initialize(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        initialize: &MirSharedInitialize,
        defined: &HashSet<ValueId>,
    ) {
        let class = self.verify_allocation_storage(function, block, initialize.allocation, None);
        let Some(target) = self.program.initializer(initialize.target) else {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "shared initializer target {} is not declared",
                    initialize.target
                ),
            );
            return;
        };
        if class.is_some_and(|class| class != initialize.target.class()) {
            self.block_error(
                function.callable(),
                block.id,
                "shared initializer does not match the exact allocation class",
            );
        }
        self.verify_arguments(
            function,
            block,
            "shared initializer",
            &initialize.arguments,
            &target.parameters,
            defined,
        );
    }

    pub(in crate::mir::verify) fn verify_shared_publish(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        publish: &MirSharedPublish,
    ) {
        self.verify_allocation_storage(function, block, publish.allocation, None);
    }

    pub(in crate::mir::verify) fn verify_shared_adopt(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        adopt: &MirSharedAdopt,
    ) {
        let allocation_class =
            self.verify_allocation_storage(function, block, adopt.allocation, None);
        let destination = function.storage(adopt.destination);
        if destination.is_none() {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "shared adoption destination {} is not declared",
                    adopt.destination
                ),
            );
        }
        if !destination.is_some_and(|storage| {
            matches!(
                storage.kind,
                MirStorageKind::Local
                    | MirStorageKind::Temporary
                    | MirStorageKind::SharedAnchor
                    | MirStorageKind::Argument
                    | MirStorageKind::Return
            ) && allocation_class.is_some_and(|class| {
                matches!(
                    storage.ty,
                    MirType::Shared(target)
                        if self.shared_target_accepts(target, MirSharedTarget::Class(class))
                )
            })
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "shared adoption requires a compatible destination owner target",
            );
        }
    }

    pub(in crate::mir::verify) fn verify_shared_copy(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        copy: &MirSharedCopy,
    ) {
        let destination = function.storage(copy.destination);
        let source = function.storage(copy.source);
        if destination.is_none() || source.is_none() {
            self.block_error(
                function.callable(),
                block.id,
                "shared copy storage is not declared in this function",
            );
            return;
        }
        if !matches!(
            (destination, source),
            (Some(destination), Some(source))
                if matches!(
                    destination.kind,
                    MirStorageKind::Local
                        | MirStorageKind::Temporary
                        | MirStorageKind::SharedAnchor
                        | MirStorageKind::Argument
                        | MirStorageKind::Return
                )
                    && matches!(
                        source.kind,
                        MirStorageKind::Local
                            | MirStorageKind::Parameter
                            | MirStorageKind::Temporary
                            | MirStorageKind::Argument
                            | MirStorageKind::Return
                    )
                    && matches!(
                        (destination.ty, source.ty),
                        (MirType::Shared(expected), MirType::Shared(actual))
                            if self.shared_target_accepts(expected, actual)
                    )
        ) {
            self.block_error(
                function.callable(),
                block.id,
                "shared copy requires compatible source and destination owner storage",
            );
        }
    }

    pub(in crate::mir::verify) fn verify_shared_move(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        transfer: &MirSharedMove,
    ) {
        let destination = function.storage(transfer.destination);
        let source = function.storage(transfer.source);
        if !matches!(
            (destination, source),
            (Some(destination), Some(source))
                if matches!(
                    destination.kind,
                    MirStorageKind::Local | MirStorageKind::Parameter
                )
                    && source.kind == MirStorageKind::Temporary
                    && matches!(
                        (destination.ty, source.ty),
                        (MirType::Shared(expected), MirType::Shared(actual))
                            if self.shared_target_accepts(expected, actual)
                    )
        ) {
            self.block_error(
                function.callable(),
                block.id,
                "shared move requires a compatible temporary and local or parameter owner destination",
            );
        }
    }

    pub(in crate::mir::verify) fn verify_shared_field_copy(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        copy: &MirSharedFieldCopy,
    ) {
        let destination = function.storage(copy.destination);
        let source = self.verify_place(function, block, &copy.source);
        if !matches!(
            (destination, source),
            (Some(destination), Some(source))
                if matches!(
                    destination.kind,
                    MirStorageKind::Local
                        | MirStorageKind::Temporary
                        | MirStorageKind::SharedAnchor
                        | MirStorageKind::Argument
                        | MirStorageKind::Return
                )
                    && matches!(
                        (destination.ty, source.ty),
                        (MirType::Shared(expected), MirType::Shared(actual))
                            if self.shared_target_accepts(expected, actual)
                    )
                    && (matches!(
                            copy.source.projections.last(),
                            Some(
                                MirPlaceProjection::Field(_)
                                    | MirPlaceProjection::ArrayElement { .. }
                            )
                        )
                        || (matches!(copy.source.base, MirPlaceBase::StaticField(_))
                            && copy.source.projections.is_empty()))
        ) {
            self.block_error(
                function.callable(),
                block.id,
                "shared field copy requires a compatible shared field and fresh owner storage",
            );
        }
    }

    pub(in crate::mir::verify) fn verify_shared_cast(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        cast: &MirSharedCast,
        runtime: bool,
    ) {
        let destination = function.storage(cast.destination);
        if !destination.is_some_and(|storage| {
            matches!(
                storage.kind,
                MirStorageKind::Local
                    | MirStorageKind::Temporary
                    | MirStorageKind::SharedAnchor
                    | MirStorageKind::Argument
                    | MirStorageKind::Return
            ) && storage.ty == MirType::Shared(cast.target)
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "shared cast requires matching fresh owner destination storage",
            );
        }
        self.verify_shared_target_declared(function.callable(), cast.target);
        let source_target = cast.source.target();
        let valid_source = match &cast.source {
            MirSharedCastSource::Owner { storage, target } => {
                function.storage(*storage).is_some_and(|source| {
                    source.ty == MirType::Shared(*target)
                        && match cast.transfer {
                            MirSharedCastTransfer::Copy => {
                                matches!(
                                    source.kind,
                                    MirStorageKind::Local | MirStorageKind::Parameter
                                ) && cast.exact_dynamic_class.is_none()
                            }
                            MirSharedCastTransfer::Adopt => {
                                source.kind == MirStorageKind::Temporary
                            }
                        }
                })
            }
            MirSharedCastSource::Field { place, target } => {
                cast.transfer == MirSharedCastTransfer::Copy
                    && cast.exact_dynamic_class.is_none()
                    && self
                        .verify_place(function, block, place)
                        .is_some_and(|source| {
                            source.ty == MirType::Shared(*target)
                                && matches!(
                                    place.projections.last(),
                                    Some(MirPlaceProjection::Field(_))
                                )
                        })
            }
        };
        if !valid_source {
            self.block_error(
                function.callable(),
                block.id,
                "shared cast source provenance or copy/adopt operation is invalid",
            );
        }
        self.verify_shared_target_declared(function.callable(), source_target);
        let target = shared_view_target(cast.target);
        let relation = cast.exact_dynamic_class.map_or_else(
            || self.classify_type_relation(source_target.ty(), target),
            |class| {
                if self.class_provides_view(class, target) {
                    TypeRelation::StaticSuccess
                } else {
                    TypeRelation::StaticFailure
                }
            },
        );
        let expected = if runtime {
            TypeRelation::Runtime
        } else {
            TypeRelation::StaticSuccess
        };
        if relation != expected {
            self.block_error(
                function.callable(),
                block.id,
                if runtime {
                    "shared cast does not require a runtime metadata check"
                } else {
                    "static shared cast is not guaranteed to succeed"
                },
            );
        }
        if let Some(class) = cast.exact_dynamic_class {
            if self.program.class(class).is_none()
                || !self.class_can_inhabit_type(class, source_target.ty())
            {
                self.block_error(
                    function.callable(),
                    block.id,
                    "shared cast exact dynamic provenance cannot inhabit its source target",
                );
            }
        }
    }

    pub(in crate::mir::verify) fn verify_shared_cast_terminator(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        cast: &MirSharedCast,
        success_target: BlockId,
        failure_target: BlockId,
    ) {
        self.verify_shared_cast(function, block, cast, true);
        self.verify_block_target(function, block, success_target);
        self.verify_block_target(function, block, failure_target);
        if success_target == failure_target {
            self.block_error(
                function.callable(),
                block.id,
                "shared cast success and failure edges must differ",
            );
        }
        if !function.block(failure_target).is_some_and(|failure| {
            matches!(
                failure.terminator,
                Some(MirTerminator::Terminate {
                    reason: MirTerminationReason::ObjectCastFailure,
                    ..
                })
            )
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "shared cast failure edge must terminate with object-cast failure",
            );
        }
    }

    pub(in crate::mir::verify) fn verify_shared_field_initialize(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        initialize: &MirSharedFieldInitialize,
    ) {
        self.verify_shared_field_destination(
            function,
            block,
            &initialize.destination,
            initialize.source,
            true,
        );
    }

    pub(in crate::mir::verify) fn verify_shared_field_replace(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        replace: &MirSharedFieldReplace,
    ) {
        self.verify_shared_field_destination(
            function,
            block,
            &replace.destination,
            replace.source,
            false,
        );
    }

    fn verify_shared_field_destination(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: &MirPlace,
        source: StorageId,
        initialization: bool,
    ) {
        let field = self.verify_place(function, block, destination);
        let source = function.storage(source);
        let is_direct_field = matches!(
            destination.projections.last(),
            Some(MirPlaceProjection::Field(_))
        );
        let is_unpublished_array_element =
            matches!(
                destination.projections.as_slice(),
                [MirPlaceProjection::ArrayElement { .. }]
            ) && destination.base.local_storage().is_some_and(|backing| {
                function
                    .storage(backing)
                    .is_some_and(|storage| storage.kind == MirStorageKind::ArrayBacking)
            });
        let receiver_initialization = destination
            .base
            .local_storage()
            .is_some_and(|storage| function.receiver() == Some(storage))
            && matches!(
                function.callable(),
                CallableId::Initializer(_) | CallableId::CopyConstructor(_)
            );
        let static_initialization = initialization
            && field.is_some_and(|field| {
                self.is_static_initializer_destination(function, destination, field.ty)
            });
        let valid = matches!(
            (field, source),
            (Some(field), Some(source))
                if field.access == MirAliasAccess::Mutable
                    && matches!(field.ty, MirType::Shared(_))
                    && field.ty == source.ty
                    && source.kind == MirStorageKind::Temporary
                    && (is_direct_field
                        || (initialization && is_unpublished_array_element)
                        || static_initialization)
        );
        if !valid
            || (initialization
                && !receiver_initialization
                && !is_unpublished_array_element
                && !static_initialization)
            || (!initialization && receiver_initialization)
        {
            self.block_error(
                function.callable(),
                block.id,
                if initialization {
                    "shared owner initialization requires a mutable receiver field or array element and matching temporary owner"
                } else {
                    "shared field replacement requires a mutable shared field and matching temporary owner"
                },
            );
        }
    }

    pub(in crate::mir::verify) fn verify_shared_release(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        release: &MirSharedRelease,
    ) {
        if !function.storage(release.owner).is_some_and(|storage| {
            matches!(
                storage.kind,
                MirStorageKind::Local
                    | MirStorageKind::Parameter
                    | MirStorageKind::Temporary
                    | MirStorageKind::SharedAnchor
            ) && matches!(storage.ty, MirType::Shared(_))
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "shared release requires local, parameter, or temporary owner storage",
            );
        }
    }

    fn verify_allocation_storage(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        allocation: StorageId,
        expected_class: Option<crate::identity::ClassId>,
    ) -> Option<crate::identity::ClassId> {
        let Some(storage) = function.storage(allocation) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("shared allocation storage {allocation} is not declared"),
            );
            return None;
        };
        let MirType::Class(class) = storage.ty else {
            self.block_error(
                function.callable(),
                block.id,
                "shared allocation storage must have exact class type",
            );
            return None;
        };
        if storage.kind != MirStorageKind::SharedAllocation {
            self.block_error(
                function.callable(),
                block.id,
                "shared construction operation requires allocation storage",
            );
        }
        if expected_class.is_some_and(|expected| expected != class) {
            self.block_error(
                function.callable(),
                block.id,
                "shared allocation instruction has the wrong exact class",
            );
        }
        Some(class)
    }
}

const fn shared_view_target(target: MirSharedTarget) -> MirViewTarget {
    match target {
        MirSharedTarget::Obj => MirViewTarget::Obj,
        MirSharedTarget::Class(class) => MirViewTarget::Class(class),
        MirSharedTarget::Interface(interface) => MirViewTarget::Interface(interface),
        MirSharedTarget::Array(_) => panic!(),
    }
}
