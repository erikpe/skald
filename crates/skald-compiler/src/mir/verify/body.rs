//! Callable signature, storage, block, and terminator verification.

use std::collections::HashSet;

use crate::identity::BindingId;

use super::{
    super::model::{
        BlockId, MirAliasAccess, MirBasicBlock, MirDefinitionRef, MirParameter, MirParameterMode,
        MirStorageKind, MirTerminator, MirType, ValueId,
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
        self.verify_cleanup_liveness(function);
        self.verify_shared_ownership(function);

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
                    | (MirStorageKind::SharedAllocation, None)
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
            ) && !matches!(storage.ty, MirType::Class(_) | MirType::Shared(_))
            {
                self.function_error(
                    function.callable(),
                    format!("compiler-owned storage {} must have class type", storage.id),
                );
            }
            if storage.kind == MirStorageKind::SharedAllocation
                && !matches!(storage.ty, MirType::Class(_))
            {
                self.function_error(
                    function.callable(),
                    format!(
                        "shared allocation storage {} must have exact class type",
                        storage.id
                    ),
                );
            }
            if matches!(storage.ty, MirType::Shared(_))
                && !matches!(
                    storage.kind,
                    MirStorageKind::Local
                        | MirStorageKind::Parameter
                        | MirStorageKind::Temporary
                        | MirStorageKind::SharedAnchor
                        | MirStorageKind::Argument
                        | MirStorageKind::Return
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
            if let MirType::Shared(target) = storage.ty {
                self.verify_shared_target_declared(function.callable(), target);
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
        let receiver_slots: Vec<_> = function
            .storage_entries()
            .iter()
            .filter(|storage| storage.kind == MirStorageKind::Receiver)
            .collect();
        let Some(receiver) = function.receiver() else {
            if !receiver_slots.is_empty() {
                self.function_error(
                    function.callable(),
                    "top-level function cannot declare receiver storage",
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
                                | MirType::Interface(_)
                                | MirType::Obj
                                | MirType::Shared(_)
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
                            | MirType::Interface(_)
                            | MirType::Obj
                            | MirType::Shared(_)
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
