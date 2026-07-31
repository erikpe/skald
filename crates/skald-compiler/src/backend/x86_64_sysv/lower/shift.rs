//! x86-64 selection for verified checked integer shifts.

use crate::mir::{MirShiftDirection, MirShiftOperation, MirTerminator, MirType, ValueId};

use super::{
    super::machine::{Instruction, Operand, Register, ShiftOperation},
    block_label, value, InstructionSelector,
};

impl InstructionSelector<'_, '_> {
    /// Selects the target-independent range check without touching `rcx`.
    /// The count reaches `cl` only in the verified success block.
    pub(super) fn select_shift_terminator(&mut self, terminator: &MirTerminator) -> bool {
        let MirTerminator::ShiftCountCheck {
            check,
            success_target,
            failure_target,
            ..
        } = terminator
        else {
            return false;
        };
        value::load_rax(value::frame_storage(self.frame, check.count), self.output);
        self.output.push(Instruction::MoveImmediate64 {
            bits: check.operation.width(),
            destination: Register::R11,
        });
        self.output.push(Instruction::Compare {
            source: Register::R11,
            destination: Register::Rax,
        });
        self.output.push(Instruction::JumpIfBelow(block_label(
            self.program,
            *success_target,
        )));
        self.output.push(Instruction::Jump(block_label(
            self.program,
            *failure_target,
        )));
        true
    }

    pub(super) fn select_shift(
        &mut self,
        operation: MirShiftOperation,
        left: ValueId,
        count: ValueId,
        ty: MirType,
        destination: Operand,
    ) {
        value::load_rax(value::frame_value(self.frame, left), self.output);
        self.output.push(Instruction::Move {
            source: value::frame_value(self.frame, count),
            destination: Register::Rcx.into(),
        });
        self.output.push(Instruction::Shift {
            operation: match (operation.direction, operation.right_shift_flavor()) {
                (MirShiftDirection::Left, None) => ShiftOperation::Left,
                (MirShiftDirection::Right, Some(crate::mir::MirRightShiftFlavor::Arithmetic)) => {
                    ShiftOperation::ArithmeticRight
                }
                (MirShiftDirection::Right, Some(crate::mir::MirRightShiftFlavor::Logical)) => {
                    ShiftOperation::LogicalRight
                }
                _ => unreachable!("verified shift operation has one exact target flavor"),
            },
            destination: Register::Rax,
        });
        value::store_canonical_rax(ty, destination, self.output);
    }
}
