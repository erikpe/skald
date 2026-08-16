//! Instruction-local structural verification for shared-owner MIR.

use std::collections::HashSet;

use crate::identity::CallableId;

use super::super::{
    super::model::{
        BlockId, MirAliasAccess, MirBasicBlock, MirDefinitionRef, MirPlace, MirPlaceProjection,
        MirSharedAdopt, MirSharedAllocate, MirSharedAllocationMode, MirSharedAllocationOrigin,
        MirSharedAllocationTarget, MirSharedCast, MirSharedCastSource, MirSharedCastTransfer,
        MirSharedCopy, MirSharedFieldCopy, MirSharedFieldInitialize, MirSharedFieldReplace,
        MirSharedInitialize, MirSharedMove, MirSharedPublish, MirSharedRelease, MirSharedTarget,
        MirStorageKind, MirTerminationReason, MirTerminator, MirType, MirViewTarget, StorageId,
        ValueId,
    },
    cell_write::CellWriteFamily,
    context::Verifier,
    type_operations::TypeRelation,
};

impl<'mir> Verifier<'mir> {
    pub(in crate::mir::verify) fn shared_target_accepts(
        &self,
        expected: MirSharedTarget,
        actual: MirSharedTarget,
    ) -> bool {
        if expected == actual {
            return true;
        }
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
                MirSharedTarget::Obj
                | MirSharedTarget::Array(_)
                | MirSharedTarget::OptionalBox(_) => false,
            },
            MirSharedTarget::Array(expected) => {
                matches!(actual, MirSharedTarget::Array(actual) if actual == expected)
            }
            MirSharedTarget::OptionalBox(expected) => {
                self.optional_box_target_accepts(expected, actual)
            }
        }
    }

    fn optional_box_target_accepts(
        &self,
        expected: crate::identity::OptionalBoxTypeId,
        actual: MirSharedTarget,
    ) -> bool {
        let MirSharedTarget::OptionalBox(actual) = actual else {
            return false;
        };
        let (Some(expected), Some(actual)) = (
            self.program.optional_box_type(expected),
            self.program.optional_box_type(actual),
        ) else {
            return false;
        };
        if expected.optional_depth != actual.optional_depth {
            return false;
        }
        let (Some(expected), Some(actual)) = (expected.object_view, actual.object_view) else {
            return false;
        };
        match expected {
            MirViewTarget::Obj => true,
            MirViewTarget::Class(expected) => matches!(
                actual,
                MirViewTarget::Class(actual)
                    if actual == expected || self.program.is_ancestor(expected, actual)
            ),
            MirViewTarget::Interface(expected) => match actual {
                MirViewTarget::Class(actual) => {
                    self.program.conformance(actual, expected).is_some()
                }
                MirViewTarget::Interface(actual) => actual == expected,
                MirViewTarget::Obj => false,
            },
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
            MirSharedTarget::OptionalBox(target) => {
                self.program.optional_box_type(target).is_some()
            }
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
        let expected_origin = match allocation.target {
            MirSharedAllocationTarget::Class(_) => MirSharedAllocationOrigin::New,
            MirSharedAllocationTarget::OptionalBox { .. } => MirSharedAllocationOrigin::OptionalBox,
        };
        if allocation.origin != expected_origin {
            let message = match allocation.target {
                MirSharedAllocationTarget::Class(_) => {
                    "shared allocation does not originate from `new`"
                }
                MirSharedAllocationTarget::OptionalBox { .. } => {
                    "optional-box allocation does not have optional-box origin"
                }
            };
            self.block_error(function.callable(), block.id, message);
        }
        let expected_type = match allocation.target {
            MirSharedAllocationTarget::Class(class) => {
                if matches!(allocation.mode, MirSharedAllocationMode::OptionalBox { .. }) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "ordinary shared allocation cannot use optional-box completion metadata",
                    );
                }
                if self.program.class(class).is_none() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("shared allocation class {class} is not declared"),
                    );
                }
                MirType::Class(class)
            }
            MirSharedAllocationTarget::OptionalBox { target, optional } => {
                if !matches!(allocation.mode, MirSharedAllocationMode::OptionalBox { .. }) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "optional-box allocation requires an explicit wrapper completion mode",
                    );
                }
                let valid = self
                    .program
                    .optional_box_type(target)
                    .is_some_and(|metadata| {
                        metadata.exact_optional == Some(optional)
                            && self.program.optional_type(optional).is_some()
                    });
                if !valid {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "optional-box allocation target does not name matching exact optional metadata",
                    );
                }
                MirType::Optional(optional)
            }
        };
        self.verify_allocation_storage(function, block, allocation.allocation, Some(expected_type));
        if let MirSharedAllocationMode::Copy { source } = &allocation.mode {
            let source = self.verify_place(function, block, source);
            if source.map(|source| source.ty) != Some(expected_type)
                || !matches!(allocation.target, MirSharedAllocationTarget::Class(_))
            {
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
        let allocation_type =
            self.verify_allocation_storage(function, block, initialize.allocation, None);
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
        if allocation_type != Some(MirType::Class(initialize.target.class())) {
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
        let allocation_type =
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
            ) && allocation_type
                .and_then(|ty| self.shared_target_for_allocation_payload(ty))
                .is_some_and(|actual| {
                    matches!(
                        storage.ty,
                        MirType::Shared(expected) if self.shared_target_accepts(expected, actual)
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
                        || (copy.source.base.static_field().is_some()
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
        let Some(target) = self.shared_object_view(cast.target) else {
            self.block_error(
                function.callable(),
                block.id,
                "shared cast target has no object-view capability",
            );
            return;
        };
        let source_view = self.shared_object_view(source_target);
        let relation = cast.exact_dynamic_class.map_or_else(
            || {
                source_view.map_or(TypeRelation::StaticFailure, |source| {
                    self.classify_type_relation(source.ty(), target)
                })
            },
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
                || !source_view
                    .is_some_and(|source| self.class_can_inhabit_type(class, source.ty()))
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
            false,
        );
    }

    pub(in crate::mir::verify) fn verify_shared_field_replace(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        replace: &MirSharedFieldReplace,
    ) {
        let cell_authorized = self.verify_cell_write_authorization(
            function,
            block,
            &replace.destination,
            replace.authorization,
            CellWriteFamily::Shared,
        );
        self.verify_shared_field_destination(
            function,
            block,
            &replace.destination,
            replace.source,
            false,
            cell_authorized,
        );
    }

    fn verify_shared_field_destination(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: &MirPlace,
        source: StorageId,
        initialization: bool,
        cell_authorized: bool,
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
        let static_replacement = !initialization
            && destination.base.static_field().is_some()
            && destination.projections.is_empty();
        let valid = matches!(
            (field, source),
            (Some(field), Some(source))
                if (field.access == MirAliasAccess::Mutable || cell_authorized)
                    && matches!(field.ty, MirType::Shared(_))
                    && field.ty == source.ty
                    && source.kind == MirStorageKind::Temporary
                    && (is_direct_field
                        || (initialization && is_unpublished_array_element)
                        || static_initialization
                        || static_replacement)
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
                    "shared owner replacement requires a mutable field or static and matching temporary owner"
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
        expected_type: Option<MirType>,
    ) -> Option<MirType> {
        let Some(storage) = function.storage(allocation) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("shared allocation storage {allocation} is not declared"),
            );
            return None;
        };
        if !matches!(storage.ty, MirType::Class(_) | MirType::Optional(_)) {
            self.block_error(
                function.callable(),
                block.id,
                "shared allocation storage must have an exact payload type",
            );
            return None;
        }
        if storage.kind != MirStorageKind::SharedAllocation {
            self.block_error(
                function.callable(),
                block.id,
                "shared construction operation requires allocation storage",
            );
        }
        if expected_type.is_some_and(|expected| expected != storage.ty) {
            self.block_error(
                function.callable(),
                block.id,
                "shared allocation instruction has the wrong exact payload type",
            );
        }
        Some(storage.ty)
    }

    fn shared_target_for_allocation_payload(&self, ty: MirType) -> Option<MirSharedTarget> {
        match ty {
            MirType::Class(class) => Some(MirSharedTarget::Class(class)),
            MirType::Optional(optional) => self
                .program
                .optional_box_types
                .iter()
                .find(|metadata| metadata.exact_optional == Some(optional))
                .map(|metadata| MirSharedTarget::OptionalBox(metadata.id)),
            _ => None,
        }
    }

    fn shared_object_view(&self, target: MirSharedTarget) -> Option<MirViewTarget> {
        match target {
            MirSharedTarget::Obj => Some(MirViewTarget::Obj),
            MirSharedTarget::Class(class) => Some(MirViewTarget::Class(class)),
            MirSharedTarget::Interface(interface) => Some(MirViewTarget::Interface(interface)),
            MirSharedTarget::OptionalBox(target) => self
                .program
                .optional_box_type(target)
                .and_then(|metadata| metadata.object_view),
            MirSharedTarget::Array(_) => None,
        }
    }
}
