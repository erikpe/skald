//! Centralized non-returning failure selection and ordinary MIR terminators.

use crate::mir::{MirProgram, MirTerminationReason, MirTerminator, MirType};

use super::{
    super::machine::{AssemblyPanicMessage, Instruction, Label, Register, XmmRegister},
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
}

impl PanicMessage {
    const ALL: [Self; 8] = [
        Self::ObjectCastFailure,
        Self::OptionalAccessFailure,
        Self::OptionalGuardOverflow,
        Self::OptionalPinnedMutation,
        Self::ArrayAllocationFailure,
        Self::ArrayIndexOutOfBounds,
        Self::ArrayInvalidSliceBounds,
        Self::ArraySliceLengthMismatch,
    ];

    const fn for_reason(reason: MirTerminationReason) -> Self {
        match reason {
            MirTerminationReason::ObjectCastFailure => Self::ObjectCastFailure,
            MirTerminationReason::OptionalAccessFailure => Self::OptionalAccessFailure,
            MirTerminationReason::OptionalGuardOverflow => Self::OptionalGuardOverflow,
            MirTerminationReason::OptionalPinnedMutation => Self::OptionalPinnedMutation,
            MirTerminationReason::ArrayAllocationFailure => Self::ArrayAllocationFailure,
            MirTerminationReason::ArrayIndexOutOfBounds => Self::ArrayIndexOutOfBounds,
            MirTerminationReason::ArrayInvalidSliceBounds => Self::ArrayInvalidSliceBounds,
            MirTerminationReason::ArraySliceLengthMismatch => Self::ArraySliceLengthMismatch,
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
        }
    }

    fn symbol(self) -> String {
        format!(".Lska_panic_message_{}", self.index())
    }
}

pub(super) struct PanicMessagePool {
    used: [bool; PanicMessage::ALL.len()],
}

impl PanicMessagePool {
    pub(super) fn build(program: &MirProgram) -> Self {
        let mut used = [false; PanicMessage::ALL.len()];
        for definition in program.executable_definitions() {
            for block in &definition.body().blocks {
                if let Some(MirTerminator::Terminate { reason, .. }) = block.terminator {
                    used[PanicMessage::for_reason(reason).index()] = true;
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
                symbol: message.symbol(),
                bytes: message.bytes(),
            })
            .collect()
    }
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
                self.select_static_panic(PanicMessage::for_reason(*reason));
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
        self.call_reporter();
        Ok(())
    }

    fn select_static_panic(&mut self, message: PanicMessage) {
        self.output.push(Instruction::LoadSymbolAddress {
            symbol: message.symbol(),
            destination: Register::Rdi,
        });
        self.output.push(Instruction::MoveImmediate64 {
            bits: message.bytes().len() as u64,
            destination: Register::Rsi,
        });
        self.call_reporter();
    }

    fn call_reporter(&mut self) {
        self.output
            .push(Instruction::Call("ska_rt_panic".to_owned()));
    }
}

pub(super) fn select(
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
            output.push(Instruction::Jump(block_label(*target)));
        }
        MirTerminator::Branch {
            condition,
            true_target,
            false_target,
            ..
        } => {
            value::load_rax(value::frame_value(frame, *condition), output);
            output.push(Instruction::Test(Register::Rax));
            output.push(Instruction::JumpIfNotZero(block_label(*true_target)));
            output.push(Instruction::Jump(block_label(*false_target)));
        }
        MirTerminator::CheckedCast { .. }
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
