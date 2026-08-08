use std::collections::HashSet;

use super::super::{
    super::model::{
        MirArrayInstruction, MirBasicBlock, MirDefinitionRef, MirStorageKind, MirType, ValueId,
    },
    context::Verifier,
};

impl Verifier<'_> {
    pub(in crate::mir::verify) fn verify_array_instruction_storage(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        instruction: &MirArrayInstruction,
        defined: &HashSet<ValueId>,
    ) {
        match instruction {
            MirArrayInstruction::Allocate {
                backing,
                array,
                length,
                ..
            } => {
                if function.storage(*backing).map(|s| (s.kind, s.ty))
                    != Some((MirStorageKind::ArrayBacking, MirType::Array(*array)))
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array allocation requires matching unpublished backing storage",
                    );
                }
                if self.verify_value_use(function, block, *length, defined) != Some(MirType::U64) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array allocation length must be a block-local `u64` value",
                    );
                }
            }
            MirArrayInstruction::AllocateElements {
                backing,
                prefix,
                array,
                ..
            } => {
                let storage_matches = function.storage(*backing).map(|s| (s.kind, s.ty))
                    == Some((MirStorageKind::ArrayBacking, MirType::Array(*array)))
                    && function.storage(*prefix).map(|s| (s.kind, s.ty))
                        == Some((MirStorageKind::ArrayPosition, MirType::U64));
                let executable_element = self.program.array_type(*array).is_some_and(|array| {
                    matches!(
                        array.element,
                        MirType::I64
                            | MirType::U64
                            | MirType::U8
                            | MirType::F64
                            | MirType::Bool
                            | MirType::Class(_)
                            | MirType::OptionalPrimitive(_)
                            | MirType::OptionalClass(_)
                    )
                });
                if !storage_matches || !executable_element {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array element-list allocation requires matching backing, prefix, and executable element type",
                    );
                }
            }
            MirArrayInstruction::InitializeElement {
                backing,
                prefix,
                value,
                ..
            } => {
                let element = function.storage(*backing).and_then(|storage| {
                    (storage.kind == MirStorageKind::ArrayBacking)
                        .then_some(storage.ty)
                        .and_then(|ty| match ty {
                            MirType::Array(array) => {
                                self.program.array_type(array).map(|entry| entry.element)
                            }
                            _ => None,
                        })
                });
                let prefix_matches = function.storage(*prefix).map(|s| (s.kind, s.ty))
                    == Some((MirStorageKind::ArrayPosition, MirType::U64));
                let value_matches = element.is_some_and(|element| {
                    matches!(
                        element,
                        MirType::I64 | MirType::U64 | MirType::U8 | MirType::F64 | MirType::Bool
                    ) && self.verify_value_use(function, block, *value, defined) == Some(element)
                });
                if !prefix_matches || !value_matches {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array element initialization requires exact primitive value, backing, and prefix types",
                    );
                }
            }
            MirArrayInstruction::CompleteElement {
                backing, prefix, ..
            } => {
                let lifecycle_element = function.storage(*backing).is_some_and(|storage| {
                    storage.kind == MirStorageKind::ArrayBacking
                        && match storage.ty {
                            MirType::Array(array) => {
                                self.program.array_type(array).is_some_and(|array| {
                                    matches!(
                                        array.element,
                                        MirType::Class(_)
                                            | MirType::OptionalPrimitive(_)
                                            | MirType::OptionalClass(_)
                                    )
                                })
                            }
                            _ => false,
                        }
                });
                let prefix_matches = function.storage(*prefix).map(|s| (s.kind, s.ty))
                    == Some((MirStorageKind::ArrayPosition, MirType::U64));
                if !lifecycle_element || !prefix_matches {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array element completion requires lifecycle-bearing backing and `u64` prefix storage",
                    );
                }
            }
            MirArrayInstruction::InitializeNext { backing, index, .. }
            | MirArrayInstruction::CopyNext { backing, index, .. } => {
                if function.storage(*backing).map(|s| s.kind) != Some(MirStorageKind::ArrayBacking)
                    || function.storage(*index).map(|s| (s.kind, s.ty))
                        != Some((MirStorageKind::ArrayPosition, MirType::U64))
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array prefix operation requires backing and `u64` index storage",
                    );
                }
                let array = function
                    .storage(*backing)
                    .and_then(|storage| match storage.ty {
                        MirType::Array(array) => self.program.array_type(array),
                        _ => None,
                    });
                let operation_matches = match (instruction, array) {
                    (MirArrayInstruction::InitializeNext { operation, .. }, Some(array)) => {
                        array.lifecycle.default == Some(*operation)
                    }
                    (MirArrayInstruction::CopyNext { operation, .. }, Some(array)) => {
                        array.lifecycle.copy == Some(*operation)
                    }
                    _ => false,
                };
                if !operation_matches {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array prefix operation does not match the declared element lifecycle",
                    );
                }
            }
            MirArrayInstruction::Publish {
                backing,
                destination,
                ..
            } => {
                let Some(source) = function.storage(*backing) else {
                    self.block_error(function.callable(), block.id, "array backing is undeclared");
                    return;
                };
                let valid = source.kind == MirStorageKind::ArrayBacking
                    && function.storage(*destination).is_some_and(|target| {
                        matches!(
                            target.kind,
                            MirStorageKind::ArrayProduced | MirStorageKind::ArraySlice
                        ) && target.ty == source.ty
                    });
                if !valid {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array publication requires matching produced descriptor storage",
                    );
                }
            }
            MirArrayInstruction::PublishShared {
                backing,
                destination,
                array,
                ..
            } => {
                let valid = function.storage(*backing).is_some_and(|source| {
                    source.kind == MirStorageKind::ArrayBacking
                        && source.ty == MirType::Array(*array)
                }) && function.storage(*destination).is_some_and(|target| {
                    target.ty == MirType::Shared(crate::mir::MirSharedTarget::Array(*array))
                });
                if !valid {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "shared array publication requires matching backing and owner storage",
                    );
                }
            }
            MirArrayInstruction::Normalize { destination, .. }
            | MirArrayInstruction::Offset { destination, .. }
            | MirArrayInstruction::Boundary { destination, .. } => {
                if function.storage(*destination).map(|s| (s.kind, s.ty))
                    != Some((MirStorageKind::ArrayPosition, MirType::U64))
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array normalization destination must be `u64` position storage",
                    );
                }
            }
            MirArrayInstruction::SliceBoundsCheck {
                start, end, array, ..
            } => {
                if !positions_match(function, [*start, *end]) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array slice-bounds check requires two `u64` position storages",
                    );
                }
                if self.program.array_type(*array).is_none() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array slice-bounds check references an undeclared array type",
                    );
                }
            }
            MirArrayInstruction::SliceLengthCheck {
                destination_start,
                destination_end,
                source,
                array,
                ..
            } => {
                if !positions_match(function, [*destination_start, *destination_end])
                    || self
                        .verify_place(function, block, source)
                        .map(|place| place.ty)
                        != Some(MirType::Array(*array))
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array slice-length check requires exact source and bound storage",
                    );
                }
            }
            MirArrayInstruction::SliceCopy {
                destination,
                source,
                start,
                end,
                array,
                operation,
                ..
            } => {
                let valid_destination = function.storage(*destination).is_some_and(|storage| {
                    storage.kind == MirStorageKind::ArraySlice
                        && storage.ty == MirType::Array(*array)
                });
                if !valid_destination
                    || !positions_match(function, [*start, *end])
                    || self
                        .verify_place(function, block, source)
                        .map(|place| place.ty)
                        != Some(MirType::Array(*array))
                    || self
                        .program
                        .array_type(*array)
                        .and_then(|entry| entry.lifecycle.copy)
                        != Some(*operation)
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array slice copy requires exact source, bounds, and slice storage",
                    );
                }
            }
            MirArrayInstruction::ElementAssign {
                destination,
                operation,
                ..
            } => {
                self.verify_array_assignment_lifecycle(function, block, destination, *operation);
            }
            MirArrayInstruction::SliceAssignNext {
                destination,
                source,
                destination_index,
                source_index,
                operation,
                ..
            } => {
                if !positions_match(function, [*destination_index, *source_index]) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array slice assignment requires two `u64` position storages",
                    );
                }
                let destination_array =
                    self.verify_place(function, block, destination)
                        .and_then(|place| match place.ty {
                            MirType::Array(array) => Some(array),
                            _ => None,
                        });
                if self
                    .verify_place(function, block, source)
                    .map(|place| place.ty)
                    != destination_array.map(MirType::Array)
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array slice assignment requires exact source and destination types",
                    );
                }
                self.verify_array_assignment_lifecycle(function, block, destination, *operation);
            }
            _ => {}
        }
    }

    fn verify_array_assignment_lifecycle(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: &crate::mir::MirPlace,
        operation: crate::mir::MirArrayAssignElement,
    ) {
        let array = destination
            .projections
            .iter()
            .rev()
            .find_map(|projection| match projection {
                crate::mir::MirPlaceProjection::ArrayElement { array, .. } => Some(*array),
                _ => None,
            })
            .or_else(|| {
                self.verify_place(function, block, destination)
                    .and_then(|place| match place.ty {
                        MirType::Array(array) => Some(array),
                        _ => None,
                    })
            });
        if array
            .and_then(|array| self.program.array_type(array))
            .and_then(|array| array.lifecycle.assignment)
            != Some(operation)
        {
            self.block_error(
                function.callable(),
                block.id,
                "array element write does not match the declared assignment lifecycle",
            );
        }
    }
}

fn positions_match(function: MirDefinitionRef<'_>, positions: [crate::mir::StorageId; 2]) -> bool {
    positions.into_iter().all(|position| {
        function
            .storage(position)
            .map(|storage| (storage.kind, storage.ty))
            == Some((MirStorageKind::ArrayPosition, MirType::U64))
    })
}
