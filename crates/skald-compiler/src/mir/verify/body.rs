//! Callable signature, storage, block, and terminator verification.

use std::collections::HashSet;

use crate::identity::BindingId;

use super::{
    super::model::{
        BlockId, MirAliasAccess, MirArrayFailure, MirArrayInstruction, MirBasicBlock,
        MirDefinitionRef, MirInstruction, MirParameter, MirParameterMode, MirStorageKind,
        MirTerminationReason, MirTerminator, MirType, ValueId,
    },
    context::Verifier,
};

impl<'mir> Verifier<'mir> {
    pub(super) fn verify_definition(
        &mut self,
        parameters: &[MirParameter],
        return_type: MirType,
        function: MirDefinitionRef<'mir>,
    ) {
        if function.class_owner() != function.callable().class() {
            self.function_error(
                function.callable(),
                "definition class owner differs from its callable identity",
            );
        }
        self.verify_storage(function);
        self.verify_values(function);
        self.verify_receiver(function);
        self.verify_parameters(parameters, function);
        self.verify_return_storage(return_type, function);
        self.verify_checked_view_definitions(function);

        if function.body().entry.callable() != function.callable() {
            self.function_error(
                function.callable(),
                format!(
                    "entry block {} is owned by another callable body",
                    function.body().entry
                ),
            );
        } else if function.block(function.body().entry).is_none() {
            self.function_error(
                function.callable(),
                format!("entry block {} is not declared", function.body().entry),
            );
        }

        let mut defined_values = HashSet::new();
        let mut seen_blocks = HashSet::new();
        for (index, block) in function.body().blocks.iter().enumerate() {
            if block.id.callable() != function.callable() {
                self.block_error(
                    function.callable(),
                    block.id,
                    "block is owned by another callable body",
                );
            }
            if block.id.index() != index {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("block table index {index} contains {}", block.id),
                );
            }
            if !seen_blocks.insert(block.id) {
                self.block_error(function.callable(), block.id, "duplicate block ID");
            }
            self.verify_block(return_type, function, block, &mut defined_values);
        }
        self.verify_path_conditions(function);
        self.verify_logical_expressions(function);
        self.verify_checked_shifts(function);
        self.verify_checked_integer_divisions(function);
        self.verify_checked_primitive_casts(function);
        self.verify_cleanup_liveness(function);
        self.verify_storage_lifetimes(function);
        self.verify_shared_ownership(function);
        self.verify_optional_initialization(function);
        self.verify_optional_guards(function);
        self.verify_array_ownership(function);
        self.verify_function_value_provenance(function);

        for value in function.values() {
            if !defined_values.contains(&value.id) {
                self.function_error(
                    function.callable(),
                    format!("value {} has no definition", value.id),
                );
            }
        }
    }

    fn verify_storage(&mut self, function: MirDefinitionRef<'_>) {
        let mut sources = HashSet::new();
        for (index, storage) in function.storage_entries().iter().enumerate() {
            if storage.id.callable() != function.callable() {
                self.function_error(
                    function.callable(),
                    format!("storage {} is owned by another callable body", storage.id),
                );
            }
            if storage.id.index() != index {
                self.function_error(
                    function.callable(),
                    format!("storage table index {index} contains {}", storage.id),
                );
            }
            if storage
                .source
                .is_some_and(|source| source.callable() != function.callable())
            {
                self.function_error(
                    function.callable(),
                    format!(
                        "storage {} has a source binding from another callable body",
                        storage.id
                    ),
                );
            }
            if storage.source.is_some_and(|source| !sources.insert(source)) {
                self.function_error(
                    function.callable(),
                    format!(
                        "source binding {} has multiple storage slots",
                        storage.source.expect("duplicate source must be present")
                    ),
                );
            }
            let source_matches_kind = matches!(
                (storage.kind, storage.source),
                (MirStorageKind::Receiver, Some(BindingId::Receiver(_)))
                    | (MirStorageKind::Parameter, Some(BindingId::Parameter(_)))
                    | (
                        MirStorageKind::AliasParameter(_),
                        Some(BindingId::Parameter(_))
                    )
                    | (MirStorageKind::Local, Some(BindingId::Local(_)))
                    | (MirStorageKind::Return, None)
                    | (MirStorageKind::Argument, None)
                    | (MirStorageKind::Temporary, None)
                    | (MirStorageKind::SharedAnchor, None)
                    | (MirStorageKind::CheckedView(_), None)
                    | (MirStorageKind::ScalarSpill, None)
                    | (MirStorageKind::PathCondition, None)
                    | (MirStorageKind::OptionalUnwrap, None)
                    | (MirStorageKind::SharedAllocation, None)
                    | (MirStorageKind::ArrayBacking, None)
                    | (MirStorageKind::ArrayProduced, None)
                    | (MirStorageKind::ArraySlice, None)
                    | (MirStorageKind::ArrayPosition, None)
                    | (MirStorageKind::ArrayAnchor(_), None)
                    | (MirStorageKind::ArrayAlias(_), None)
            );
            if !source_matches_kind {
                self.function_error(
                    function.callable(),
                    format!(
                        "storage {} kind does not match its source binding",
                        storage.id
                    ),
                );
            }
            if storage.ty == MirType::Unit {
                self.function_error(
                    function.callable(),
                    format!(
                        "storage {} cannot have payload-free type `unit`",
                        storage.id
                    ),
                );
            }
            if matches!(storage.ty, MirType::Interface(_) | MirType::Obj)
                && !matches!(
                    storage.kind,
                    MirStorageKind::AliasParameter(_) | MirStorageKind::CheckedView(_)
                )
            {
                self.function_error(
                    function.callable(),
                    format!(
                        "storage {} with a non-owning interface or `Obj` type must be an alias parameter",
                        storage.id
                    ),
                );
            }
            if matches!(
                storage.kind,
                MirStorageKind::Return
                    | MirStorageKind::Argument
                    | MirStorageKind::Temporary
                    | MirStorageKind::SharedAnchor
            ) && !matches!(
                storage.ty,
                MirType::Class(_) | MirType::Array(_) | MirType::Shared(_) | MirType::Optional(_)
            ) {
                self.function_error(
                    function.callable(),
                    format!(
                        "compiler-owned storage {} must have an owning aggregate type",
                        storage.id
                    ),
                );
            }
            if storage.kind == MirStorageKind::SharedAllocation
                && !matches!(storage.ty, MirType::Class(_) | MirType::Optional(_))
            {
                self.function_error(
                    function.callable(),
                    format!(
                        "shared allocation storage {} must have an exact payload type",
                        storage.id
                    ),
                );
            }
            if matches!(
                storage.kind,
                MirStorageKind::ArrayBacking
                    | MirStorageKind::ArrayProduced
                    | MirStorageKind::ArraySlice
                    | MirStorageKind::ArrayAnchor(_)
            ) && !matches!(storage.ty, MirType::Array(_))
            {
                self.function_error(
                    function.callable(),
                    format!("array storage {} must have exact array type", storage.id),
                );
            }
            if storage.kind == MirStorageKind::ArrayPosition && storage.ty != MirType::U64 {
                self.function_error(
                    function.callable(),
                    format!("array position storage {} must be `u64`", storage.id),
                );
            }
            if storage.kind == MirStorageKind::PathCondition && storage.ty != MirType::Bool {
                self.function_error(
                    function.callable(),
                    format!("path-condition storage {} must be `bool`", storage.id),
                );
            }
            if (matches!(storage.ty, MirType::Shared(_))
                || self.optional_shared(storage.ty).is_some())
                && !matches!(
                    storage.kind,
                    MirStorageKind::Local
                        | MirStorageKind::Parameter
                        | MirStorageKind::Temporary
                        | MirStorageKind::SharedAnchor
                        | MirStorageKind::Argument
                        | MirStorageKind::Return
                        | MirStorageKind::SharedAllocation
                )
            {
                self.function_error(
                    function.callable(),
                    format!(
                        "shared owner storage {} has an unsupported ownership role",
                        storage.id
                    ),
                );
            }
            if let MirType::Class(class) = storage.ty {
                if self.program.class(class).is_none() {
                    self.function_error(
                        function.callable(),
                        format!("storage {} has undeclared class type {class}", storage.id),
                    );
                }
            }
            if let MirType::Interface(interface) = storage.ty {
                if self.program.interface(interface).is_none() {
                    self.function_error(
                        function.callable(),
                        format!(
                            "storage {} has undeclared interface type {interface}",
                            storage.id
                        ),
                    );
                }
            }
            if let MirType::Array(array) = storage.ty {
                if self.program.array_type(array).is_none() {
                    self.function_error(
                        function.callable(),
                        format!("storage {} has undeclared array type {array}", storage.id),
                    );
                }
            }
            if let MirType::Shared(target) = storage.ty {
                self.verify_shared_target_declared(function.callable(), target);
            }
            if let Some(target) = self.optional_shared(storage.ty) {
                self.verify_shared_target_declared(function.callable(), target);
            }
            if let MirType::Optional(optional) = storage.ty {
                if self.program.optional_type(optional).is_none() {
                    self.function_error(
                        function.callable(),
                        format!(
                            "storage {} has undeclared optional type {optional}",
                            storage.id
                        ),
                    );
                }
            }
        }
    }

    fn verify_return_storage(&mut self, return_type: MirType, function: MirDefinitionRef<'_>) {
        let slots: Vec<_> = function
            .storage_entries()
            .iter()
            .filter(|storage| storage.kind == MirStorageKind::Return)
            .collect();
        match return_type {
            MirType::Array(array) => {
                let valid = function.return_storage().is_some_and(|return_storage| {
                    slots.len() == 1
                        && slots[0].id == return_storage
                        && slots[0].ty == MirType::Array(array)
                });
                if !valid {
                    self.function_error(
                        function.callable(),
                        "array-returning definition must identify exactly one matching return storage slot",
                    );
                }
            }
            MirType::Class(class) => {
                let Some(return_storage) = function.return_storage() else {
                    self.function_error(
                        function.callable(),
                        "object-returning definition has no return storage",
                    );
                    return;
                };
                let valid = slots.len() == 1
                    && slots[0].id == return_storage
                    && slots[0].ty == MirType::Class(class);
                if !valid {
                    self.function_error(
                        function.callable(),
                        "object-returning definition must identify exactly one matching return storage slot",
                    );
                }
            }
            MirType::Optional(optional) => {
                let Some(return_storage) = function.return_storage() else {
                    self.function_error(
                        function.callable(),
                        "optional-returning definition has no return storage",
                    );
                    return;
                };
                let valid = slots.len() == 1
                    && slots[0].id == return_storage
                    && slots[0].ty == MirType::Optional(optional);
                if !valid {
                    self.function_error(
                        function.callable(),
                        "optional-returning definition must identify exactly one matching return storage slot",
                    );
                }
            }
            MirType::Shared(target) => {
                let Some(return_storage) = function.return_storage() else {
                    self.function_error(
                        function.callable(),
                        "shared-returning definition has no return storage",
                    );
                    return;
                };
                let valid = slots.len() == 1
                    && slots[0].id == return_storage
                    && slots[0].ty == MirType::Shared(target);
                if !valid {
                    self.function_error(
                        function.callable(),
                        "shared-returning definition must identify exactly one matching return owner slot",
                    );
                }
            }
            _ if function.return_storage().is_some() || !slots.is_empty() => self.function_error(
                function.callable(),
                "non-object definition cannot declare return storage",
            ),
            _ => {}
        }
    }

    fn verify_values(&mut self, function: MirDefinitionRef<'_>) {
        for (index, value) in function.values().iter().enumerate() {
            if value.id.callable() != function.callable() {
                self.function_error(
                    function.callable(),
                    format!("value {} is owned by another callable body", value.id),
                );
            }
            if value.id.index() != index {
                self.function_error(
                    function.callable(),
                    format!("value table index {index} contains {}", value.id),
                );
            }
            if !value.ty.is_scalar_value() {
                self.function_error(
                    function.callable(),
                    format!("value {} must have a scalar value type", value.id),
                );
            }
        }
    }

    fn verify_parameters(&mut self, parameters: &[MirParameter], function: MirDefinitionRef<'_>) {
        if function.parameters().len() != parameters.len() {
            self.function_error(
                function.callable(),
                format!(
                    "definition has {} parameters but declaration requires {}",
                    function.parameters().len(),
                    parameters.len()
                ),
            );
        }
        let mut seen = HashSet::new();
        for (index, parameter) in function.parameters().iter().enumerate() {
            let Some(storage) = function.storage(*parameter) else {
                self.function_error(
                    function.callable(),
                    format!("parameter storage {parameter} is not declared"),
                );
                continue;
            };
            if !seen.insert(*parameter) {
                self.function_error(
                    function.callable(),
                    format!("duplicate parameter storage {parameter}"),
                );
            }
            if !matches!(storage.source, Some(BindingId::Parameter(_))) {
                self.function_error(
                    function.callable(),
                    format!("parameter {parameter} does not identify parameter storage"),
                );
            }
            if !matches!(storage.source, Some(BindingId::Parameter(id)) if id.index() == index) {
                self.function_error(
                    function.callable(),
                    format!("parameter position {index} has mismatched source binding"),
                );
            }
            let Some(descriptor) = parameters.get(index) else {
                continue;
            };
            let expected_kind = match descriptor.mode {
                MirParameterMode::Value => MirStorageKind::Parameter,
                MirParameterMode::ReadOnlyAlias => {
                    MirStorageKind::AliasParameter(MirAliasAccess::ReadOnly)
                }
                MirParameterMode::MutableAlias => {
                    MirStorageKind::AliasParameter(MirAliasAccess::Mutable)
                }
            };
            if storage.kind != expected_kind {
                self.function_error(
                    function.callable(),
                    format!("parameter position {index} storage mode differs from declaration"),
                );
            }
            if descriptor.ty != storage.ty {
                self.function_error(
                    function.callable(),
                    format!("parameter position {index} type differs from declaration"),
                );
            }
        }
        for storage in function.storage_entries().iter().filter(|storage| {
            matches!(
                storage.kind,
                MirStorageKind::Parameter | MirStorageKind::AliasParameter(_)
            )
        }) {
            if !seen.contains(&storage.id) {
                self.function_error(
                    function.callable(),
                    format!(
                        "parameter storage {} is not listed by the definition",
                        storage.id
                    ),
                );
            }
        }
    }

    fn verify_receiver(&mut self, function: MirDefinitionRef<'_>) {
        let receiver_required = match function.callable() {
            crate::identity::CallableId::Method(method) => {
                self.program.method(method).is_some_and(|method| {
                    matches!(
                        method.kind,
                        super::super::model::MirMethodKind::Instance { .. }
                    )
                })
            }
            crate::identity::CallableId::Initializer(_)
            | crate::identity::CallableId::CopyConstructor(_)
            | crate::identity::CallableId::CopyAssignment(_)
            | crate::identity::CallableId::Destructor(_) => true,
            crate::identity::CallableId::Function(_)
            | crate::identity::CallableId::StaticInitializer(_) => false,
        };
        let receiver_slots: Vec<_> = function
            .storage_entries()
            .iter()
            .filter(|storage| storage.kind == MirStorageKind::Receiver)
            .collect();
        if function.receiver().is_some() != receiver_required {
            let unexpected_receiver = match function.callable() {
                crate::identity::CallableId::Method(method)
                    if self.program.method(method).is_some_and(|method| {
                        method.kind == super::super::model::MirMethodKind::Static
                    }) =>
                {
                    "static member definition must not have a receiver"
                }
                _ => "receiverless callable definition must not identify a receiver",
            };
            self.function_error(
                function.callable(),
                if receiver_required {
                    "instance member definition requires a receiver"
                } else {
                    unexpected_receiver
                },
            );
        }
        let Some(receiver) = function.receiver() else {
            if !receiver_slots.is_empty() {
                self.function_error(
                    function.callable(),
                    "definition without a receiver cannot declare receiver storage",
                );
            }
            return;
        };
        let Some(storage) = function.storage(receiver) else {
            self.function_error(
                function.callable(),
                format!("receiver storage {receiver} is not declared"),
            );
            return;
        };
        if receiver_slots.len() != 1
            || storage.kind != MirStorageKind::Receiver
            || storage.source != Some(BindingId::Receiver(function.callable()))
        {
            self.function_error(
                function.callable(),
                "member definition must identify exactly one receiver storage slot",
            );
        }
        let expected = function
            .callable()
            .class()
            .map(MirType::Class)
            .expect("member callable has a class owner");
        if storage.ty != expected {
            self.function_error(
                function.callable(),
                "receiver storage has the wrong class type",
            );
        }
    }

    pub(super) fn verify_terminator(
        &mut self,
        return_type: MirType,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        defined_in_block: &HashSet<ValueId>,
    ) {
        match &block.terminator {
            Some(MirTerminator::Return { value, .. }) => {
                if let Some(value) = value {
                    if let Some(ty) =
                        self.verify_value_use(function, block, *value, defined_in_block)
                    {
                        if matches!(
                            return_type,
                            MirType::Unit
                                | MirType::Class(_)
                                | MirType::Array(_)
                                | MirType::Interface(_)
                                | MirType::Obj
                                | MirType::Shared(_)
                                | MirType::Optional(_)
                        ) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "unit and object returns must not have a scalar operand",
                            );
                        } else if ty != return_type {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "return operand type mismatch",
                            );
                        }
                    }
                } else if return_type != MirType::Unit
                    && !matches!(
                        return_type,
                        MirType::Class(_)
                            | MirType::Array(_)
                            | MirType::Interface(_)
                            | MirType::Obj
                            | MirType::Shared(_)
                            | MirType::Optional(_)
                    )
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "value-returning function return has no operand",
                    );
                }
            }
            Some(MirTerminator::ReturnShared { owner, .. }) => {
                let valid = matches!(return_type, MirType::Shared(_))
                    && function.return_storage() == Some(*owner)
                    && function.storage(*owner).is_some_and(|storage| {
                        storage.kind == MirStorageKind::Return && storage.ty == return_type
                    });
                if !valid {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "shared return must transfer the definition's matching return owner",
                    );
                }
            }
            Some(MirTerminator::ReturnOptionalShared { owner, .. }) => {
                let valid = self.optional_shared(return_type).is_some()
                    && function.return_storage() == Some(*owner)
                    && function.storage(*owner).is_some_and(|storage| {
                        storage.kind == MirStorageKind::Return && storage.ty == return_type
                    });
                if !valid {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "optional shared return must transfer the definition's matching return owner",
                    );
                }
            }
            Some(MirTerminator::Panic { message, .. }) => {
                let expected = self
                    .program
                    .string_language_item
                    .map(|item| MirType::Class(item.class));
                let actual = self
                    .verify_place(function, block, message)
                    .map(|place| place.ty);
                if expected.is_none() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "panic requires the canonical string language item",
                    );
                } else if actual != expected {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "panic message must be an exact canonical string place",
                    );
                }
            }
            Some(MirTerminator::Goto { target, .. }) => {
                self.verify_block_target(function, block, *target);
            }
            Some(MirTerminator::Branch {
                condition,
                true_target,
                false_target,
                ..
            }) => {
                if let Some(ty) =
                    self.verify_value_use(function, block, *condition, defined_in_block)
                {
                    if ty != MirType::Bool {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "branch condition is not `bool`",
                        );
                    }
                }
                self.verify_block_target(function, block, *true_target);
                self.verify_block_target(function, block, *false_target);
            }
            Some(MirTerminator::ShiftCountCheck {
                check,
                success_target,
                failure_target,
                ..
            }) => self.verify_shift_count_check(
                function,
                block,
                check,
                *success_target,
                *failure_target,
            ),
            Some(MirTerminator::IntegerDivisorCheck {
                check,
                success_target,
                failure_target,
                ..
            }) => self.verify_integer_divisor_check(
                function,
                block,
                check,
                *success_target,
                *failure_target,
            ),
            Some(MirTerminator::PrimitiveCastRangeCheck {
                check,
                success_target,
                failure_target,
                ..
            }) => self.verify_primitive_cast_range_check(
                function,
                block,
                check,
                *success_target,
                *failure_target,
            ),
            Some(MirTerminator::CheckedCast {
                binding,
                success_target,
                failure_target,
                ..
            }) => self.verify_checked_cast_terminator(
                function,
                block,
                binding,
                *success_target,
                *failure_target,
            ),
            Some(MirTerminator::SharedCast {
                cast,
                success_target,
                failure_target,
                ..
            }) => self.verify_shared_cast_terminator(
                function,
                block,
                cast,
                *success_target,
                *failure_target,
            ),
            Some(MirTerminator::OptionalUnwrap {
                source,
                destination,
                success_target,
                failure_target,
                ..
            }) => self.verify_optional_unwrap_terminator(
                function,
                block,
                source,
                *destination,
                *success_target,
                *failure_target,
            ),
            Some(MirTerminator::OptionalSharedUnwrap {
                unwrap,
                success_target,
                failure_target,
                ..
            }) => self.verify_optional_shared_unwrap_terminator(
                function,
                block,
                unwrap,
                *success_target,
                *failure_target,
            ),
            Some(MirTerminator::BeginOptionalView {
                begin,
                success_target,
                absent_target,
                overflow_target,
                ..
            }) => self.verify_optional_view_begin(
                function,
                block,
                begin,
                *success_target,
                *absent_target,
                *overflow_target,
            ),
            Some(MirTerminator::BeginOptionalBoxView {
                begin,
                success_target,
                absent_target,
                overflow_target,
                ..
            }) => self.verify_optional_box_view_begin(
                function,
                block,
                begin,
                *success_target,
                *absent_target,
                *overflow_target,
            ),
            Some(MirTerminator::CheckOptionalMutation {
                source,
                success_target,
                failure_target,
                ..
            }) => self.verify_optional_mutation_check(
                function,
                block,
                source,
                *success_target,
                *failure_target,
            ),
            Some(MirTerminator::ArrayPositionCheck {
                position,
                kind,
                success_target,
                failure_target,
                ..
            }) => {
                if function
                    .storage(*position)
                    .map(|storage| (storage.kind, storage.ty))
                    != Some((MirStorageKind::ArrayPosition, MirType::U64))
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array position check requires `u64` array-position storage",
                    );
                }
                self.verify_block_target(function, block, *success_target);
                self.verify_block_target(function, block, *failure_target);
                let expected = match kind {
                    crate::mir::MirArrayPositionKind::Element => {
                        crate::mir::MirTerminationReason::ArrayIndexOutOfBounds
                    }
                    crate::mir::MirArrayPositionKind::SliceBound => {
                        crate::mir::MirTerminationReason::ArrayInvalidSliceBounds
                    }
                    crate::mir::MirArrayPositionKind::RangeOffset => {
                        crate::mir::MirTerminationReason::ArrayIndexOutOfBounds
                    }
                };
                if !matches!(
                    function.block(*failure_target).and_then(|block| block.terminator.as_ref()),
                    Some(MirTerminator::Terminate { reason, .. }) if *reason == expected
                ) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array position failure edge must terminate with the matching reason",
                    );
                }
            }
            Some(MirTerminator::ArrayOperationCheck {
                failure,
                success_target,
                failure_target,
                ..
            }) => {
                self.verify_block_target(function, block, *success_target);
                self.verify_block_target(function, block, *failure_target);
                let operation_matches = match (failure, block.instructions.last()) {
                    (
                        MirArrayFailure::AllocationSize,
                        Some(MirInstruction::Array(MirArrayInstruction::Allocate {
                            failure: operation_failure,
                            ..
                        })),
                    ) => operation_failure == failure,
                    (
                        MirArrayFailure::AllocationSize,
                        Some(MirInstruction::Array(MirArrayInstruction::AllocateElements {
                            failure: operation_failure,
                            ..
                        })),
                    ) => operation_failure == failure,
                    (
                        MirArrayFailure::InvalidSliceBounds,
                        Some(MirInstruction::Array(MirArrayInstruction::SliceBoundsCheck {
                            ..
                        })),
                    )
                    | (
                        MirArrayFailure::SliceLengthMismatch,
                        Some(MirInstruction::Array(MirArrayInstruction::SliceLengthCheck {
                            ..
                        })),
                    ) => true,
                    _ => false,
                };
                if !operation_matches {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array operation check must immediately follow its matching checked operation",
                    );
                }
                let expected = match failure {
                    MirArrayFailure::AllocationSize => MirTerminationReason::ArrayAllocationFailure,
                    MirArrayFailure::IndexOutOfBounds => {
                        MirTerminationReason::ArrayIndexOutOfBounds
                    }
                    MirArrayFailure::InvalidSliceBounds => {
                        MirTerminationReason::ArrayInvalidSliceBounds
                    }
                    MirArrayFailure::SliceLengthMismatch => {
                        MirTerminationReason::ArraySliceLengthMismatch
                    }
                };
                if !matches!(
                    function.block(*failure_target).and_then(|block| block.terminator.as_ref()),
                    Some(MirTerminator::Terminate { reason, .. }) if *reason == expected
                ) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array operation failure edge must terminate with the matching reason",
                    );
                }
            }
            Some(MirTerminator::ArrayLoop {
                backing,
                index,
                length,
                body_target,
                complete_target,
                ..
            }) => {
                if function.storage(*backing).map(|storage| storage.kind)
                    != Some(MirStorageKind::ArrayBacking)
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array loop requires unpublished backing storage",
                    );
                }
                if function
                    .storage(*index)
                    .map(|storage| (storage.kind, storage.ty))
                    != Some((MirStorageKind::ArrayPosition, MirType::U64))
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array loop index storage must be `u64`",
                    );
                }
                if function.storage(*length).map(|storage| storage.ty) != Some(MirType::U64) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array loop length storage must be `u64`",
                    );
                }
                self.verify_block_target(function, block, *body_target);
                self.verify_block_target(function, block, *complete_target);
                let valid_body = function.block(*body_target).is_some_and(|body| {
                    (body.instructions.is_empty()
                        || body.instructions.iter().any(|instruction| {
                        matches!(
                            instruction,
                            crate::mir::MirInstruction::Array(
                                crate::mir::MirArrayInstruction::InitializeNext { index: body_index, .. }
                                    | crate::mir::MirArrayInstruction::CopyNext { index: body_index, .. }
                            ) if body_index == index
                        )
                    }))
                    && body.instructions.iter().all(|instruction| {
                        !matches!(
                            instruction,
                            crate::mir::MirInstruction::Array(
                                crate::mir::MirArrayInstruction::InitializeNext {
                                    backing: body_backing,
                                    ..
                                }
                                | crate::mir::MirArrayInstruction::CopyNext {
                                    backing: body_backing,
                                    ..
                                }
                            ) if body_backing != backing
                        )
                    })
                    && matches!(
                        body.terminator,
                        Some(MirTerminator::Goto { target, .. }) if target == block.id
                    )
                });
                if !valid_body {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array loop body must advance its prefix and return to the loop header",
                    );
                }
            }
            Some(MirTerminator::Terminate { .. }) => {}
            None => self.block_error(function.callable(), block.id, "block has no terminator"),
        }
    }

    pub(super) fn verify_block_target(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        target: BlockId,
    ) {
        if target.callable() != function.callable() {
            self.block_error(
                function.callable(),
                block.id,
                format!("control-flow target {target} is owned by another callable body"),
            );
        } else if function.block(target).is_none() {
            self.block_error(
                function.callable(),
                block.id,
                format!("control-flow target {target} is not declared"),
            );
        }
    }
}

#[cfg(test)]
mod tests;
