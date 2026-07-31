//! Verified checked-division fixtures shared by MIR and backend tests.

use super::*;
use crate::mir::{MirIntegerDivisionOperation, MirIntegerDivisorCheck};

/// Builds one executable checked division or remainder. A correct result
/// returns zero, a mismatch returns 91, and a zero divisor takes the exact
/// language panic edge before any target divide instruction executes.
pub(crate) fn checked_integer_division_program(
    operation: MirIntegerDivisionOperation,
    dividend_bits: u64,
    divisor_bits: u64,
    expected_bits: u64,
) -> MirProgram {
    let mut program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .expect("fixture entry function exists");
    let callable = definition.function;
    let span = definition.span;
    let dividend_storage = StorageId::new(callable, 0);
    let divisor_storage = StorageId::new(callable, 1);
    let result_storage = StorageId::new(callable, 2);
    definition.storage = vec![
        storage(
            dividend_storage,
            None,
            "division-dividend",
            MirStorageKind::ScalarSpill,
            operation.operand_type(),
            span,
        ),
        storage(
            divisor_storage,
            None,
            "division-divisor",
            MirStorageKind::ScalarSpill,
            operation.operand_type(),
            span,
        ),
        storage(
            result_storage,
            None,
            "division-result",
            MirStorageKind::ScalarSpill,
            operation.result_type(),
            span,
        ),
    ];

    let mut values = Vec::new();
    let dividend = next_fixture_value(callable, &mut values, operation.operand_type(), span);
    let divisor = next_fixture_value(callable, &mut values, operation.operand_type(), span);
    let secured_dividend =
        next_fixture_value(callable, &mut values, operation.operand_type(), span);
    let secured_divisor = next_fixture_value(callable, &mut values, operation.operand_type(), span);
    let divided = next_fixture_value(callable, &mut values, operation.result_type(), span);
    let result = next_fixture_value(callable, &mut values, operation.result_type(), span);
    let expected = next_fixture_value(callable, &mut values, operation.result_type(), span);
    let equal = next_fixture_value(callable, &mut values, MirType::Bool, span);
    let ok = next_fixture_value(callable, &mut values, MirType::I64, span);
    let mismatch = next_fixture_value(callable, &mut values, MirType::I64, span);
    definition.values = values;

    let entry = BlockId::new(callable, 0);
    let success = BlockId::new(callable, 1);
    let failure = BlockId::new(callable, 2);
    let join = BlockId::new(callable, 3);
    let correct = BlockId::new(callable, 4);
    let incorrect = BlockId::new(callable, 5);
    definition.body = MirBody {
        entry,
        path_conditions: Vec::new(),
        logical_expressions: Vec::new(),
        blocks: vec![
            block(
                entry,
                vec![
                    storage_live(dividend_storage, span),
                    storage_live(divisor_storage, span),
                    storage_live(result_storage, span),
                    assign(
                        dividend,
                        fixture_integer_constant(operation.operand, dividend_bits),
                        operation.operand_type(),
                        span,
                    ),
                    store(MirPlace::base(dividend_storage), dividend, span),
                    assign(
                        divisor,
                        fixture_integer_constant(operation.operand, divisor_bits),
                        operation.operand_type(),
                        span,
                    ),
                    store(MirPlace::base(divisor_storage), divisor, span),
                ],
                Some(MirTerminator::IntegerDivisorCheck {
                    check: MirIntegerDivisorCheck {
                        operation,
                        dividend: dividend_storage,
                        divisor: divisor_storage,
                        result: result_storage,
                    },
                    success_target: success,
                    failure_target: failure,
                    span,
                }),
                span,
            ),
            block(
                success,
                vec![
                    assign(
                        secured_dividend,
                        MirRvalueKind::Load(MirPlace::base(dividend_storage)),
                        operation.operand_type(),
                        span,
                    ),
                    assign(
                        secured_divisor,
                        MirRvalueKind::Load(MirPlace::base(divisor_storage)),
                        operation.operand_type(),
                        span,
                    ),
                    assign(
                        divided,
                        MirRvalueKind::IntegerDivision {
                            operation,
                            dividend: secured_dividend,
                            divisor: secured_divisor,
                        },
                        operation.result_type(),
                        span,
                    ),
                    store(MirPlace::base(result_storage), divided, span),
                ],
                Some(MirTerminator::Goto { target: join, span }),
                span,
            ),
            block(
                failure,
                vec![],
                Some(MirTerminator::Terminate {
                    reason: operation.failure_reason(),
                    span,
                }),
                span,
            ),
            block(
                join,
                vec![
                    assign(
                        result,
                        MirRvalueKind::Load(MirPlace::base(result_storage)),
                        operation.result_type(),
                        span,
                    ),
                    assign(
                        expected,
                        fixture_integer_constant(operation.operand, expected_bits),
                        operation.result_type(),
                        span,
                    ),
                    assign(
                        equal,
                        MirRvalueKind::PrimitiveComparison {
                            operation: MirPrimitiveComparison {
                                predicate: MirComparisonPredicate::Equal,
                                operand: MirComparisonOperand::Integer(operation.operand),
                            },
                            left: result,
                            right: expected,
                        },
                        MirType::Bool,
                        span,
                    ),
                    storage_dead(result_storage, span),
                    storage_dead(divisor_storage, span),
                    storage_dead(dividend_storage, span),
                ],
                Some(MirTerminator::Branch {
                    condition: equal,
                    true_target: correct,
                    false_target: incorrect,
                    span,
                }),
                span,
            ),
            block(
                correct,
                vec![assign(
                    ok,
                    MirRvalueKind::ConstantI64(0),
                    MirType::I64,
                    span,
                )],
                Some(MirTerminator::Return {
                    value: Some(ok),
                    span,
                }),
                span,
            ),
            block(
                incorrect,
                vec![assign(
                    mismatch,
                    MirRvalueKind::ConstantI64(91),
                    MirType::I64,
                    span,
                )],
                Some(MirTerminator::Return {
                    value: Some(mismatch),
                    span,
                }),
                span,
            ),
        ],
    };
    super::super::verify_mir(&program).expect("checked integer-division fixture must be valid");
    program
}
