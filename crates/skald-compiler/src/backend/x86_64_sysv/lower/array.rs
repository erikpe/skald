//! Primitive inline-array instruction and control-flow selection.

use crate::{
    backend::{BackendError, Target},
    mir::{
        MirArrayFailure, MirArrayInstruction, MirArrayPositionKind, MirPlace, MirPlaceProjection,
        MirTerminationReason, MirTerminator, MirType,
    },
};

use super::{
    super::{
        frame::FramePlace,
        layout::{ARRAY_LENGTH_OFFSET, ARRAY_OWNER_COUNT_OFFSET},
        machine::{Instruction, Label, Operand, Register},
        symbol,
    },
    block_label, value, InstructionSelector,
};

mod helpers;

pub(super) fn lower_helpers(
    program: &crate::mir::MirProgram,
    data_layout: &super::super::layout::DataLayout,
) -> Result<Vec<super::super::machine::AssemblyFunction>, BackendError> {
    helpers::lower_all(program, data_layout)
}

const RUNTIME_ALLOC: &str = "ska_rt_alloc";

impl InstructionSelector<'_, '_> {
    pub(super) fn select_array_copy_construction(
        &mut self,
        destination: &MirPlace,
        source: &MirPlace,
        array: crate::identity::ArrayTypeId,
    ) -> Result<(), BackendError> {
        self.clone_array_preserving_destination(destination, source, array)?;
        value::store_rax(value::memory(Register::Rdx, 0), self.output);
        Ok(())
    }

    pub(super) fn select_array_copy_assignment(
        &mut self,
        destination: &MirPlace,
        source: &MirPlace,
        array: crate::identity::ArrayTypeId,
    ) -> Result<(), BackendError> {
        self.clone_array_preserving_destination(destination, source, array)?;
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rdx, 0),
            destination: Register::Rdi.into(),
        });
        value::store_rax(value::memory(Register::Rdx, 0), self.output);
        self.output
            .push(Instruction::Call(symbol::array_release(array)));
        Ok(())
    }

    pub(super) fn select_array_field_cleanup(
        &mut self,
        owner: &MirPlace,
        array: crate::identity::ArrayTypeId,
    ) -> Result<(), BackendError> {
        let (_, owner) = self.frame_place(owner)?;
        value::load_rax(owner, self.output);
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdi.into(),
        });
        self.output
            .push(Instruction::Call(symbol::array_release(array)));
        Ok(())
    }

    pub(super) fn select_array_instruction(
        &mut self,
        instruction: &MirArrayInstruction,
    ) -> Result<(), BackendError> {
        match instruction {
            MirArrayInstruction::Allocate {
                backing,
                array,
                length,
                ..
            } => self.select_array_allocate(*backing, *array, *length),
            MirArrayInstruction::InitializeNext {
                backing,
                index,
                operation: crate::mir::MirArrayDefaultElement::Primitive,
                ..
            } => {
                let array = self.array_for_storage(*backing)?;
                value::load_rax(value::frame_storage(self.frame, *backing), self.output);
                self.output.push(Instruction::Move {
                    source: Register::Rax.into(),
                    destination: Register::Rdi.into(),
                });
                value::load_rax(value::frame_storage(self.frame, *index), self.output);
                self.output.push(Instruction::Move {
                    source: Register::Rax.into(),
                    destination: Register::Rsi.into(),
                });
                self.output
                    .push(Instruction::Call(symbol::array_initialize_element(array)));
                self.advance_array_index(*index);
                Ok(())
            }
            MirArrayInstruction::CopyNext {
                backing,
                source,
                index,
                operation: crate::mir::MirArrayCopyElement::Primitive,
                ..
            } => {
                let array = self.array_for_storage(*backing)?;
                let (_, source) = self.frame_place(source)?;
                value::load_rax(value::frame_storage(self.frame, *backing), self.output);
                self.output.push(Instruction::Move {
                    source: Register::Rax.into(),
                    destination: Register::Rdi.into(),
                });
                value::load_rax(source, self.output);
                self.output.push(Instruction::Move {
                    source: Register::Rax.into(),
                    destination: Register::Rsi.into(),
                });
                value::load_rax(value::frame_storage(self.frame, *index), self.output);
                self.output.push(Instruction::Move {
                    source: Register::Rax.into(),
                    destination: Register::Rdx.into(),
                });
                self.output
                    .push(Instruction::Call(symbol::array_copy_element(array)));
                self.advance_array_index(*index);
                Ok(())
            }
            MirArrayInstruction::Publish {
                backing,
                destination,
                ..
            } => {
                value::load_rax(value::frame_storage(self.frame, *backing), self.output);
                value::store_rax(value::frame_storage(self.frame, *destination), self.output);
                self.clear_storage(*backing);
                Ok(())
            }
            MirArrayInstruction::Adopt {
                destination,
                source,
                ..
            } => {
                let (_, destination) = self.frame_place(destination)?;
                value::load_rax(value::frame_storage(self.frame, *source), self.output);
                value::store_rax(destination, self.output);
                self.clear_storage(*source);
                Ok(())
            }
            MirArrayInstruction::Replace {
                destination,
                source,
                array,
                ..
            } => {
                let (_, destination) = self.frame_place(destination)?;
                value::load_rax(destination, self.output);
                self.output.push(Instruction::Move {
                    source: Register::Rax.into(),
                    destination: Register::Rdi.into(),
                });
                value::load_rax(value::frame_storage(self.frame, *source), self.output);
                value::store_rax(destination, self.output);
                self.clear_storage(*source);
                self.output
                    .push(Instruction::Call(symbol::array_release(*array)));
                Ok(())
            }
            MirArrayInstruction::Release { owner, array, .. } => {
                let (_, source) = self.frame_place(owner)?;
                value::load_rax(source, self.output);
                self.output.push(Instruction::Move {
                    source: Register::Rax.into(),
                    destination: Register::Rdi.into(),
                });
                self.output
                    .push(Instruction::Call(symbol::array_release(*array)));
                self.clear_place(owner)
            }
            MirArrayInstruction::AnchorBegin { anchor, owner, .. } => {
                let (_, owner) = self.frame_place(owner)?;
                value::load_rax(owner, self.output);
                value::store_rax(value::frame_storage(self.frame, *anchor), self.output);
                Ok(())
            }
            MirArrayInstruction::AnchorEnd { anchor, .. } => {
                self.clear_storage(*anchor);
                Ok(())
            }
            MirArrayInstruction::Normalize {
                destination,
                owner,
                index,
                kind: MirArrayPositionKind::Element,
                ..
            } => self.select_array_element_normalize(*destination, owner, *index),
            _ => Err(self
                .array_error("array instruction escaped the primitive inline legality boundary")),
        }
    }

    pub(super) fn select_array_length(
        &mut self,
        source: &MirPlace,
        result: crate::mir::ValueId,
    ) -> Result<(), BackendError> {
        let (_, source) = self.frame_place(source)?;
        let empty = self.array_label(result.index(), "length_empty");
        let complete = self.array_label(result.index(), "length_complete");
        value::load_rax(source, self.output);
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(empty.clone()));
        value::load_rax(
            value::memory(Register::Rax, ARRAY_LENGTH_OFFSET),
            self.output,
        );
        self.output.push(Instruction::Jump(complete.clone()));
        self.output.push(Instruction::Label(empty));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rax,
        });
        self.output.push(Instruction::Label(complete));
        value::store_rax(value::frame_value(self.frame, result), self.output);
        Ok(())
    }

    pub(super) fn select_array_terminator(
        &mut self,
        terminator: &MirTerminator,
    ) -> Result<bool, BackendError> {
        match terminator {
            MirTerminator::ArrayOperationCheck {
                failure: MirArrayFailure::AllocationSize,
                success_target,
                failure_target,
                ..
            } => {
                self.output.push(Instruction::Test(Register::R11));
                self.output
                    .push(Instruction::JumpIfNotZero(block_label(*success_target)));
                self.output
                    .push(Instruction::Jump(block_label(*failure_target)));
                Ok(true)
            }
            MirTerminator::ArrayLoop {
                index,
                length,
                body_target,
                complete_target,
                ..
            } => {
                value::load_rax(value::frame_storage(self.frame, *index), self.output);
                self.output.push(Instruction::Move {
                    source: value::frame_storage(self.frame, *length),
                    destination: Register::R11.into(),
                });
                self.output.push(Instruction::Compare {
                    source: Register::R11,
                    destination: Register::Rax,
                });
                self.output
                    .push(Instruction::JumpIfBelow(block_label(*body_target)));
                self.output
                    .push(Instruction::Jump(block_label(*complete_target)));
                Ok(true)
            }
            MirTerminator::ArrayPositionCheck {
                position,
                kind: MirArrayPositionKind::Element,
                success_target,
                failure_target,
                ..
            } => {
                value::load_rax(value::frame_storage(self.frame, *position), self.output);
                self.output.push(Instruction::MoveImmediate64 {
                    bits: u64::MAX,
                    destination: Register::R11,
                });
                self.output.push(Instruction::Compare {
                    source: Register::R11,
                    destination: Register::Rax,
                });
                self.output
                    .push(Instruction::JumpIfNotZero(block_label(*success_target)));
                self.output
                    .push(Instruction::Jump(block_label(*failure_target)));
                Ok(true)
            }
            MirTerminator::Terminate {
                reason:
                    MirTerminationReason::ArrayAllocationFailure
                    | MirTerminationReason::ArrayIndexOutOfBounds,
                ..
            } => {
                self.output.push(Instruction::Trap);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn select_array_allocate(
        &mut self,
        backing: crate::mir::StorageId,
        array: crate::identity::ArrayTypeId,
        length: crate::mir::ValueId,
    ) -> Result<(), BackendError> {
        let layout = self
            .data_layout
            .array(array)
            .ok_or_else(|| self.array_error(format!("array {array} has no target layout")))?;
        let empty = self.next_array_label("allocate_empty");
        let failure = self.next_array_label("allocate_failure");
        let complete = self.next_array_label("allocate_complete");

        value::load_rax(value::frame_value(self.frame, length), self.output);
        self.output.push(Instruction::MoveImmediate64 {
            bits: layout.maximum_length(),
            destination: Register::R11,
        });
        self.output.push(Instruction::Compare {
            source: Register::R11,
            destination: Register::Rax,
        });
        self.output.push(Instruction::JumpIfAbove(failure.clone()));
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(empty.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::try_from(layout.stride()).expect("array stride fits u64"),
            destination: Register::R11,
        });
        self.output.push(Instruction::Multiply {
            source: Register::R11,
            destination: Register::Rax,
        });
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::try_from(layout.element_offset()).expect("array offset fits u64"),
            destination: Register::R11,
        });
        self.output.push(Instruction::Add {
            source: Register::R11,
            destination: Register::Rax,
        });
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdi.into(),
        });
        self.output
            .push(Instruction::Call(RUNTIME_ALLOC.to_owned()));
        value::store_rax(value::frame_storage(self.frame, backing), self.output);
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdx.into(),
        });
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::Rax,
        });
        value::store_rax(
            value::memory(Register::Rdx, ARRAY_OWNER_COUNT_OFFSET),
            self.output,
        );
        value::load_rax(value::frame_value(self.frame, length), self.output);
        value::store_rax(
            value::memory(Register::Rdx, ARRAY_LENGTH_OFFSET),
            self.output,
        );
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        });
        self.output.push(Instruction::Jump(complete.clone()));

        self.output.push(Instruction::Label(empty));
        self.clear_storage(backing);
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        });
        self.output.push(Instruction::Jump(complete.clone()));

        self.output.push(Instruction::Label(failure));
        self.clear_storage(backing);
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::R11,
        });
        self.output.push(Instruction::Label(complete));
        Ok(())
    }

    pub(super) fn select_array_element_place(
        &mut self,
        place: &MirPlace,
    ) -> Result<(FramePlace, Operand), BackendError> {
        let Some(MirPlaceProjection::ArrayElement {
            array,
            normalized_index,
        }) = place.projections.last()
        else {
            return Err(self.array_error("primitive array element place has no final projection"));
        };
        let declaration = self
            .program
            .array_type(*array)
            .ok_or_else(|| self.array_error(format!("array {array} is not declared")))?;
        let layout = self
            .data_layout
            .array(*array)
            .ok_or_else(|| self.array_error(format!("array {array} has no target layout")))?;
        let scale = u8::try_from(layout.stride())
            .map_err(|_| self.array_error(format!("array {array} stride cannot be encoded")))?;
        let displacement = i32::try_from(layout.element_offset())
            .map_err(|_| self.array_error(format!("array {array} offset cannot be encoded")))?;

        let mut owner = place.clone();
        owner.projections.pop();
        let (_, owner) = self.frame_place(&owner)?;
        value::load_rax(owner, self.output);
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::R11.into(),
        });
        self.output.push(Instruction::Move {
            source: value::frame_storage(self.frame, *normalized_index),
            destination: Register::Rcx.into(),
        });
        Ok((
            FramePlace::array_element(declaration.element),
            value::indexed_memory(Register::R11, Register::Rcx, scale, displacement),
        ))
    }

    fn select_array_element_normalize(
        &mut self,
        destination: crate::mir::StorageId,
        owner: &MirPlace,
        index: crate::mir::ValueId,
    ) -> Result<(), BackendError> {
        let (_, owner) = self.frame_place(owner)?;
        let empty = self.next_array_label("normalize_empty");
        let length_ready = self.next_array_label("normalize_length_ready");
        let valid = self.next_array_label("normalize_valid");
        let complete = self.next_array_label("normalize_complete");

        value::load_rax(owner, self.output);
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(empty.clone()));
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rax, ARRAY_LENGTH_OFFSET),
            destination: Register::Rdx.into(),
        });
        self.output.push(Instruction::Jump(length_ready.clone()));
        self.output.push(Instruction::Label(empty));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rdx,
        });

        self.output.push(Instruction::Label(length_ready));
        value::load_rax(value::frame_value(self.frame, index), self.output);
        self.output.push(Instruction::Test(Register::Rax));
        let compare = self.next_array_label("normalize_compare");
        self.output
            .push(Instruction::JumpIfNotSign(compare.clone()));
        self.output.push(Instruction::Add {
            source: Register::Rdx,
            destination: Register::Rax,
        });

        self.output.push(Instruction::Label(compare));
        self.output.push(Instruction::Compare {
            source: Register::Rdx,
            destination: Register::Rax,
        });
        self.output.push(Instruction::JumpIfBelow(valid.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::MAX,
            destination: Register::Rax,
        });
        self.output.push(Instruction::Jump(complete.clone()));
        self.output.push(Instruction::Label(valid));
        self.output.push(Instruction::Label(complete));
        value::store_rax(value::frame_storage(self.frame, destination), self.output);
        Ok(())
    }

    fn clear_storage(&mut self, storage: crate::mir::StorageId) {
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rax,
        });
        value::store_rax(value::frame_storage(self.frame, storage), self.output);
    }

    fn advance_array_index(&mut self, index: crate::mir::StorageId) {
        value::load_rax(value::frame_storage(self.frame, index), self.output);
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        });
        self.output.push(Instruction::Add {
            source: Register::R11,
            destination: Register::Rax,
        });
        value::store_rax(value::frame_storage(self.frame, index), self.output);
    }

    fn clone_array_preserving_destination(
        &mut self,
        destination: &MirPlace,
        source: &MirPlace,
        array: crate::identity::ArrayTypeId,
    ) -> Result<(), BackendError> {
        self.materialize_place_address(destination, Register::Rdx)?;
        self.output.push(Instruction::ReserveStack(16));
        self.output.push(Instruction::Move {
            source: Register::Rdx.into(),
            destination: value::memory(Register::Rsp, 0),
        });
        let (_, source) = self.frame_place(source)?;
        value::load_rax(source, self.output);
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdi.into(),
        });
        self.output
            .push(Instruction::Call(symbol::array_clone(array)));
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rsp, 0),
            destination: Register::Rdx.into(),
        });
        self.output.push(Instruction::ReleaseStack(16));
        Ok(())
    }

    fn clear_place(&mut self, place: &MirPlace) -> Result<(), BackendError> {
        let (_, destination) = self.frame_place(place)?;
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rax,
        });
        value::store_rax(destination, self.output);
        Ok(())
    }

    fn array_for_storage(
        &self,
        storage: crate::mir::StorageId,
    ) -> Result<crate::identity::ArrayTypeId, BackendError> {
        match self
            .function
            .storage(storage)
            .expect("verified array storage exists")
            .ty
        {
            MirType::Array(array) => Ok(array),
            _ => Err(self.array_error("array storage has no array type")),
        }
    }

    fn next_array_label(&mut self, purpose: &str) -> Label {
        let label = self.array_label(self.array_sequence, purpose);
        self.array_sequence += 1;
        label
    }

    fn array_label(&self, sequence: usize, purpose: &str) -> Label {
        Label::new(format!(
            ".Lska_{}_array_{}_{}_{}",
            symbol::local_label_stem(self.function.callable()),
            self.block.index(),
            sequence,
            purpose
        ))
    }

    fn array_error(&self, message: impl Into<String>) -> BackendError {
        BackendError::new(Target::X86_64SysV, Some(self.function.callable()), message)
    }
}
