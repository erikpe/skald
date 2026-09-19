//! Small execution oracle for constant numeric fixtures, independent of MIR
//! diamond recognition. This tests lowered control/data associations, not native
//! instruction recipes. Unsupported operations fail rather than being skipped.
use crate::backend::{
    failure::FailureMessage,
    lir::{
        Constant, Conversion, DivisionResult, Operation, ScalarCheck, ShiftDirection, Terminator,
        UnaryOperation, VerifiedCallable,
    },
    plan::ScalarType,
};
use std::collections::BTreeMap;

pub(super) fn run(body: &VerifiedCallable<'_>) -> Result<Constant, FailureMessage> {
    run_with_inputs(body, &[])
}

pub(super) fn run_with_inputs(
    body: &VerifiedCallable<'_>,
    inputs: &[Constant],
) -> Result<Constant, FailureMessage> {
    let draft = body.draft();
    assert_eq!(draft.inputs().len(), inputs.len());
    let blocks = draft.blocks().map(|(_, b)| b).collect::<Vec<_>>();
    let mut values = draft
        .inputs()
        .iter()
        .copied()
        .zip(inputs.iter().copied())
        .collect::<BTreeMap<_, _>>();
    let mut addresses = BTreeMap::new();
    let mut memory = BTreeMap::new();
    let mut current = draft.entry().unwrap().index();
    for _ in 0..1000 {
        let block = blocks[current];
        let terminal = block.terminator.as_ref().unwrap();
        if let Terminator::ReportFailure { reason, .. } = terminal {
            return Err(*reason);
        }
        for instruction in &block.instructions {
            let result = match instruction.operation {
                Operation::Constant(value) => Some(value),
                Operation::ObjectAddress(object) => {
                    addresses.insert(instruction.results[0], object);
                    None
                }
                Operation::Store { address, value, .. } => {
                    memory.insert(addresses[&address], values[&value]);
                    None
                }
                Operation::Load { address, .. } => Some(memory[&addresses[&address]]),
                Operation::Lifetime { .. } => None,
                Operation::Unary {
                    operation: UnaryOperation::Negate,
                    value,
                } => Some(match values[&value] {
                    Constant::I64(n) => Constant::I64(n.wrapping_neg()),
                    Constant::F64(bits) => Constant::F64(bits ^ (1 << 63)),
                    other => panic!("invalid negation {other:?}"),
                }),
                Operation::Divide {
                    result,
                    dividend,
                    divisor,
                    ..
                } => Some(divide(values[&dividend], values[&divisor], result)),
                Operation::Shift {
                    direction,
                    value,
                    count,
                    ..
                } => Some(shift(values[&value], values[&count], direction)),
                Operation::Convert {
                    conversion,
                    value,
                    target,
                    ..
                } => Some(convert(values[&value], target, conversion)),
                ref other => panic!("unsupported numeric oracle operation: {other:?}"),
            };
            if let Some(result) = result {
                assert_eq!(instruction.results.len(), 1);
                values.insert(instruction.results[0], result);
            }
        }
        current = match terminal {
            Terminator::Jump(edge) => edge.target.index(),
            Terminator::ScalarCheck {
                relation,
                success,
                failure,
            } => {
                let valid = match relation {
                    ScalarCheck::NonZeroDivisor { divisor, .. } => match values[divisor] {
                        Constant::I64(n) => n != 0,
                        Constant::U64(n) => n != 0,
                        Constant::U8(n) => n != 0,
                        other => panic!("invalid divisor {other:?}"),
                    },
                    ScalarCheck::ShiftCountBelowWidth { count, width } => {
                        let Constant::U64(count) = values[count] else {
                            panic!("count is not u64")
                        };
                        count < u64::from(*width)
                    }
                    ScalarCheck::FiniteTruncatedF64InIntegerRange { source, target } => {
                        let Constant::F64(bits) = values[source] else {
                            panic!("source is not float")
                        };
                        let n = f64::from_bits(bits).trunc();
                        n.is_finite()
                            && match target {
                                ScalarType::I64 => (-9_223_372_036_854_775_808.0
                                    ..9_223_372_036_854_775_808.0)
                                    .contains(&n),
                                ScalarType::U64 => (0.0..18_446_744_073_709_551_616.0).contains(&n),
                                ScalarType::U8 => (0.0..256.0).contains(&n),
                                _ => panic!("invalid float target"),
                            }
                    }
                };
                if valid {
                    success.target.index()
                } else {
                    failure.target.index()
                }
            }
            Terminator::Return(results) => {
                assert_eq!(results.len(), 1);
                return Ok(values[&results[0]]);
            }
            other => panic!("unsupported numeric oracle terminal: {other:?}"),
        };
    }
    panic!("numeric fixture did not terminate")
}

fn divide(left: Constant, right: Constant, result: DivisionResult) -> Constant {
    match (left, right) {
        (Constant::I64(a), Constant::I64(b)) => {
            let (a, b) = (i128::from(a), i128::from(b));
            let mut q = a / b;
            if a % b != 0 && (a % b < 0) != (b < 0) {
                q -= 1;
            }
            Constant::I64(select(result, q, a - q * b) as i64)
        }
        (Constant::U64(a), Constant::U64(b)) => Constant::U64(select(result, a / b, a % b)),
        (Constant::U8(a), Constant::U8(b)) => Constant::U8(select(result, a / b, a % b)),
        other => panic!("invalid division {other:?}"),
    }
}
fn shift(value: Constant, count: Constant, direction: ShiftDirection) -> Constant {
    let Constant::U64(count) = count else {
        panic!("invalid count")
    };
    let count = u32::try_from(count).unwrap();
    match (value, direction) {
        (Constant::I64(n), ShiftDirection::Left) => Constant::I64(n.wrapping_shl(count)),
        (Constant::I64(n), ShiftDirection::ArithmeticRight) => Constant::I64(n >> count),
        (Constant::U64(n), ShiftDirection::Left) => Constant::U64(n.wrapping_shl(count)),
        (Constant::U64(n), ShiftDirection::LogicalRight) => Constant::U64(n >> count),
        (Constant::U8(n), ShiftDirection::Left) => Constant::U8(n.wrapping_shl(count)),
        (Constant::U8(n), ShiftDirection::LogicalRight) => Constant::U8(n >> count),
        other => panic!("invalid shift {other:?}"),
    }
}
fn convert(value: Constant, target: ScalarType, conversion: Conversion) -> Constant {
    match (conversion, value, target) {
        (Conversion::Identity, value, _) => value,
        (Conversion::FloatBits, Constant::U64(bits), ScalarType::F64) => Constant::F64(bits),
        (Conversion::FloatBits, Constant::F64(bits), ScalarType::U64) => Constant::U64(bits),
        (Conversion::TruncateFloat, Constant::F64(bits), target) => {
            let value = f64::from_bits(bits).trunc();
            match target {
                ScalarType::I64 => Constant::I64(value as i64),
                ScalarType::U64 => Constant::U64(value as u64),
                ScalarType::U8 => Constant::U8(value as u8),
                _ => panic!("invalid target"),
            }
        }
        other => panic!("unsupported conversion {other:?}"),
    }
}

fn select<T>(result: DivisionResult, quotient: T, remainder: T) -> T {
    if result == DivisionResult::Quotient {
        quotient
    } else {
        remainder
    }
}
