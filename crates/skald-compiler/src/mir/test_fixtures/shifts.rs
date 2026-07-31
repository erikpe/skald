//! Verified checked-shift fixtures shared by MIR and backend tests.

use super::*;
use crate::mir::{MirShiftCountCheck, MirShiftOperation, MirTerminationReason};

/// Builds one executable checked shift. A correct result returns zero, a
/// mismatched result returns 91, and an invalid count takes the language panic
/// edge before any target shift executes.
pub(crate) fn checked_shift_program(
    operation: MirShiftOperation,
    left_bits: u64,
    count_bits: u64,
    expected_bits: u64,
) -> MirProgram {
    let mut program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .expect("fixture entry function exists");
    let callable = definition.function;
    let span = definition.span;
    let left_storage = StorageId::new(callable, 0);
    let count_storage = StorageId::new(callable, 1);
    let result_storage = StorageId::new(callable, 2);
    definition.storage = vec![
        storage(
            left_storage,
            None,
            "shift-left",
            MirStorageKind::ScalarSpill,
            operation.left_type(),
            span,
        ),
        storage(
            count_storage,
            None,
            "shift-count",
            MirStorageKind::ScalarSpill,
            operation.count_type(),
            span,
        ),
        storage(
            result_storage,
            None,
            "shift-result",
            MirStorageKind::ScalarSpill,
            operation.result_type(),
            span,
        ),
    ];

    let mut values = Vec::new();
    let left = next_fixture_value(callable, &mut values, operation.left_type(), span);
    let count = next_fixture_value(callable, &mut values, operation.count_type(), span);
    let secured_left = next_fixture_value(callable, &mut values, operation.left_type(), span);
    let secured_count = next_fixture_value(callable, &mut values, operation.count_type(), span);
    let shifted = next_fixture_value(callable, &mut values, operation.result_type(), span);
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
                    storage_live(left_storage, span),
                    storage_live(count_storage, span),
                    storage_live(result_storage, span),
                    assign(
                        left,
                        fixture_integer_constant(operation.left, left_bits),
                        operation.left_type(),
                        span,
                    ),
                    store(MirPlace::base(left_storage), left, span),
                    assign(
                        count,
                        MirRvalueKind::ConstantU64(count_bits),
                        MirType::U64,
                        span,
                    ),
                    store(MirPlace::base(count_storage), count, span),
                ],
                Some(MirTerminator::ShiftCountCheck {
                    check: MirShiftCountCheck {
                        operation,
                        left: left_storage,
                        count: count_storage,
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
                        secured_left,
                        MirRvalueKind::Load(MirPlace::base(left_storage)),
                        operation.left_type(),
                        span,
                    ),
                    assign(
                        secured_count,
                        MirRvalueKind::Load(MirPlace::base(count_storage)),
                        MirType::U64,
                        span,
                    ),
                    assign(
                        shifted,
                        MirRvalueKind::Shift {
                            operation,
                            left: secured_left,
                            count: secured_count,
                        },
                        operation.result_type(),
                        span,
                    ),
                    store(MirPlace::base(result_storage), shifted, span),
                ],
                Some(MirTerminator::Goto { target: join, span }),
                span,
            ),
            block(
                failure,
                vec![],
                Some(MirTerminator::Terminate {
                    reason: MirTerminationReason::ShiftCountOutOfRange,
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
                        fixture_integer_constant(operation.left, expected_bits),
                        operation.result_type(),
                        span,
                    ),
                    assign(
                        equal,
                        MirRvalueKind::PrimitiveComparison {
                            operation: MirPrimitiveComparison {
                                predicate: MirComparisonPredicate::Equal,
                                operand: MirComparisonOperand::Integer(operation.left),
                            },
                            left: result,
                            right: expected,
                        },
                        MirType::Bool,
                        span,
                    ),
                    storage_dead(result_storage, span),
                    storage_dead(count_storage, span),
                    storage_dead(left_storage, span),
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
    super::super::verify_mir(&program).expect("checked shift fixture must be valid");
    program
}
