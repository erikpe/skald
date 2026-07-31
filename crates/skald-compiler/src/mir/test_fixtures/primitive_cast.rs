//! Verified checked primitive-cast fixtures shared by MIR and backend tests.

use super::*;
use crate::mir::{MirF64ToIntegerRange, MirPrimitiveCastRangeCheck, MirTerminationReason};

/// Builds one executable checked floating-to-integer cast. A correct result
/// returns zero, a mismatch returns 91, and an invalid source takes the exact
/// language panic edge before conversion or result initialization.
pub(crate) fn checked_primitive_cast_program(
    source_bits: u64,
    target: MirIntegerType,
    expected_bits: u64,
) -> MirProgram {
    let mut program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .expect("fixture entry function exists");
    let callable = definition.function;
    let span = definition.span;
    let source_storage = StorageId::new(callable, 0);
    let result_storage = StorageId::new(callable, 1);
    let relation = MirF64ToIntegerRange { target };
    definition.storage = vec![
        storage(
            source_storage,
            None,
            "primitive-cast-source",
            MirStorageKind::ScalarSpill,
            MirType::F64,
            span,
        ),
        storage(
            result_storage,
            None,
            "primitive-cast-result",
            MirStorageKind::ScalarSpill,
            relation.result_type(),
            span,
        ),
    ];

    let mut values = Vec::new();
    let source = next_fixture_value(callable, &mut values, MirType::F64, span);
    let secured_source = next_fixture_value(callable, &mut values, MirType::F64, span);
    let converted = next_fixture_value(callable, &mut values, relation.result_type(), span);
    let result = next_fixture_value(callable, &mut values, relation.result_type(), span);
    let expected = next_fixture_value(callable, &mut values, relation.result_type(), span);
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
                    storage_live(source_storage, span),
                    storage_live(result_storage, span),
                    assign(
                        source,
                        MirRvalueKind::ConstantF64Bits(source_bits),
                        MirType::F64,
                        span,
                    ),
                    store(MirPlace::base(source_storage), source, span),
                ],
                Some(MirTerminator::PrimitiveCastRangeCheck {
                    check: MirPrimitiveCastRangeCheck {
                        relation,
                        source: source_storage,
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
                        secured_source,
                        MirRvalueKind::Load(MirPlace::base(source_storage)),
                        MirType::F64,
                        span,
                    ),
                    assign(
                        converted,
                        MirRvalueKind::CheckedF64ToInteger {
                            relation,
                            operand: secured_source,
                        },
                        relation.result_type(),
                        span,
                    ),
                    store(MirPlace::base(result_storage), converted, span),
                ],
                Some(MirTerminator::Goto { target: join, span }),
                span,
            ),
            block(
                failure,
                vec![],
                Some(MirTerminator::Terminate {
                    reason: MirTerminationReason::PrimitiveCastOutOfRange,
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
                        relation.result_type(),
                        span,
                    ),
                    assign(
                        expected,
                        fixture_integer_constant(target, expected_bits),
                        relation.result_type(),
                        span,
                    ),
                    assign(
                        equal,
                        MirRvalueKind::PrimitiveComparison {
                            operation: MirPrimitiveComparison {
                                predicate: MirComparisonPredicate::Equal,
                                operand: MirComparisonOperand::Integer(target),
                            },
                            left: result,
                            right: expected,
                        },
                        MirType::Bool,
                        span,
                    ),
                    storage_dead(result_storage, span),
                    storage_dead(source_storage, span),
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
    super::super::verify_mir(&program).expect("checked primitive-cast fixture must be valid");
    program
}
