//! Finite target recipes. Semantic correction graphs have already been selected.
use super::super::selected::{
    Cell, FloatCondition, Instruction as SelectedInstruction, IntegerCondition, Numeric, Opcode,
};
use super::{
    model::*,
    operands::{mov, Inputs},
};
use crate::backend::{
    frame::{AddressRecipe, Base},
    lir::{BinaryOperation, CallTarget as SelectedCallTarget, Constant, UnaryOperation},
    placement::Site,
    selected::{Bundle, Payload},
};
impl Inputs<'_, '_, '_, '_> {
    pub(super) fn recipe(
        &self,
        site: Site,
        selected: &SelectedInstruction,
        successors: &[BlockId],
    ) -> Result<Group, RealizeError> {
        let d = selected.describe();
        let o = |slot| self.operand(site, slot);
        let r = |slot| self.reg(site, slot);
        let s = |slot| self.scratch(site, slot);
        let bits = |slot: usize| d.operands[slot].representation.bits();
        let mut code = vec![];
        match selected.opcode() {
            Opcode::Constant { constant, .. } => match *constant {
                Constant::F64(value) => {
                    code.push(mov(64, Operand::Immediate(value), Operand::Register(s(0)?)));
                    code.push(mov(64, Operand::Register(s(0)?), o(0)?));
                }
                _ => {
                    let value = match *constant {
                        Constant::I64(v) => v as u64,
                        Constant::U64(v) => v,
                        Constant::U8(v) => v as u64,
                        Constant::Bool(v) => u64::from(v),
                        Constant::Null(_) => 0,
                        Constant::F64(_) => unreachable!(),
                    };
                    code.push(mov(bits(0), Operand::Immediate(value), o(0)?));
                }
            },
            Opcode::Alu {
                operation, left, ..
            } => {
                let op = alu(*operation);
                if *operation == BinaryOperation::Multiply && left.representation.bits() == 8 {
                    for slot in 0..2 {
                        code.push(Instruction::Convert {
                            op: Convert::ZeroExtend,
                            input_bits: 8,
                            output_bits: 32,
                            source: r(slot)?,
                            destination: s(slot)?,
                        });
                    }
                    code.push(Instruction::Alu {
                        op,
                        float: false,
                        bits: 32,
                        source: Operand::Register(s(1)?),
                        destination: s(0)?,
                    });
                    code.push(mov(8, Operand::Register(s(0)?), o(2)?));
                } else {
                    code.push(Instruction::Alu {
                        op,
                        float: left.representation.kind
                            == crate::backend::selected::RepresentationKind::Float,
                        bits: bits(2),
                        source: o(1)?,
                        destination: r(2)?,
                    });
                }
            }
            Opcode::Unary {
                operation, input, ..
            } => {
                if *operation == UnaryOperation::LogicalNot {
                    code.push(Instruction::Compare {
                        float: false,
                        bits: bits(0),
                        left: o(0)?,
                        right: Operand::Immediate(0),
                    });
                    code.push(Instruction::Set {
                        condition: Condition::Equal,
                        destination: r(1)?,
                    });
                } else if input.representation.kind
                    == crate::backend::selected::RepresentationKind::Float
                {
                    code.push(mov(64, o(0)?, Operand::Register(s(0)?)));
                    code.push(mov(
                        64,
                        Operand::Immediate(1 << 63),
                        Operand::Register(s(1)?),
                    ));
                    code.push(Instruction::Alu {
                        op: Alu::Xor,
                        float: false,
                        bits: 64,
                        source: Operand::Register(s(1)?),
                        destination: s(0)?,
                    });
                    code.push(mov(64, Operand::Register(s(0)?), o(1)?));
                } else {
                    code.push(Instruction::Unary {
                        op: if *operation == UnaryOperation::Negate {
                            Unary::Negate
                        } else {
                            Unary::Complement
                        },
                        bits: bits(1),
                        destination: r(1)?,
                    });
                }
            }
            Opcode::IntegerCompare { condition, .. } => {
                code.push(Instruction::Compare {
                    float: false,
                    bits: bits(0),
                    left: o(0)?,
                    right: o(1)?,
                });
                code.push(Instruction::Set {
                    condition: integer_condition(*condition),
                    destination: r(2)?,
                });
            }
            Opcode::FloatCompare { condition, .. } => {
                code.push(Instruction::Compare {
                    float: true,
                    bits: 64,
                    left: o(0)?,
                    right: o(1)?,
                });
                let condition_code = match condition {
                    FloatCondition::EqualOrdered => Condition::Equal,
                    FloatCondition::NotEqualOrUnordered => Condition::NotEqual,
                    FloatCondition::BelowOrdered => Condition::Below,
                    FloatCondition::BelowEqualOrdered => Condition::BelowEqual,
                    FloatCondition::Above => Condition::Above,
                    FloatCondition::AboveEqual => Condition::AboveEqual,
                };
                code.push(Instruction::Set {
                    condition: condition_code,
                    destination: r(2)?,
                });
                if condition.parity() {
                    let unordered = *condition == FloatCondition::NotEqualOrUnordered;
                    code.push(Instruction::Set {
                        condition: if unordered {
                            Condition::Parity
                        } else {
                            Condition::NotParity
                        },
                        destination: s(0)?,
                    });
                    code.push(Instruction::Alu {
                        op: if unordered { Alu::Or } else { Alu::And },
                        float: false,
                        bits: 8,
                        source: Operand::Register(s(0)?),
                        destination: r(2)?,
                    });
                }
            }
            Opcode::Numeric(numeric) => match numeric {
                Numeric::Boundary(_) => {}
                Numeric::Cell { cell, .. } => match cell {
                    Cell::Copy => code.push(mov(bits(1), o(0)?, o(1)?)),
                    Cell::Narrow => code.push(mov(bits(1), o(0)?, o(1)?)),
                    Cell::FloatBits => code.push(mov(64, o(0)?, o(1)?)),
                    Cell::ZeroExtend | Cell::SignedToFloat => code.push(Instruction::Convert {
                        op: if *cell == Cell::ZeroExtend {
                            Convert::ZeroExtend
                        } else {
                            Convert::SignedToFloat
                        },
                        input_bits: bits(0),
                        output_bits: bits(1),
                        source: r(0)?,
                        destination: r(1)?,
                    }),
                },
                Numeric::CheckedTruncate { .. } => code.push(Instruction::Convert {
                    op: Convert::TruncateFloat,
                    input_bits: 64,
                    output_bits: 64,
                    source: r(0)?,
                    destination: r(1)?,
                }),
                Numeric::Dividend { signed, .. } => {
                    if *signed {
                        code.push(Instruction::DividendSignExtend)
                    } else {
                        code.push(Instruction::Alu {
                            op: Alu::Xor,
                            float: false,
                            bits: 32,
                            source: Operand::Register(r(1)?),
                            destination: r(1)?,
                        })
                    }
                }
                Numeric::Divide { signed, .. } => code.push(Instruction::Divide {
                    signed: *signed,
                    divisor: r(2)?,
                }),
                Numeric::Shift { direction, .. } => code.push(Instruction::Shift {
                    op: match direction {
                        crate::backend::lir::ShiftDirection::Left => Shift::Left,
                        crate::backend::lir::ShiftDirection::ArithmeticRight => {
                            Shift::ArithmeticRight
                        }
                        crate::backend::lir::ShiftDirection::LogicalRight => Shift::LogicalRight,
                    },
                    bits: bits(2),
                    destination: r(2)?,
                    variable: true,
                }),
                Numeric::ShiftOne { .. } => code.push(Instruction::Shift {
                    op: Shift::LogicalRight,
                    bits: 64,
                    destination: r(1)?,
                    variable: false,
                }),
            },
            Opcode::SymbolAddress { symbol, .. } => code.push(Instruction::RipAddress {
                artifact: *symbol,
                destination: r(0)?,
            }),
            Opcode::ObjectAddress { object, .. } => {
                if self.frame.object_access(site, *object) != Some(AddressRecipe::Direct) {
                    return Err(RealizeError::UnsupportedRecipe(site));
                }
                let region = self
                    .frame
                    .object(*object)
                    .ok_or(RealizeError::UnsupportedRecipe(site))?;
                code.push(Instruction::Lea {
                    source: Operand::Memory {
                        base: match region.base {
                            Base::Frame => super::super::Gpr::Rbp,
                            Base::Stack => super::super::Gpr::Rsp,
                        },
                        displacement: i32::try_from(region.offset)
                            .map_err(|_| RealizeError::Overflow)?,
                    },
                    destination: r(0)?,
                });
            }
            Opcode::ByteOffset { .. } => code.push(Instruction::Alu {
                op: Alu::Add,
                float: false,
                bits: 64,
                source: o(1)?,
                destination: r(2)?,
            }),
            Opcode::Load { .. } | Opcode::TraceLoad { .. } => {
                let bytes = if let Opcode::Load { bytes, .. } = selected.opcode() {
                    *bytes
                } else {
                    8
                };
                let Register::Gpr(base) = r(0)? else {
                    return Err(RealizeError::InvalidResource);
                };
                code.push(mov(
                    (bytes * 8) as u16,
                    Operand::Memory {
                        base,
                        displacement: 0,
                    },
                    o(1)?,
                ));
            }
            Opcode::Store { .. } | Opcode::TraceStore { .. } => {
                let bytes = if let Opcode::Store { bytes, .. } = selected.opcode() {
                    *bytes
                } else {
                    8
                };
                let Register::Gpr(base) = r(0)? else {
                    return Err(RealizeError::InvalidResource);
                };
                code.push(mov(
                    (bytes * 8) as u16,
                    o(1)?,
                    Operand::Memory {
                        base,
                        displacement: 0,
                    },
                ));
            }
            Opcode::TlsAddress { .. } => {
                code.push(Instruction::TlsBase { destination: r(0)? });
                code.push(Instruction::TlsOffset {
                    artifact: crate::backend::plan::ArtifactId::TraceTls,
                    destination: r(0)?,
                });
            }
            Opcode::Call(call) => {
                let target = match call.target {
                    SelectedCallTarget::Direct(artifact) => CallTarget::Direct(artifact),
                    SelectedCallTarget::Indirect(_) => {
                        CallTarget::Indirect(r(call.arguments.len())?)
                    }
                };
                code.push(Instruction::Call(target));
                if call.never {
                    code.push(Instruction::HardTrap);
                }
            }
            Opcode::Failure { .. } => {
                code.push(Instruction::Call(CallTarget::Direct(
                    crate::backend::plan::ArtifactId::Runtime(
                        crate::backend::plan::RuntimeService::Panic,
                    ),
                )));
                code.push(Instruction::HardTrap);
            }
            Opcode::HardTrap => code.push(Instruction::HardTrap),
            Opcode::Lifetime { .. } | Opcode::Return { .. } => {}
            Opcode::Jump => code.push(Instruction::Jump(successors[0])),
            Opcode::Branch { .. } | Opcode::CheckBranch { .. } => {
                code.push(Instruction::Compare {
                    float: false,
                    bits: bits(0),
                    left: o(0)?,
                    right: Operand::Immediate(0),
                });
                code.push(Instruction::JumpIf {
                    condition: Condition::NotEqual,
                    destination: successors[0],
                });
                code.push(Instruction::Jump(successors[1]));
            }
        }
        if let Some(Bundle::Bounded { steps, .. }) = d.bundle {
            if code.len() > usize::from(steps.get()) {
                return Err(RealizeError::RecipeBound(site));
            }
        }
        Ok(Group {
            origin: Origin::Selected(site),
            instructions: code,
            dependencies: d.artifacts.iter().map(|(id, _)| *id).collect(),
        })
    }
}
fn alu(op: BinaryOperation) -> Alu {
    match op {
        BinaryOperation::Add => Alu::Add,
        BinaryOperation::Subtract => Alu::Subtract,
        BinaryOperation::Multiply => Alu::Multiply,
        BinaryOperation::FloatDivide => Alu::DivideFloat,
        BinaryOperation::And => Alu::And,
        BinaryOperation::Or => Alu::Or,
        BinaryOperation::Xor => Alu::Xor,
    }
}
fn integer_condition(c: IntegerCondition) -> Condition {
    match c {
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
    }
}
