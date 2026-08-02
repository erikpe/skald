//! Exact structural and type checks for standard-I/O MIR.

use std::collections::HashSet;

use super::{
    super::model::{
        MirAliasAccess, MirBasicBlock, MirDefinitionRef, MirIoBuffer, MirIoInstruction,
        MirIoOperation, MirStorageKind, MirType, ValueId,
    },
    context::Verifier,
};

impl Verifier<'_> {
    pub(super) fn verify_io_instruction(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        instruction: &MirIoInstruction,
        defined_values: &mut HashSet<ValueId>,
        defined_in_block: &mut HashSet<ValueId>,
    ) {
        match &instruction.operation {
            MirIoOperation::StandardHandle { stream } => {
                self.require_io_value(
                    function,
                    block,
                    *stream,
                    MirType::U8,
                    defined_in_block,
                    "stream selector",
                );
            }
            MirIoOperation::Open { path, mode } => {
                self.verify_io_buffer(function, block, path, MirAliasAccess::ReadOnly);
                self.require_io_value(
                    function,
                    block,
                    *mode,
                    MirType::U8,
                    defined_in_block,
                    "open mode",
                );
            }
            MirIoOperation::Read {
                handle,
                destination,
                offset,
            } => {
                self.require_io_value(
                    function,
                    block,
                    *handle,
                    MirType::I64,
                    defined_in_block,
                    "handle",
                );
                self.verify_io_buffer(function, block, destination, MirAliasAccess::Mutable);
                self.verify_io_offset(function, block, *offset);
            }
            MirIoOperation::Write {
                handle,
                source,
                offset,
            } => {
                self.require_io_value(
                    function,
                    block,
                    *handle,
                    MirType::I64,
                    defined_in_block,
                    "handle",
                );
                self.verify_io_buffer(function, block, source, MirAliasAccess::ReadOnly);
                self.verify_io_offset(function, block, *offset);
            }
            MirIoOperation::Close { handle } => {
                self.require_io_value(
                    function,
                    block,
                    *handle,
                    MirType::I64,
                    defined_in_block,
                    "handle",
                );
            }
        }

        let Some(result) = function.value(instruction.result) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("standard-I/O result {} is not declared", instruction.result),
            );
            return;
        };
        if result.ty != MirType::I64 {
            self.block_error(
                function.callable(),
                block.id,
                "standard-I/O result must have exact type `i64`",
            );
        }
        if !defined_values.insert(instruction.result) {
            self.block_error(
                function.callable(),
                block.id,
                format!("value {} is defined more than once", instruction.result),
            );
        }
        defined_in_block.insert(instruction.result);
    }

    fn require_io_value(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        value: ValueId,
        expected: MirType,
        defined: &HashSet<ValueId>,
        role: &str,
    ) {
        if self.verify_value_use(function, block, value, defined) != Some(expected) {
            self.block_error(
                function.callable(),
                block.id,
                format!("standard-I/O {role} must be a block-local `{expected}` value"),
            );
        }
    }

    fn verify_io_buffer(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        buffer: &MirIoBuffer,
        expected_access: MirAliasAccess,
    ) {
        let byte_array = self
            .program
            .array_type(buffer.array)
            .is_some_and(|array| array.element == MirType::U8);
        let place = self.verify_place(function, block, &buffer.place);
        let place_matches = place.is_some_and(|place| {
            place.ty == MirType::Array(buffer.array)
                && (expected_access == MirAliasAccess::ReadOnly
                    || place.access == MirAliasAccess::Mutable)
        });
        let anchor_matches = function
            .storage(buffer.anchor)
            .is_some_and(|storage| matches!(storage.kind, MirStorageKind::ArrayAnchor(_)));
        if !byte_array || buffer.access != expected_access || !place_matches || !anchor_matches {
            self.block_error(
                function.callable(),
                block.id,
                "standard-I/O buffer requires an exact `u8[]` place, compatible access, and matching array anchor",
            );
        }
    }

    fn verify_io_offset(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        offset: crate::mir::StorageId,
    ) {
        if function
            .storage(offset)
            .map(|storage| (storage.kind, storage.ty))
            != Some((MirStorageKind::ArrayPosition, MirType::U64))
        {
            self.block_error(
                function.callable(),
                block.id,
                "standard-I/O offset requires checked `u64` array-position storage",
            );
        }
    }
}
