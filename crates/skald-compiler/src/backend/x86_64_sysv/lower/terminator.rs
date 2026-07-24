//! MIR terminator instruction selection.

use crate::mir::{MirTerminator, MirType};

use super::{
    super::machine::{Instruction, Label, Register, XmmRegister},
    block_label, value, FrameLayout,
};

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
        MirTerminator::CheckedNarrow { .. }
        | MirTerminator::CheckedCast { .. }
        | MirTerminator::Terminate { .. } => {
            unreachable!("type-operation terminators use their dedicated selector")
        }
    }
}
