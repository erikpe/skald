//! Ordered ordinary-instruction verification.

use std::collections::HashSet;

use super::{
    super::model::{
        MirAliasAccess, MirAssignment, MirBasicBlock, MirCopyAssignment, MirCopyConstruction,
        MirDefinitionRef, MirEndFullExpression, MirInstruction, MirPlace, MirPlaceBase, MirRvalue,
        MirRvalueKind, MirStorageKind, MirStore, MirType, ValueId,
    },
    context::Verifier,
    place::places_overlap,
};

#[derive(Clone, Copy)]
enum CopyOperationKind {
    Construction,
    Assignment,
}

impl Verifier<'_> {
    pub(super) fn verify_block(
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
            self.verify_instruction(
                function,
                block,
                instruction,
                defined_values,
                &mut defined_in_block,
            );
        }
        self.verify_terminator(return_type, function, block, &defined_in_block);
    }

    fn verify_instruction(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        instruction: &MirInstruction,
        defined_values: &mut HashSet<ValueId>,
        defined_in_block: &mut HashSet<ValueId>,
    ) {
        match instruction {
            MirInstruction::Assign(assignment) => self.verify_assignment(
                function,
                block,
                assignment,
                defined_values,
                defined_in_block,
            ),
            MirInstruction::Call(call) => {
                self.verify_call(function, block, call, defined_values, defined_in_block)
            }
            MirInstruction::Cleanup(cleanup) => {
                self.verify_cleanup_instruction(function, block, cleanup)
            }
            MirInstruction::Initialize(initialize) => {
                self.verify_initialize(function, block, initialize, defined_in_block)
            }
            MirInstruction::CopyConstruct(copy) => {
                self.verify_copy_construction(function, block, copy)
            }
            MirInstruction::CopyAssign(copy) => self.verify_copy_assignment(function, block, copy),
            MirInstruction::EndFullExpression(end) => {
                self.verify_full_expression_end(function, block, end)
            }
            MirInstruction::Store(store) => {
                self.verify_store(function, block, store, defined_in_block)
            }
            MirInstruction::BindNarrowedAlias(binding) => {
                self.verify_narrowed_alias_binding(function, block, binding, false)
            }
            MirInstruction::EndNarrowedAlias(end) => {
                self.verify_narrowed_alias_end(function, block, end)
            }
            MirInstruction::BindCheckedView(binding) => {
                self.verify_checked_view_binding(function, block, binding, false)
            }
            MirInstruction::EndCheckedView(end) => {
                self.verify_checked_view_end(function, block, end)
            }
        }
    }

    fn verify_assignment(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        assignment: &MirAssignment,
        defined_values: &mut HashSet<ValueId>,
        defined_in_block: &mut HashSet<ValueId>,
    ) {
        let Some(result) = function.value(assignment.result) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("assignment result {} is not declared", assignment.result),
            );
            return;
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
        self.verify_rvalue(function, block, &assignment.rvalue, defined_in_block);
        defined_values.insert(assignment.result);
        defined_in_block.insert(assignment.result);
    }

    fn verify_copy_construction(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        copy: &MirCopyConstruction,
    ) {
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

    fn verify_copy_assignment(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        copy: &MirCopyAssignment,
    ) {
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

    fn verify_full_expression_end(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        end: &MirEndFullExpression,
    ) {
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
            if destination.map(|place| place.ty) != Some(MirType::Class(cleanup.target)) {
                self.block_error(
                    function.callable(),
                    block.id,
                    "full-expression cleanup has the wrong class type",
                );
            }
        }
    }

    fn verify_store(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        store: &MirStore,
        defined_in_block: &HashSet<ValueId>,
    ) {
        let destination = self.verify_place(function, block, &store.destination);
        let storage_ty = destination.map(|place| place.ty);
        let value_ty = self.verify_value_use(function, block, store.value, defined_in_block);
        if storage_ty.is_some_and(|ty| !ty.is_scalar_value()) {
            self.block_error(
                function.callable(),
                block.id,
                "store destination must have scalar value type",
            );
        }
        if storage_ty.is_some() && value_ty.is_some() && storage_ty != value_ty {
            self.block_error(function.callable(), block.id, "store operand type mismatch");
        }
        if destination.is_some_and(|place| place.access != MirAliasAccess::Mutable) {
            self.block_error(
                function.callable(),
                block.id,
                "store destination requires mutable access",
            );
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
        if matches!(
            destination_place.base,
            MirPlaceBase::AliasParameter(_) | MirPlaceBase::NarrowedAlias(_)
        ) || destination_storage.is_some_and(|storage| {
            matches!(
                storage.kind,
                MirStorageKind::AliasParameter(_) | MirStorageKind::NarrowedAlias(_)
            )
        }) {
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
            MirRvalueKind::TypeTest { source, target } => {
                self.verify_type_test(function, block, rvalue, source, *target)
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
}

#[cfg(test)]
mod tests;
