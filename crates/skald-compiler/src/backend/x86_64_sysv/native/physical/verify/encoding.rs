//! Target encoding rules, independent of the formatter and expansion templates.
use super::super::model::*;
use super::Reason;

fn gpr(r: Register) -> bool {
    matches!(r, Register::Gpr(_))
}
fn xmm(r: Register) -> bool {
    matches!(r, Register::Xmm(i) if i < 16)
}
fn integer(o: Operand, bits: u16, immediate: bool) -> bool {
    match o {
        Operand::Register(r) => gpr(r),
        Operand::Memory { .. } => true,
        Operand::Immediate(v) => {
            immediate
                && match bits {
                    8 => v <= u8::MAX as u64,
                    16 => v <= u16::MAX as u64,
                    32 => v <= u32::MAX as u64,
                    64 => v <= i32::MAX as u64 || v >= i32::MIN as i64 as u64,
                    _ => false,
                }
        }
    }
}
fn float(o: Operand) -> bool {
    matches!(o, Operand::Register(r) if xmm(r)) || matches!(o, Operand::Memory { .. })
}
fn memory(o: Operand) -> bool {
    matches!(o, Operand::Memory { .. })
}
fn width(bits: u16) -> bool {
    matches!(bits, 8 | 16 | 32 | 64)
}
pub(super) fn check(i: &Instruction, blocks: usize) -> Result<(), Reason> {
    let legal = match *i {
        Instruction::Move {
            kind,
            bits,
            source,
            destination,
        } => match kind {
            MoveKind::Integer => {
                width(bits)
                    && integer(destination, bits, false)
                    && (matches!(source, Operand::Immediate(_))
                        && matches!(destination, Operand::Register(r) if gpr(r))
                        && bits == 64
                        || integer(source, bits, true))
                    && !(memory(source) && memory(destination))
            }
            MoveKind::Float => {
                bits == 64
                    && float(source)
                    && float(destination)
                    && !(memory(source) && memory(destination))
            }
            MoveKind::Bits => {
                bits == 64
                    && match (source, destination) {
                        (Operand::Register(a), Operand::Register(b)) => {
                            (gpr(a) && xmm(b)) || (xmm(a) && gpr(b))
                        }
                        _ => false,
                    }
            }
        },
        Instruction::Alu {
            op,
            float: f,
            bits,
            source,
            destination,
        } => {
            if f {
                bits == 64
                    && xmm(destination)
                    && float(source)
                    && matches!(
                        op,
                        Alu::Add | Alu::Subtract | Alu::Multiply | Alu::DivideFloat
                    )
            } else {
                width(bits)
                    && gpr(destination)
                    && integer(source, bits, true)
                    && op != Alu::DivideFloat
                    && !(op == Alu::Multiply
                        && (bits == 8 || matches!(source, Operand::Immediate(_))))
            }
        }
        Instruction::Unary {
            bits, destination, ..
        } => width(bits) && gpr(destination),
        Instruction::Compare {
            float: f,
            bits,
            left,
            right,
        } => {
            if f {
                bits == 64 && matches!(left,Operand::Register(r) if xmm(r)) && float(right)
            } else {
                width(bits)
                    && integer(left, bits, false)
                    && integer(right, bits, true)
                    && !(memory(left) && memory(right))
            }
        }
        Instruction::Set { destination, .. } => gpr(destination),
        Instruction::Convert {
            op,
            input_bits,
            output_bits,
            source,
            destination,
        } => match op {
            Convert::ZeroExtend => {
                gpr(source)
                    && gpr(destination)
                    && matches!(input_bits, 8 | 16)
                    && matches!(output_bits, 16 | 32 | 64)
                    && input_bits < output_bits
            }
            Convert::SignedToFloat => {
                gpr(source)
                    && xmm(destination)
                    && matches!(input_bits, 32 | 64)
                    && output_bits == 64
            }
            Convert::TruncateFloat => {
                xmm(source)
                    && gpr(destination)
                    && input_bits == 64
                    && matches!(output_bits, 32 | 64)
            }
        },
        Instruction::DividendSignExtend => true,
        Instruction::Divide { divisor, .. } => gpr(divisor),
        Instruction::Shift {
            bits, destination, ..
        } => width(bits) && gpr(destination),
        Instruction::Lea {
            source,
            destination,
        } => memory(source) && gpr(destination),
        Instruction::RipAddress { destination, .. }
        | Instruction::TlsBase { destination }
        | Instruction::TlsOffset { destination, .. } => gpr(destination),
        Instruction::Call(CallTarget::Direct(_)) => true,
        Instruction::Call(CallTarget::Indirect(r)) => gpr(r),
        Instruction::Jump(id)
        | Instruction::JumpIf {
            destination: id, ..
        } => id.0 < blocks,
        Instruction::Push(_) | Instruction::Pop(_) => true,
        Instruction::StackSubtract(bytes) => bytes > 0 && bytes <= i32::MAX as u32,
        Instruction::Return | Instruction::HardTrap => true,
    };
    if legal {
        Ok(())
    } else {
        Err(Reason::Encoding)
    }
}
