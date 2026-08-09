//! Standard-I/O instruction selection for runtime ABI version 9.

use crate::{
    backend::BackendError,
    mir::{MirIoBuffer, MirIoInstruction, MirIoOperation, MirType, StorageId},
};

use super::{
    super::{
        layout::{ARRAY_LENGTH_OFFSET, SHARED_ARRAY_LENGTH_OFFSET},
        machine::{Instruction, Label, Register},
        symbol,
    },
    value, InstructionSelector,
};

const RUNTIME_IO_STANDARD_HANDLE: &str = "ska_rt_io_standard_handle";
const RUNTIME_IO_OPEN: &str = "ska_rt_io_open";
const RUNTIME_IO_READ: &str = "ska_rt_io_read";
const RUNTIME_IO_WRITE: &str = "ska_rt_io_write";
const RUNTIME_IO_CLOSE: &str = "ska_rt_io_close";

impl InstructionSelector<'_, '_> {
    pub(super) fn select_io_instruction(
        &mut self,
        instruction: &MirIoInstruction,
    ) -> Result<(), BackendError> {
        match &instruction.operation {
            MirIoOperation::StandardHandle { stream } => {
                self.load_io_argument(*stream, Register::Rdi);
                self.call_io_runtime(RUNTIME_IO_STANDARD_HANDLE, instruction.result);
            }
            MirIoOperation::Open { path, mode } => {
                self.select_io_buffer_range(path, None, Register::Rdi, Register::Rsi)?;
                self.load_io_argument(*mode, Register::Rdx);
                self.call_io_runtime(RUNTIME_IO_OPEN, instruction.result);
            }
            MirIoOperation::Read {
                handle,
                destination,
                offset,
            } => {
                self.select_io_buffer_range(
                    destination,
                    Some(*offset),
                    Register::Rsi,
                    Register::Rdx,
                )?;
                self.load_io_argument(*handle, Register::Rdi);
                self.call_io_runtime(RUNTIME_IO_READ, instruction.result);
            }
            MirIoOperation::Write {
                handle,
                source,
                offset,
            } => {
                self.select_io_buffer_range(source, Some(*offset), Register::Rsi, Register::Rdx)?;
                self.load_io_argument(*handle, Register::Rdi);
                self.call_io_runtime(RUNTIME_IO_WRITE, instruction.result);
            }
            MirIoOperation::Close { handle } => {
                self.load_io_argument(*handle, Register::Rdi);
                self.call_io_runtime(RUNTIME_IO_CLOSE, instruction.result);
            }
        }
        Ok(())
    }

    fn select_io_buffer_range(
        &mut self,
        buffer: &MirIoBuffer,
        offset: Option<StorageId>,
        pointer: Register,
        length: Register,
    ) -> Result<(), BackendError> {
        let shared = self.load_array_owner(&buffer.place)?;
        let layout = self
            .data_layout
            .array(buffer.array)
            .expect("verified I/O byte-array layout exists");
        let length_offset = if shared {
            SHARED_ARRAY_LENGTH_OFFSET
        } else {
            ARRAY_LENGTH_OFFSET
        };
        let element_offset = if shared {
            layout.shared_element_offset()
        } else {
            layout.element_offset()
        };
        let element_offset =
            i32::try_from(element_offset).expect("array header offset is encodable");
        let empty = self.next_io_label("range_empty");
        let complete = self.next_io_label("range_complete");

        // An empty descriptor has no backing header to inspect. The runtime
        // receives a null pointer and zero length, which is valid for every
        // version-9 byte operation and never requires a dereference.
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(empty.clone()));
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rax, length_offset),
            destination: length.into(),
        });
        self.output.push(Instruction::LoadEffectiveAddress {
            source: value::memory(Register::Rax, element_offset),
            destination: pointer,
        });
        if let Some(offset) = offset {
            self.output.push(Instruction::Move {
                source: value::frame_storage(self.frame, offset),
                destination: Register::R10.into(),
            });
            self.output.push(Instruction::Add {
                source: Register::R10,
                destination: pointer,
            });
            self.output.push(Instruction::Subtract {
                source: Register::R10,
                destination: length,
            });
        }
        self.output.push(Instruction::Jump(complete.clone()));
        self.output.push(Instruction::Label(empty));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: pointer,
        });
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: length,
        });
        self.output.push(Instruction::Label(complete));
        Ok(())
    }

    fn load_io_argument(&mut self, value_id: crate::mir::ValueId, destination: Register) {
        self.output.push(Instruction::Move {
            source: value::frame_value(self.frame, value_id),
            destination: destination.into(),
        });
    }

    fn call_io_runtime(&mut self, symbol: &str, result: crate::mir::ValueId) {
        self.output.push(Instruction::Call(symbol.to_owned()));
        value::store_canonical_rax(
            MirType::I64,
            value::frame_value(self.frame, result),
            self.output,
        );
    }

    fn next_io_label(&mut self, purpose: &str) -> Label {
        let sequence = self.io_sequence;
        self.io_sequence += 1;
        Label::new(format!(
            ".Lska.{}.io_{}_{}_{}",
            symbol::local_label_stem(self.program, self.function.callable()),
            self.block.index(),
            sequence,
            purpose
        ))
    }
}
