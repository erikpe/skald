//! Test-only concrete cell execution, independent of the selector and verifier.
use super::super::numeric::{Cell, Numeric};
use super::*;
use crate::{
    backend::{
        failure::FailureMessage,
        graph::{SelectedBlockId, SelectedObjectId, SelectedValueId},
        lir::{BinaryOperation, ShiftDirection, UnaryOperation},
    },
    primitive_comparison::PrimitiveComparisonPredicate as Predicate,
};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum Value {
    Bits(u64),
    Float(u64),
    Address(SelectedObjectId),
}
impl Value {
    fn bits(self) -> u64 {
        let Self::Bits(v) = self else {
            panic!("expected bits")
        };
        v
    }
    fn float(self) -> f64 {
        let Self::Float(v) = self else {
            panic!("expected float")
        };
        f64::from_bits(v)
    }
}
type Edges = Vec<(SelectedBlockId, Vec<SelectedValueId>)>;
type Terminal<'a> = (&'a Instruction, Edges);
struct Block<'a> {
    parameters: Vec<SelectedValueId>,
    instructions: Vec<&'a Instruction>,
    terminal: Option<Terminal<'a>>,
}
pub(super) fn run(
    body: &selected::VerifiedSelectedCallable<'_, Instruction>,
    inputs: &[Value],
) -> Result<Value, FailureMessage> {
    let mut entry = None;
    let mut values = BTreeMap::new();
    let mut blocks = BTreeMap::<SelectedBlockId, Block<'_>>::new();
    body.visit(|fact| {
        match fact {
            SelectedFact::Entry {
                entry: e,
                inputs: ids,
                ..
            } => {
                entry = e;
                assert_eq!(ids.len(), inputs.len());
                values.extend(ids.iter().copied().zip(inputs.iter().copied()));
            }
            SelectedFact::Block { id, parameters, .. } => {
                blocks.insert(
                    id,
                    Block {
                        parameters: parameters.to_vec(),
                        instructions: vec![],
                        terminal: None,
                    },
                );
            }
            SelectedFact::Instruction { block, payload, .. } => {
                blocks.get_mut(&block).unwrap().instructions.push(payload)
            }
            SelectedFact::Terminal {
                block,
                payload: Some(node),
                edges,
            } => blocks.get_mut(&block).unwrap().terminal = Some((node, edges.to_vec())),
            _ => {}
        }
        Ok::<_, std::convert::Infallible>(())
    })
    .unwrap();
    let mut current = entry.unwrap();
    let mut memory = BTreeMap::new();
    for _ in 0..1000 {
        let block = &blocks[&current];
        for node in &block.instructions {
            let mut results = vec![];
            let get = |v: ValueRef| values[&v.value];
            let mask = |v: ValueRef, n: u64| {
                if v.representation.bits() == 8 {
                    n & 255
                } else {
                    n
                }
            };
            match node.opcode() {
                Opcode::Constant { constant, out } => {
                    let v = match *constant {
                        lir::Constant::I64(n) => Value::Bits(n as u64),
                        lir::Constant::U64(n) => Value::Bits(n),
                        lir::Constant::U8(n) => Value::Bits(u64::from(n)),
                        lir::Constant::Bool(n) => Value::Bits(u64::from(n)),
                        lir::Constant::F64(n) => Value::Float(n),
                        lir::Constant::Null(_) => Value::Bits(0),
                    };
                    results.push((out.value, v));
                }
                Opcode::ObjectAddress { object, out } => {
                    results.push((out.value, Value::Address(*object)))
                }
                Opcode::Store { address, value, .. } => {
                    memory.insert(get(*address), get(*value));
                }
                Opcode::Load { address, out, .. } => {
                    results.push((out.value, memory[&get(*address)]))
                }
                Opcode::Lifetime { .. } => {}
                Opcode::Alu {
                    operation,
                    left,
                    right,
                    out,
                } => {
                    let a = get(*left);
                    let b = get(*right);
                    let v = if out.representation.kind == selected::RepresentationKind::Float {
                        Value::Float(
                            match operation {
                                BinaryOperation::Add => a.float() + b.float(),
                                BinaryOperation::Subtract => a.float() - b.float(),
                                BinaryOperation::Multiply => a.float() * b.float(),
                                BinaryOperation::FloatDivide => a.float() / b.float(),
                                _ => panic!("invalid float cell"),
                            }
                            .to_bits(),
                        )
                    } else {
                        let a = a.bits();
                        let b = b.bits();
                        Value::Bits(mask(
                            *out,
                            match operation {
                                BinaryOperation::Add => a.wrapping_add(b),
                                BinaryOperation::Subtract => a.wrapping_sub(b),
                                BinaryOperation::Multiply => a.wrapping_mul(b),
                                BinaryOperation::And => a & b,
                                BinaryOperation::Or => a | b,
                                BinaryOperation::Xor => a ^ b,
                                _ => panic!("invalid integer cell"),
                            },
                        ))
                    };
                    results.push((out.value, v));
                }
                Opcode::IntegerCompare {
                    predicate,
                    signed,
                    left,
                    right,
                    out,
                    ..
                } => {
                    let a = get(*left).bits();
                    let b = get(*right).bits();
                    let ordering = if *signed {
                        (a as i64).cmp(&(b as i64))
                    } else {
                        a.cmp(&b)
                    };
                    let yes = match predicate {
                        Predicate::Equal => ordering.is_eq(),
                        Predicate::NotEqual => !ordering.is_eq(),
                        Predicate::LessThan => ordering.is_lt(),
                        Predicate::LessEqual => !ordering.is_gt(),
                        Predicate::GreaterThan => ordering.is_gt(),
                        Predicate::GreaterEqual => !ordering.is_lt(),
                    };
                    results.push((out.value, Value::Bits(u64::from(yes))));
                }
                Opcode::FloatCompare {
                    predicate,
                    left,
                    right,
                    out,
                    ..
                } => {
                    let a = get(*left).float();
                    let b = get(*right).float();
                    let yes = match predicate {
                        Predicate::Equal => a == b,
                        Predicate::NotEqual => a != b,
                        Predicate::LessThan => a < b,
                        Predicate::LessEqual => a <= b,
                        Predicate::GreaterThan => a > b,
                        Predicate::GreaterEqual => a >= b,
                    };
                    results.push((out.value, Value::Bits(u64::from(yes))));
                }
                Opcode::Unary {
                    operation,
                    input,
                    out,
                } => {
                    let v = get(*input);
                    let v = match operation {
                        UnaryOperation::Negate if matches!(v, Value::Float(_)) => {
                            Value::Float(v.float().to_bits() ^ (1 << 63))
                        }
                        UnaryOperation::Negate => Value::Bits(mask(*out, v.bits().wrapping_neg())),
                        UnaryOperation::Complement => Value::Bits(mask(*out, !v.bits())),
                        UnaryOperation::LogicalNot => Value::Bits(u64::from(v.bits() == 0)),
                    };
                    results.push((out.value, v));
                }
                Opcode::Numeric(n) => match n {
                    Numeric::Boundary(_) => {}
                    Numeric::Cell { cell, input, out } => {
                        let v = get(*input);
                        let v = match cell {
                            Cell::Copy => v,
                            Cell::ZeroExtend => Value::Bits(v.bits() & 255),
                            Cell::Narrow => Value::Bits(v.bits() & 255),
                            Cell::SignedToFloat => Value::Float((v.bits() as i64 as f64).to_bits()),
                            Cell::FloatBits => match v {
                                Value::Bits(n) => Value::Float(n),
                                Value::Float(n) => Value::Bits(n),
                                _ => panic!("bad bit transfer"),
                            },
                        };
                        results.push((out.value, v));
                    }
                    Numeric::Dividend { signed, low, high } => results.push((
                        high.value,
                        Value::Bits(if *signed && get(*low).bits() >> 63 != 0 {
                            u64::MAX
                        } else {
                            0
                        }),
                    )),
                    Numeric::Divide {
                        signed,
                        low,
                        high,
                        divisor,
                        quotient,
                        remainder,
                        ..
                    } => {
                        let a = get(*low).bits();
                        let b = get(*divisor).bits();
                        assert_ne!(b, 0);
                        let (q, r) = if *signed {
                            assert!(!(a == i64::MIN as u64 && b == u64::MAX));
                            ((a as i64 / b as i64) as u64, (a as i64 % b as i64) as u64)
                        } else {
                            assert_eq!(get(*high).bits(), 0);
                            (a / b, a % b)
                        };
                        results.push((quotient.value, Value::Bits(q)));
                        results.push((remainder.value, Value::Bits(r)));
                    }
                    Numeric::Shift {
                        direction,
                        input,
                        count,
                        out,
                        ..
                    } => {
                        let n = get(*count).bits();
                        assert!(n < u64::from(input.representation.bits()));
                        let v = get(*input).bits();
                        let answer = match direction {
                            ShiftDirection::Left => v << n,
                            ShiftDirection::LogicalRight => v >> n,
                            ShiftDirection::ArithmeticRight => ((v as i64) >> n) as u64,
                        };
                        results.push((out.value, Value::Bits(mask(*out, answer))));
                    }
                    Numeric::ShiftOne { input, out } => {
                        results.push((out.value, Value::Bits(get(*input).bits() >> 1)))
                    }
                    Numeric::CheckedTruncate { input, out, .. } => {
                        let n = get(*input).float();
                        assert!((-9223372036854775808.0..9223372036854775808.0).contains(&n));
                        results.push((out.value, Value::Bits((n.trunc() as i64) as u64)));
                    }
                },
                Opcode::SymbolAddress { .. } => {}
                other => panic!("unsupported oracle instruction {other:?}"),
            }
            values.extend(results);
        }
        let (node, edges) = block.terminal.as_ref().unwrap();
        let slot = match node.opcode() {
            Opcode::Return {
                values: returns, ..
            } => return Ok(values[&returns[0].value]),
            Opcode::Failure { reason, .. } => return Err(*reason),
            Opcode::Jump => 0,
            Opcode::Branch { condition } | Opcode::CheckBranch { condition, .. } => {
                usize::from(values[&condition.value].bits() == 0)
            }
            _ => panic!("unsupported oracle terminal"),
        };
        let (to, args) = &edges[slot];
        let simultaneous = args.iter().map(|v| values[v]).collect::<Vec<_>>();
        for (param, value) in blocks[to].parameters.iter().zip(simultaneous) {
            values.insert(*param, value);
        }
        current = *to;
    }
    panic!("numeric fixture did not terminate")
}
