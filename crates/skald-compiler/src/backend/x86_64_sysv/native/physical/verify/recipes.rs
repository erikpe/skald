//! Finite recipe acceptance. This cursor never invokes expansion or formatting.
use super::super::{
    super::{
        selected::{
            Cell, FloatCondition, Instruction as SelectedInstruction, IntegerCondition, Numeric,
            Opcode,
        },
        Gpr,
    },
    model::*,
};
use super::{
    authority::{Authority, Cursor},
    Reason,
};
use crate::backend::{
    frame::{AddressRecipe, Base},
    lir::{
        BinaryOperation, CallTarget as LogicalCallTarget, Constant, ShiftDirection, UnaryOperation,
    },
    placement::Site,
    plan::{ArtifactId, RuntimeService},
    selected::{Bundle, Payload, RepresentationKind},
};

pub(super) fn check(
    a: &Authority<'_, '_, '_, '_>,
    site: Site,
    p: &SelectedInstruction,
    successors: &[BlockId],
    code: &[Instruction],
) -> Result<(), Reason> {
    let description = p.describe();
    if let Some(Bundle::Bounded { steps, .. }) = &description.bundle {
        if code.len() > usize::from(steps.get()) {
            return Err(Reason::Recipe);
        }
    }
    let operand = |slot| a.operand(site, slot);
    let reg = |slot| a.reg(site, slot);
    let scratch = |slot| a.scratch(site, slot);
    let width = |slot: usize| description.operands[slot].representation.bits();
    let mut c = Cursor::new(code);
    match p.opcode() {
        Opcode::Constant { constant, .. } => {
            let value = match *constant {
                Constant::I64(n) => n as u64,
                Constant::U64(n) | Constant::F64(n) => n,
                Constant::U8(n) => u64::from(n),
                Constant::Bool(n) => u64::from(n),
                Constant::Null(_) => 0,
            };
            if matches!(constant, Constant::F64(_)) {
                c.movement(
                    64,
                    Operand::Immediate(value),
                    Operand::Register(scratch(0)?),
                )?;
                c.movement(64, Operand::Register(scratch(0)?), operand(0)?)?;
            } else {
                c.movement(width(0), Operand::Immediate(value), operand(0)?)?;
            }
        }
        Opcode::Alu {
            operation, left, ..
        } => {
            let op = match operation {
                BinaryOperation::Add => Alu::Add,
                BinaryOperation::Subtract => Alu::Subtract,
                BinaryOperation::Multiply => Alu::Multiply,
                BinaryOperation::FloatDivide => Alu::DivideFloat,
                BinaryOperation::And => Alu::And,
                BinaryOperation::Or => Alu::Or,
                BinaryOperation::Xor => Alu::Xor,
            };
            if *operation == BinaryOperation::Multiply && width(0) == 8 {
                for slot in [0, 1] {
                    c.expect(Instruction::Convert {
                        op: Convert::ZeroExtend,
                        input_bits: 8,
                        output_bits: 32,
                        source: reg(slot)?,
                        destination: scratch(slot)?,
                    })?;
                }
                c.expect(Instruction::Alu {
                    op: Alu::Multiply,
                    float: false,
                    bits: 32,
                    source: Operand::Register(scratch(1)?),
                    destination: scratch(0)?,
                })?;
                c.movement(8, Operand::Register(scratch(0)?), operand(2)?)?;
            } else {
                c.expect(Instruction::Alu {
                    op,
                    float: left.representation.kind == RepresentationKind::Float,
                    bits: width(2),
                    source: operand(1)?,
                    destination: reg(2)?,
                })?;
            }
        }
        Opcode::Unary {
            operation, input, ..
        } => match operation {
            UnaryOperation::LogicalNot => {
                c.expect(Instruction::Compare {
                    float: false,
                    bits: width(0),
                    left: operand(0)?,
                    right: Operand::Immediate(0),
                })?;
                c.expect(Instruction::Set {
                    condition: Condition::Equal,
                    destination: reg(1)?,
                })?;
            }
            UnaryOperation::Negate if input.representation.kind == RepresentationKind::Float => {
                c.movement(64, operand(0)?, Operand::Register(scratch(0)?))?;
                c.movement(
                    64,
                    Operand::Immediate(0x8000_0000_0000_0000),
                    Operand::Register(scratch(1)?),
                )?;
                c.expect(Instruction::Alu {
                    op: Alu::Xor,
                    float: false,
                    bits: 64,
                    source: Operand::Register(scratch(1)?),
                    destination: scratch(0)?,
                })?;
                c.movement(64, Operand::Register(scratch(0)?), operand(1)?)?;
            }
            UnaryOperation::Negate | UnaryOperation::Complement => {
                c.expect(Instruction::Unary {
                    op: if *operation == UnaryOperation::Negate {
                        Unary::Negate
                    } else {
                        Unary::Complement
                    },
                    bits: width(1),
                    destination: reg(1)?,
                })?
            }
        },
        Opcode::IntegerCompare { condition, .. } => {
            let condition = match condition {
                IntegerCondition::Equal => Condition::Equal,
                IntegerCondition::NotEqual => Condition::NotEqual,
                IntegerCondition::Less => Condition::Less,
                IntegerCondition::LessEqual => Condition::LessEqual,
                IntegerCondition::Greater => Condition::Greater,
                IntegerCondition::GreaterEqual => Condition::GreaterEqual,
                IntegerCondition::Below => Condition::Below,
                IntegerCondition::BelowEqual => Condition::BelowEqual,
                IntegerCondition::Above => Condition::Above,
                IntegerCondition::AboveEqual => Condition::AboveEqual,
            };
            c.expect(Instruction::Compare {
                float: false,
                bits: width(0),
                left: operand(0)?,
                right: operand(1)?,
            })?;
            c.expect(Instruction::Set {
                condition,
                destination: reg(2)?,
            })?;
        }
        Opcode::FloatCompare { condition, .. } => {
            c.expect(Instruction::Compare {
                float: true,
                bits: 64,
                left: operand(0)?,
                right: operand(1)?,
            })?;
            let (condition_code, parity) = match condition {
                FloatCondition::EqualOrdered => {
                    (Condition::Equal, Some((Condition::NotParity, Alu::And)))
                }
                FloatCondition::NotEqualOrUnordered => {
                    (Condition::NotEqual, Some((Condition::Parity, Alu::Or)))
                }
                FloatCondition::BelowOrdered => {
                    (Condition::Below, Some((Condition::NotParity, Alu::And)))
                }
                FloatCondition::BelowEqualOrdered => (
                    Condition::BelowEqual,
                    Some((Condition::NotParity, Alu::And)),
                ),
                FloatCondition::Above => (Condition::Above, None),
                FloatCondition::AboveEqual => (Condition::AboveEqual, None),
            };
            c.expect(Instruction::Set {
                condition: condition_code,
                destination: reg(2)?,
            })?;
            if let Some((condition, op)) = parity {
                c.expect(Instruction::Set {
                    condition,
                    destination: scratch(0)?,
                })?;
                c.expect(Instruction::Alu {
                    op,
                    float: false,
                    bits: 8,
                    source: Operand::Register(scratch(0)?),
                    destination: reg(2)?,
                })?;
            }
        }
        Opcode::Numeric(n) => match n {
            Numeric::Boundary(_) => {}
            Numeric::Cell { cell, .. } => match cell {
                Cell::Copy | Cell::Narrow => c.movement(width(1), operand(0)?, operand(1)?)?,
                Cell::FloatBits => c.movement(64, operand(0)?, operand(1)?)?,
                Cell::ZeroExtend | Cell::SignedToFloat => c.expect(Instruction::Convert {
                    op: if *cell == Cell::ZeroExtend {
                        Convert::ZeroExtend
                    } else {
                        Convert::SignedToFloat
                    },
                    input_bits: width(0),
                    output_bits: width(1),
                    source: reg(0)?,
                    destination: reg(1)?,
                })?,
            },
            Numeric::CheckedTruncate { .. } => c.expect(Instruction::Convert {
                op: Convert::TruncateFloat,
                input_bits: 64,
                output_bits: 64,
                source: reg(0)?,
                destination: reg(1)?,
            })?,
            Numeric::Dividend { signed, .. } => {
                if *signed {
                    c.expect(Instruction::DividendSignExtend)?;
                } else {
                    c.expect(Instruction::Alu {
                        op: Alu::Xor,
                        float: false,
                        bits: 32,
                        source: Operand::Register(Register::Gpr(Gpr::Rdx)),
                        destination: Register::Gpr(Gpr::Rdx),
                    })?;
                }
            }
            Numeric::Divide { signed, .. } => c.expect(Instruction::Divide {
                signed: *signed,
                divisor: reg(2)?,
            })?,
            Numeric::Shift { direction, .. } => c.expect(Instruction::Shift {
                op: match direction {
                    ShiftDirection::Left => Shift::Left,
                    ShiftDirection::ArithmeticRight => Shift::ArithmeticRight,
                    ShiftDirection::LogicalRight => Shift::LogicalRight,
                },
                bits: width(2),
                destination: reg(2)?,
                variable: true,
            })?,
            Numeric::ShiftOne { .. } => c.expect(Instruction::Shift {
                op: Shift::LogicalRight,
                bits: 64,
                destination: reg(1)?,
                variable: false,
            })?,
        },
        Opcode::ObjectAddress { object, .. } => {
            if a.frame.object_access(site, *object) != Some(AddressRecipe::Direct) {
                return Err(Reason::Recipe);
            }
            let region = a.frame.object(*object).ok_or(Reason::Recipe)?;
            c.expect(Instruction::Lea {
                source: Operand::Memory {
                    base: match region.base {
                        Base::Frame => Gpr::Rbp,
                        Base::Stack => Gpr::Rsp,
                    },
                    displacement: i32::try_from(region.offset).map_err(|_| Reason::Recipe)?,
                },
                destination: reg(0)?,
            })?;
        }
        Opcode::SymbolAddress { symbol, .. } => c.expect(Instruction::RipAddress {
            artifact: *symbol,
            destination: reg(0)?,
        })?,
        Opcode::ByteOffset { .. } => c.expect(Instruction::Alu {
            op: Alu::Add,
            float: false,
            bits: 64,
            source: operand(1)?,
            destination: reg(2)?,
        })?,
        Opcode::Load { .. }
        | Opcode::TraceLoad { .. }
        | Opcode::Store { .. }
        | Opcode::TraceStore { .. } => {
            let Register::Gpr(base) = reg(0)? else {
                return Err(Reason::Recipe);
            };
            let address = Operand::Memory {
                base,
                displacement: 0,
            };
            let bytes = match p.opcode() {
                Opcode::Load { bytes, .. } | Opcode::Store { bytes, .. } => *bytes,
                _ => 8,
            };
            if matches!(p.opcode(), Opcode::Load { .. } | Opcode::TraceLoad { .. }) {
                c.movement((bytes * 8) as u16, address, operand(1)?)?;
            } else {
                c.movement((bytes * 8) as u16, operand(1)?, address)?;
            }
        }
        Opcode::TlsAddress { .. } => {
            c.expect(Instruction::TlsBase {
                destination: reg(0)?,
            })?;
            c.expect(Instruction::TlsOffset {
                artifact: ArtifactId::TraceTls,
                destination: reg(0)?,
            })?;
        }
        Opcode::Call(call) => {
            let target = match call.target {
                LogicalCallTarget::Direct(id) => CallTarget::Direct(id),
                LogicalCallTarget::Indirect(_) => CallTarget::Indirect(reg(call.arguments.len())?),
            };
            c.expect(Instruction::Call(target))?;
            if call.never {
                c.expect(Instruction::HardTrap)?;
            }
        }
        Opcode::Failure { .. } => {
            c.expect(Instruction::Call(CallTarget::Direct(ArtifactId::Runtime(
                RuntimeService::Panic,
            ))))?;
            c.expect(Instruction::HardTrap)?;
        }
        Opcode::HardTrap => c.expect(Instruction::HardTrap)?,
        Opcode::Lifetime { .. } | Opcode::Return { .. } => {}
        Opcode::Jump => c.expect(Instruction::Jump(
            *successors.first().ok_or(Reason::Topology)?,
        ))?,
        Opcode::Branch { .. } | Opcode::CheckBranch { .. } => {
            if successors.len() != 2 {
                return Err(Reason::Topology);
            }
            c.expect(Instruction::Compare {
                float: false,
                bits: width(0),
                left: operand(0)?,
                right: Operand::Immediate(0),
            })?;
            c.expect(Instruction::JumpIf {
                condition: Condition::NotEqual,
                destination: successors[0],
            })?;
            c.expect(Instruction::Jump(successors[1]))?;
        }
    }
    c.finish()
}
