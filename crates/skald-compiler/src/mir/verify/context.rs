//! Shared MIR verifier context.

use std::collections::HashSet;

use crate::identity::{BindingId, CallableId};

use super::{super::model::*, place::places_overlap, sink::ErrorSink, MirVerificationError};

pub(super) struct Verifier<'mir> {
    pub(super) program: &'mir MirProgram,
    pub(super) errors: ErrorSink,
}

#[derive(Clone, Copy)]
struct VerifiedPlace {
    ty: MirType,
    access: MirAliasAccess,
}

#[derive(Clone, Copy)]
enum CopyOperationKind {
    Construction,
    Assignment,
}

impl<'mir> Verifier<'mir> {
    pub(super) fn new(program: &'mir MirProgram) -> Self {
        Self {
            program,
            errors: ErrorSink::new(),
        }
    }

    pub(super) fn into_errors(self) -> Vec<MirVerificationError> {
        self.errors.into_errors()
    }

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
            if matches!(
                storage.kind,
                MirStorageKind::Return | MirStorageKind::Argument | MirStorageKind::Temporary
            ) && !matches!(storage.ty, MirType::Class(_))
            {
                self.function_error(
                    function.callable(),
                    format!("compiler-owned storage {} must have class type", storage.id),
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

    fn verify_block(
        &mut self,
        return_type: MirType,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        defined_values: &mut HashSet<ValueId>,
    ) {
        // MIR transient values are deliberately block-local before SSA. A
        // separate set per block prevents vector order from accidentally
        // permitting values to cross control-flow edges.
        let mut defined_in_block = HashSet::new();
        for instruction in &block.instructions {
            match instruction {
                MirInstruction::Assign(assignment) => {
                    let Some(result) = function.value(assignment.result) else {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("assignment result {} is not declared", assignment.result),
                        );
                        continue;
                    };
                    if defined_values.contains(&assignment.result) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("value {} is defined more than once", assignment.result),
                        );
                    }
                    if result.ty != assignment.rvalue.ty {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("assignment type does not match value {}", assignment.result),
                        );
                    }
                    self.verify_rvalue(function, block, &assignment.rvalue, &defined_in_block);
                    defined_values.insert(assignment.result);
                    defined_in_block.insert(assignment.result);
                }
                MirInstruction::Call(call) => {
                    self.verify_call(function, block, call, defined_values, &mut defined_in_block);
                }
                MirInstruction::Cleanup(cleanup) => {
                    let destination = self.verify_place(function, block, &cleanup.destination);
                    if matches!(cleanup.destination.base, MirPlaceBase::AliasParameter(_)) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "cleanup destination must be owning storage",
                        );
                    }
                    if function
                        .storage(cleanup.destination.base.storage())
                        .is_some_and(|storage| {
                            matches!(
                                storage.kind,
                                MirStorageKind::Return
                                    | MirStorageKind::Argument
                                    | MirStorageKind::Temporary
                            )
                        })
                    {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "return, caller argument, and temporary storage require their dedicated lifetime boundary",
                        );
                    }
                    if self.program.class(cleanup.target).is_none() {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("cleanup target {} is not declared", cleanup.target),
                        );
                    }
                    match destination.map(|place| place.ty) {
                        Some(MirType::Class(class)) if class != cleanup.target => {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "cleanup destination has the wrong class type",
                            );
                        }
                        Some(MirType::Class(_)) => {}
                        Some(_) => self.block_error(
                            function.callable(),
                            block.id,
                            "cleanup destination must have class type",
                        ),
                        None => {}
                    }
                    if destination.is_some_and(|place| place.access != MirAliasAccess::Mutable) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "cleanup destination requires mutable access",
                        );
                    }
                }
                MirInstruction::Initialize(initialize) => {
                    let destination = self.verify_place(function, block, &initialize.destination);
                    if matches!(initialize.destination.base, MirPlaceBase::AliasParameter(_)) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "initializer destination must be owning storage",
                        );
                    }
                    let Some(target) = self.program.initializer(initialize.target) else {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("initializer target {} is not declared", initialize.target),
                        );
                        continue;
                    };
                    if destination.map(|place| place.ty)
                        != Some(MirType::Class(initialize.target.class()))
                    {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "initializer destination has the wrong class type",
                        );
                    }
                    if destination.is_some_and(|place| place.access != MirAliasAccess::Mutable) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "initializer destination requires mutable access",
                        );
                    }
                    self.verify_arguments(
                        function,
                        block,
                        "initializer",
                        &initialize.arguments,
                        &target.parameters,
                        &defined_in_block,
                    );
                }
                MirInstruction::CopyConstruct(copy) => {
                    self.verify_copy_places(
                        function,
                        block,
                        &copy.destination,
                        &copy.source,
                        copy.class,
                        CopyOperationKind::Construction,
                    );
                    let selected = self
                        .program
                        .class(copy.class)
                        .and_then(|class| class.copy_constructor.selected());
                    if selected != Some(copy.operation) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "copy-construction operation does not match the class capability",
                        );
                    }
                }
                MirInstruction::CopyAssign(copy) => {
                    self.verify_copy_places(
                        function,
                        block,
                        &copy.destination,
                        &copy.source,
                        copy.class,
                        CopyOperationKind::Assignment,
                    );
                    let selected = self
                        .program
                        .class(copy.class)
                        .and_then(|class| class.copy_assignment.selected());
                    if selected != Some(copy.operation) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "copy-assignment operation does not match the class capability",
                        );
                    }
                }
                MirInstruction::EndFullExpression(end) => {
                    for cleanup in &end.temporaries {
                        let destination = self.verify_place(function, block, &cleanup.destination);
                        let is_temporary = function
                            .storage(cleanup.destination.base.storage())
                            .is_some_and(|storage| storage.kind == MirStorageKind::Temporary);
                        if !is_temporary
                            || !cleanup.destination.projections.is_empty()
                            || !matches!(cleanup.destination.base, MirPlaceBase::Storage(_))
                        {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "full-expression cleanup must name complete temporary storage",
                            );
                        }
                        if destination.map(|place| place.ty) != Some(MirType::Class(cleanup.target))
                        {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "full-expression cleanup has the wrong class type",
                            );
                        }
                    }
                }
                MirInstruction::Store(store) => {
                    let destination = self.verify_place(function, block, &store.destination);
                    let storage_ty = destination.map(|place| place.ty);
                    let value_ty =
                        self.verify_value_use(function, block, store.value, &defined_in_block);
                    if storage_ty.is_some_and(|ty| !ty.is_scalar_value()) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "store destination must have scalar value type",
                        );
                    }
                    if storage_ty.is_some() && value_ty.is_some() && storage_ty != value_ty {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "store operand type mismatch",
                        );
                    }
                    if destination.is_some_and(|place| place.access != MirAliasAccess::Mutable) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "store destination requires mutable access",
                        );
                    }
                }
            }
        }

        match &block.terminator {
            Some(MirTerminator::Return { value, .. }) => {
                if let Some(value) = value {
                    if let Some(ty) =
                        self.verify_value_use(function, block, *value, &defined_in_block)
                    {
                        if matches!(return_type, MirType::Unit | MirType::Class(_)) {
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
                } else if return_type != MirType::Unit && !matches!(return_type, MirType::Class(_))
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "value-returning function return has no operand",
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
                    self.verify_value_use(function, block, *condition, &defined_in_block)
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
            None => self.block_error(function.callable(), block.id, "block has no terminator"),
        }
    }

    fn verify_copy_places(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination_place: &MirPlace,
        source_place: &MirPlace,
        class: crate::identity::ClassId,
        operation: CopyOperationKind,
    ) {
        let destination = self.verify_place(function, block, destination_place);
        let source = self.verify_place(function, block, source_place);
        let construction = matches!(operation, CopyOperationKind::Construction);
        if self.program.class(class).is_none() {
            self.block_error(
                function.callable(),
                block.id,
                format!("copy operation class {class} is not declared"),
            );
        }
        if destination.map(|place| place.ty) != Some(MirType::Class(class))
            || source.map(|place| place.ty) != Some(MirType::Class(class))
        {
            self.block_error(
                function.callable(),
                block.id,
                "copy source and destination must have the exact operation class",
            );
        }
        if destination.is_some_and(|place| place.access != MirAliasAccess::Mutable) {
            self.block_error(
                function.callable(),
                block.id,
                "copy destination requires mutable access",
            );
        }
        let destination_storage = function.storage(destination_place.base.storage());
        if matches!(destination_place.base, MirPlaceBase::AliasParameter(_))
            || destination_storage
                .is_some_and(|storage| matches!(storage.kind, MirStorageKind::AliasParameter(_)))
        {
            self.block_error(
                function.callable(),
                block.id,
                if construction {
                    "copy-construction destination must be owning storage"
                } else {
                    "copy-assignment destination must be owning storage"
                },
            );
        }
        if !construction
            && destination_place.projections.is_empty()
            && function.receiver() == Some(destination_place.base.storage())
        {
            self.block_error(
                function.callable(),
                block.id,
                "copy assignment cannot replace the complete receiver",
            );
        }
        if construction && places_overlap(destination_place, source_place) {
            self.block_error(
                function.callable(),
                block.id,
                "copy-construction source and destination must not overlap",
            );
        }
    }

    fn verify_block_target(
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

    fn verify_call(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        call: &MirCall,
        defined_values: &mut HashSet<ValueId>,
        defined_in_block: &mut HashSet<ValueId>,
    ) {
        let arguments_defined = defined_in_block.clone();
        let result_ty = match call.result {
            Some(result) => {
                let metadata = function.value(result);
                if metadata.is_none() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("call result {result} is not declared"),
                    );
                }
                if !defined_values.insert(result) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("value {result} is defined more than once"),
                    );
                }
                defined_in_block.insert(result);
                metadata.map(|metadata| metadata.ty)
            }
            None => None,
        };

        let (parameters, return_type) = match call.target {
            MirCallTarget::Direct(target_id) => {
                if call.receiver.is_some() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "ordinary function call must not have a receiver",
                    );
                }
                let Some(target) = self.program.declarations.get(target_id) else {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("call target {target_id} is not declared"),
                    );
                    return;
                };
                (&target.parameters, target.return_type)
            }
            MirCallTarget::Method(target_id) => {
                let Some(target) = self.program.method(target_id) else {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("method target {target_id} is not declared"),
                    );
                    return;
                };
                match &call.receiver {
                    Some(receiver) => {
                        let receiver = self.verify_place(function, block, receiver);
                        if receiver.map(|place| place.ty) != Some(MirType::Class(target_id.class()))
                        {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "method receiver has the wrong class type",
                            );
                        }
                        if target.receiver_access == MirReceiverAccess::Mutable
                            && receiver.is_some_and(|place| place.access != MirAliasAccess::Mutable)
                        {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "mutable method receiver requires mutable access",
                            );
                        }
                    }
                    None => self.block_error(
                        function.callable(),
                        block.id,
                        "method call requires a receiver",
                    ),
                }
                (&target.parameters, target.return_type)
            }
        };
        self.verify_arguments(
            function,
            block,
            "call",
            &call.arguments,
            parameters,
            &arguments_defined,
        );

        let destination = call
            .destination
            .as_ref()
            .and_then(|place| self.verify_place(function, block, place));

        match (return_type, result_ty, destination) {
            (MirType::Unit, Some(_), _) => self.block_error(
                function.callable(),
                block.id,
                "unit-returning call must not have a result",
            ),
            (MirType::Unit, None, Some(_)) => self.block_error(
                function.callable(),
                block.id,
                "unit-returning call must not have a destination",
            ),
            (MirType::Unit, None, None) => {}
            (MirType::Class(_), Some(_), _) => self.block_error(
                function.callable(),
                block.id,
                "object-returning call must not have a scalar result",
            ),
            (MirType::Class(class), None, destination) => {
                let complete_destination = call.destination.as_ref().is_some_and(|place| {
                    place.projections.is_empty()
                        && matches!(place.base, MirPlaceBase::Storage(_))
                        && function
                            .storage(place.base.storage())
                            .is_some_and(|storage| {
                                matches!(
                                    storage.kind,
                                    MirStorageKind::Local | MirStorageKind::Temporary
                                )
                            })
                });
                if destination.map(|place| place.ty) != Some(MirType::Class(class))
                    || !complete_destination
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "object-returning call requires complete exact-class local or temporary destination storage",
                    );
                }
            }
            (_, Some(_), Some(_)) => self.block_error(
                function.callable(),
                block.id,
                "scalar-returning call must not have an object destination",
            ),
            (_, Some(result_ty), None) if result_ty != return_type => {
                self.block_error(function.callable(), block.id, "call result type mismatch")
            }
            (_, None, _) => self.block_error(
                function.callable(),
                block.id,
                "value-returning call has no result",
            ),
            _ => {}
        }
    }

    fn verify_rvalue(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        rvalue: &MirRvalue,
        defined: &HashSet<ValueId>,
    ) {
        match &rvalue.kind {
            MirRvalueKind::ConstantI64(_) => {
                if rvalue.ty != MirType::I64 {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "integer constant is not `i64`",
                    );
                }
            }
            MirRvalueKind::ConstantU64(_) => {
                if rvalue.ty != MirType::U64 {
                    self.block_error(function.callable(), block.id, "u64 constant is not `u64`");
                }
            }
            MirRvalueKind::ConstantU8(_) => {
                if rvalue.ty != MirType::U8 {
                    self.block_error(function.callable(), block.id, "u8 constant is not `u8`");
                }
            }
            MirRvalueKind::ConstantF64Bits(_) => {
                if rvalue.ty != MirType::F64 {
                    self.block_error(function.callable(), block.id, "f64 constant is not `f64`");
                }
            }
            MirRvalueKind::ConstantBool(_) => {
                if rvalue.ty != MirType::Bool {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "boolean constant is not `bool`",
                    );
                }
            }
            MirRvalueKind::Load(place) => {
                let place_ty = self
                    .verify_place(function, block, place)
                    .map(|place| place.ty);
                if place_ty.is_some_and(|ty| !ty.is_scalar_value()) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "load source must have scalar value type",
                    );
                }
                if place_ty.is_some() && place_ty != Some(rvalue.ty) {
                    self.block_error(function.callable(), block.id, "load result type mismatch");
                }
            }
            MirRvalueKind::Unary { operation, operand } => {
                let expected = operation.operand_type();
                if rvalue.ty != expected {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "unary operation result type mismatch",
                    );
                }
                self.verify_arithmetic_operand(function, block, *operand, expected, defined);
            }
            MirRvalueKind::Binary {
                operation,
                left,
                right,
            } => {
                let expected = operation.operand_type();
                if rvalue.ty != expected {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "binary operation result type mismatch",
                    );
                }
                self.verify_arithmetic_operand(function, block, *left, expected, defined);
                self.verify_arithmetic_operand(function, block, *right, expected, defined);
            }
        }
    }

    fn verify_arithmetic_operand(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        value: ValueId,
        expected: MirType,
        defined: &HashSet<ValueId>,
    ) {
        if let Some(ty) = self.verify_value_use(function, block, value, defined) {
            if ty != expected {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("arithmetic operand is not `{expected}`"),
                );
            }
        }
    }

    fn verify_arguments(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        kind: &str,
        arguments: &[MirArgument],
        parameters: &[MirParameter],
        defined: &HashSet<ValueId>,
    ) {
        if arguments.len() != parameters.len() {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "{kind} has {} arguments but requires {}",
                    arguments.len(),
                    parameters.len()
                ),
            );
        }
        let mut owned_arguments = HashSet::new();
        for (index, argument) in arguments.iter().enumerate() {
            let Some(parameter) = parameters.get(index) else {
                match argument {
                    MirArgument::Value(value) => {
                        self.verify_value_use(function, block, *value, defined);
                    }
                    MirArgument::Place(place) | MirArgument::OwnedPlace(place) => {
                        self.verify_place(function, block, place);
                    }
                }
                continue;
            };
            match (argument, parameter.mode) {
                (MirArgument::Value(value), MirParameterMode::Value) => {
                    let argument_ty = self.verify_value_use(function, block, *value, defined);
                    if argument_ty.is_some() && argument_ty != Some(parameter.ty) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("{kind} argument {index} type mismatch"),
                        );
                    }
                }
                (MirArgument::OwnedPlace(place), MirParameterMode::Value)
                    if matches!(parameter.ty, MirType::Class(_)) =>
                {
                    let argument = self.verify_place(function, block, place);
                    let complete_argument_storage = matches!(place.base, MirPlaceBase::Storage(_))
                        && place.projections.is_empty()
                        && function
                            .storage(place.base.storage())
                            .is_some_and(|storage| storage.kind == MirStorageKind::Argument);
                    if !complete_argument_storage {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!(
                                "{kind} argument {index} must transfer complete caller argument storage"
                            ),
                        );
                    }
                    if argument.is_some_and(|argument| argument.ty != parameter.ty) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("{kind} argument {index} type mismatch"),
                        );
                    }
                    if !owned_arguments.insert(place.clone()) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("{kind} argument {index} transfers storage more than once"),
                        );
                    }
                }
                (MirArgument::Place(place), MirParameterMode::ReadOnlyAlias)
                | (MirArgument::Place(place), MirParameterMode::MutableAlias) => {
                    let argument = self.verify_place(function, block, place);
                    if argument.is_some_and(|argument| argument.ty != parameter.ty) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("{kind} argument {index} type mismatch"),
                        );
                    }
                    if parameter.mode == MirParameterMode::MutableAlias
                        && argument
                            .is_some_and(|argument| argument.access != MirAliasAccess::Mutable)
                    {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("{kind} argument {index} requires mutable access"),
                        );
                    }
                }
                (MirArgument::Value(value), _) => {
                    self.verify_value_use(function, block, *value, defined);
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{kind} argument {index} must be a place"),
                    );
                }
                (MirArgument::Place(place), MirParameterMode::Value) => {
                    self.verify_place(function, block, place);
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{kind} argument {index} must be a scalar value or owned place"),
                    );
                }
                (MirArgument::OwnedPlace(place), _) => {
                    self.verify_place(function, block, place);
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{kind} argument {index} cannot transfer ownership"),
                    );
                }
            }
        }
    }

    fn verify_place(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        place: &MirPlace,
    ) -> Option<VerifiedPlace> {
        let storage_id = place.base.storage();
        let Some(storage) = function.storage(storage_id) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("place base {storage_id} is not declared in this function"),
            );
            return None;
        };
        let access = match (place.base, storage.kind) {
            (MirPlaceBase::Storage(_), MirStorageKind::AliasParameter(_)) => {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("alias parameter storage {storage_id} requires an indirect base"),
                );
                return None;
            }
            (MirPlaceBase::AliasParameter(_), MirStorageKind::AliasParameter(access)) => access,
            (MirPlaceBase::AliasParameter(_), _) => {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("indirect alias base {storage_id} is not alias parameter storage"),
                );
                return None;
            }
            (MirPlaceBase::Storage(_), _) => self.storage_access(function, storage),
        };
        let mut ty = storage.ty;
        for projection in &place.projections {
            match *projection {
                MirPlaceProjection::Field(field_id) => {
                    let MirType::Class(owner) = ty else {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("field projection {field_id} has a non-class base"),
                        );
                        return None;
                    };
                    if field_id.class() != owner {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("field projection {field_id} belongs to the wrong class"),
                        );
                        return None;
                    }
                    let Some(field) = self.program.field(field_id) else {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("field projection {field_id} is not declared"),
                        );
                        return None;
                    };
                    ty = field.ty;
                }
            }
        }
        Some(VerifiedPlace { ty, access })
    }

    fn storage_access(
        &self,
        function: MirDefinitionRef<'_>,
        storage: &MirStorage,
    ) -> MirAliasAccess {
        if storage.kind != MirStorageKind::Receiver {
            return MirAliasAccess::Mutable;
        }
        match function.callable() {
            CallableId::Method(method) => match self
                .program
                .method(method)
                .map(|method| method.receiver_access)
            {
                Some(MirReceiverAccess::ReadOnly) => MirAliasAccess::ReadOnly,
                Some(MirReceiverAccess::Mutable) => MirAliasAccess::Mutable,
                None => MirAliasAccess::ReadOnly,
            },
            CallableId::Initializer(_)
            | CallableId::CopyAssignment(_)
            | CallableId::Destructor(_) => MirAliasAccess::Mutable,
            CallableId::Function(_) => MirAliasAccess::ReadOnly,
        }
    }

    fn verify_value_use(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        value: ValueId,
        defined: &HashSet<ValueId>,
    ) -> Option<MirType> {
        let Some(metadata) = function.value(value) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("value {value} is not declared in this function"),
            );
            return None;
        };
        if !defined.contains(&value) {
            self.block_error(
                function.callable(),
                block.id,
                format!("value {value} is used before it is defined in this block"),
            );
        }
        Some(metadata.ty)
    }

    pub(super) fn program_error(&mut self, message: impl Into<String>) {
        self.errors.program(message);
    }

    pub(super) fn function_error(
        &mut self,
        callable: impl Into<CallableId>,
        message: impl Into<String>,
    ) {
        self.errors.callable(callable, message);
    }

    pub(super) fn block_error(
        &mut self,
        callable: impl Into<CallableId>,
        block: BlockId,
        message: impl Into<String>,
    ) {
        self.errors.block(callable, block, message);
    }
}
