//! Centralized non-returning failure selection and ordinary MIR terminators.

use crate::mir::{MirTerminationReason, MirTerminator, MirType};

use super::{
    super::machine::{
        AssemblyFunction, AssemblyPanicMessage, Instruction, Label, Register, XmmRegister,
    },
    block_label, value, FrameLayout, InstructionSelector,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(usize)]
enum PanicMessage {
    ObjectCastFailure,
    OptionalAccessFailure,
    OptionalGuardOverflow,
    OptionalPinnedMutation,
    ArrayAllocationFailure,
    ArrayIndexOutOfBounds,
    ArrayInvalidSliceBounds,
    ArraySliceLengthMismatch,
    OwnershipCountOverflow,
    ShiftCountOutOfRange,
}

impl PanicMessage {
    const ALL: [Self; 10] = [
        Self::ObjectCastFailure,
        Self::OptionalAccessFailure,
        Self::OptionalGuardOverflow,
        Self::OptionalPinnedMutation,
        Self::ArrayAllocationFailure,
        Self::ArrayIndexOutOfBounds,
        Self::ArrayInvalidSliceBounds,
        Self::ArraySliceLengthMismatch,
        Self::OwnershipCountOverflow,
        Self::ShiftCountOutOfRange,
    ];

    const fn for_reason(reason: MirTerminationReason) -> Option<Self> {
        match reason {
            MirTerminationReason::ObjectCastFailure => Some(Self::ObjectCastFailure),
            MirTerminationReason::OptionalAccessFailure => Some(Self::OptionalAccessFailure),
            MirTerminationReason::OptionalGuardOverflow => Some(Self::OptionalGuardOverflow),
            MirTerminationReason::OptionalPinnedMutation => Some(Self::OptionalPinnedMutation),
            MirTerminationReason::ArrayAllocationFailure => Some(Self::ArrayAllocationFailure),
            MirTerminationReason::ArrayIndexOutOfBounds => Some(Self::ArrayIndexOutOfBounds),
            MirTerminationReason::ArrayInvalidSliceBounds => Some(Self::ArrayInvalidSliceBounds),
            MirTerminationReason::ArraySliceLengthMismatch => Some(Self::ArraySliceLengthMismatch),
            MirTerminationReason::ShiftCountOutOfRange => Some(Self::ShiftCountOutOfRange),
            MirTerminationReason::IntegerDivisionByZero
            | MirTerminationReason::IntegerRemainderByZero => None,
        }
    }

    const fn index(self) -> usize {
        self as usize
    }

    const fn bytes(self) -> &'static [u8] {
        match self {
            Self::ObjectCastFailure => b"checked object cast failed",
            Self::OptionalAccessFailure => b"optional value is absent",
            Self::OptionalGuardOverflow => b"optional presence guard overflow",
            Self::OptionalPinnedMutation => b"cannot mutate a guarded optional value",
            Self::ArrayAllocationFailure => b"array allocation failed",
            Self::ArrayIndexOutOfBounds => b"array index out of bounds",
            Self::ArrayInvalidSliceBounds => b"array slice bounds are invalid",
            Self::ArraySliceLengthMismatch => b"array slice length mismatch",
            Self::OwnershipCountOverflow => b"ownership count overflow",
            Self::ShiftCountOutOfRange => b"shift count out of range",
        }
    }

    const fn symbol(self) -> &'static str {
        match self {
            Self::ObjectCastFailure => ".Lska_panic_message_0",
            Self::OptionalAccessFailure => ".Lska_panic_message_1",
            Self::OptionalGuardOverflow => ".Lska_panic_message_2",
            Self::OptionalPinnedMutation => ".Lska_panic_message_3",
            Self::ArrayAllocationFailure => ".Lska_panic_message_4",
            Self::ArrayIndexOutOfBounds => ".Lska_panic_message_5",
            Self::ArrayInvalidSliceBounds => ".Lska_panic_message_6",
            Self::ArraySliceLengthMismatch => ".Lska_panic_message_7",
            Self::OwnershipCountOverflow => ".Lska_panic_message_8",
            Self::ShiftCountOutOfRange => ".Lska_panic_message_9",
        }
    }
}

pub(super) struct PanicMessagePool {
    used: [bool; PanicMessage::ALL.len()],
}

impl PanicMessagePool {
    pub(super) fn build(functions: &[AssemblyFunction]) -> Self {
        let mut used = [false; PanicMessage::ALL.len()];
        for function in functions {
            for instruction in &function.instructions {
                let Instruction::LoadSymbolAddress { symbol, .. } = instruction else {
                    continue;
                };
                if let Some(message) = PanicMessage::ALL
                    .into_iter()
                    .find(|message| message.symbol() == symbol)
                {
                    used[message.index()] = true;
                }
            }
        }
        Self { used }
    }

    pub(super) fn into_assembly(self) -> Vec<AssemblyPanicMessage> {
        PanicMessage::ALL
            .into_iter()
            .filter(|message| self.used[message.index()])
            .map(|message| AssemblyPanicMessage {
                symbol: message.symbol().to_owned(),
                bytes: message.bytes(),
            })
            .collect()
    }
}

pub(super) fn emit_ownership_overflow(output: &mut Vec<Instruction>) {
    emit_static_panic(PanicMessage::OwnershipCountOverflow, output);
}

impl InstructionSelector<'_, '_> {
    pub(super) fn select_termination(
        &mut self,
        terminator: &MirTerminator,
    ) -> Result<bool, crate::backend::BackendError> {
        match terminator {
            MirTerminator::Panic { message, .. } => {
                self.select_dynamic_panic(message)?;
                Ok(true)
            }
            MirTerminator::Terminate { reason, .. } => {
                let Some(message) = PanicMessage::for_reason(*reason) else {
                    return Err(crate::backend::BackendError::new(
                        crate::backend::Target::X86_64SysV,
                        Some(self.function.callable()),
                        format!(
                            "termination reason `{}` is not executable yet",
                            reason.mnemonic()
                        ),
                    ));
                };
                self.select_static_panic(message);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn select_dynamic_panic(
        &mut self,
        message: &crate::mir::MirPlace,
    ) -> Result<(), crate::backend::BackendError> {
        let item = self
            .program
            .string_language_item
            .expect("verified panic requires the string language item");
        self.materialize_place_address(message, Register::Rdx)?;
        let field_offset = |field| {
            let offset = self
                .data_layout
                .field(field)
                .expect("verified string field has target layout")
                .offset;
            i32::try_from(offset).expect("target layout bounds every field displacement")
        };

        self.output.push(Instruction::Move {
            source: value::memory(Register::Rdx, field_offset(item.storage_field)),
            destination: Register::Rdi.into(),
        });
        let data_offset = self
            .data_layout
            .array(item.storage_array)
            .expect("verified string backing has target layout")
            .shared_element_offset();
        self.output.push(Instruction::LoadEffectiveAddress {
            source: value::memory(
                Register::Rdi,
                i32::try_from(data_offset).expect("array data offset fits x86-64"),
            ),
            destination: Register::Rdi,
        });
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rdx, field_offset(item.start_field)),
            destination: Register::Rax.into(),
        });
        self.output.push(Instruction::Add {
            source: Register::Rax,
            destination: Register::Rdi,
        });
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rdx, field_offset(item.length_field)),
            destination: Register::Rsi.into(),
        });
        emit_reporter_call(self.output);
        Ok(())
    }

    fn select_static_panic(&mut self, message: PanicMessage) {
        emit_static_panic(message, self.output);
    }
}

fn emit_static_panic(message: PanicMessage, output: &mut Vec<Instruction>) {
    output.push(Instruction::LoadSymbolAddress {
        symbol: message.symbol().to_owned(),
        destination: Register::Rdi,
    });
    output.push(Instruction::MoveImmediate64 {
        bits: message.bytes().len() as u64,
        destination: Register::Rsi,
    });
    emit_reporter_call(output);
}

fn emit_reporter_call(output: &mut Vec<Instruction>) {
    output.push(Instruction::Call("ska_rt_panic".to_owned()));
}

pub(super) fn select(
    program: &crate::mir::MirProgram,
    terminator: &MirTerminator,
    frame: &FrameLayout,
    return_type: MirType,
    epilogue: &Label,
    output: &mut Vec<Instruction>,
) {
    match terminator {
        MirTerminator::Return { value: result, .. } => {
            if let Some(result) = result {
                let source = value::frame_value(frame, *result);
                if return_type == MirType::F64 {
                    value::load_float(value::float_operand(source), XmmRegister::Xmm0, output);
                } else {
                    value::load_rax(source, output);
                    value::canonicalize_rax(return_type, output);
                }
            }
            output.push(Instruction::Jump(epilogue.clone()));
        }
        MirTerminator::ReturnShared { owner, .. }
        | MirTerminator::ReturnOptionalShared { owner, .. } => {
            value::load_rax(value::frame_storage(frame, *owner), output);
            output.push(Instruction::Jump(epilogue.clone()));
        }
        MirTerminator::Goto { target, .. } => {
            output.push(Instruction::Jump(block_label(program, *target)));
        }
        MirTerminator::Branch {
            condition,
            true_target,
            false_target,
            ..
        } => {
            value::load_rax(value::frame_value(frame, *condition), output);
            output.push(Instruction::Test(Register::Rax));
            output.push(Instruction::JumpIfNotZero(block_label(
                program,
                *true_target,
            )));
            output.push(Instruction::Jump(block_label(program, *false_target)));
        }
        MirTerminator::CheckedCast { .. }
        | MirTerminator::ShiftCountCheck { .. }
        | MirTerminator::Panic { .. }
        | MirTerminator::SharedCast { .. }
        | MirTerminator::OptionalUnwrap { .. }
        | MirTerminator::OptionalSharedUnwrap { .. }
        | MirTerminator::BeginOptionalView { .. }
        | MirTerminator::CheckOptionalMutation { .. }
        | MirTerminator::ArrayPositionCheck { .. }
        | MirTerminator::ArrayOperationCheck { .. }
        | MirTerminator::ArrayLoop { .. }
        | MirTerminator::Terminate { .. } => {
            unreachable!("type-operation terminators use their dedicated selector")
        }
    }
}
