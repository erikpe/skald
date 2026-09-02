use crate::{
    identity::{ArrayTypeId, FunctionId},
    mir::{
        MirComparisonOperand, MirComparisonPredicate, MirF64ToIntegerRange,
        MirIntegerBitwiseOperation, MirIntegerDivisionKind, MirIntegerDivisionOperation,
        MirIntegerType, MirPathConditionValue, MirPlace, MirPrimitiveCast, MirPrimitiveComparison,
        MirPrimitiveType, MirRvalueKind, MirShiftDirection, MirShiftOperation, MirType,
        MirUnaryOperation, PathConditionId, StorageId, ValueId,
    },
};

use super::{evaluate_rvalue, PrimitiveConstant, PrimitiveEvaluation};

fn value(index: usize) -> ValueId {
    ValueId::new(FunctionId::new(0), index)
}

fn storage(index: usize) -> StorageId {
    StorageId::new(FunctionId::new(0), index)
}

fn evaluate(kind: &MirRvalueKind) -> PrimitiveEvaluation {
    evaluate_rvalue(kind, |_| None)
}

fn evaluate_unary(operation: MirUnaryOperation, operand: PrimitiveConstant) -> PrimitiveEvaluation {
    let operand_id = value(0);
    evaluate_rvalue(
        &MirRvalueKind::Unary {
            operation,
            operand: operand_id,
        },
        |candidate| (candidate == operand_id).then_some(operand),
    )
}

fn evaluate_binary(
    operation: crate::mir::MirBinaryOperation,
    left: PrimitiveConstant,
    right: PrimitiveConstant,
) -> PrimitiveEvaluation {
    let left_id = value(0);
    let right_id = value(1);
    evaluate_rvalue(
        &MirRvalueKind::Binary {
            operation,
            left: left_id,
            right: right_id,
        },
        |candidate| match candidate {
            candidate if candidate == left_id => Some(left),
            candidate if candidate == right_id => Some(right),
            _ => None,
        },
    )
}

fn evaluate_comparison(
    operation: MirPrimitiveComparison,
    left: PrimitiveConstant,
    right: PrimitiveConstant,
) -> PrimitiveEvaluation {
    let left_id = value(0);
    let right_id = value(1);
    evaluate_rvalue(
        &MirRvalueKind::PrimitiveComparison {
            operation,
            left: left_id,
            right: right_id,
        },
        |candidate| match candidate {
            candidate if candidate == left_id => Some(left),
            candidate if candidate == right_id => Some(right),
            _ => None,
        },
    )
}

fn evaluate_cast(operation: MirPrimitiveCast, operand: PrimitiveConstant) -> PrimitiveEvaluation {
    let operand_id = value(0);
    evaluate_rvalue(
        &MirRvalueKind::PrimitiveCast {
            operation,
            operand: operand_id,
        },
        |candidate| (candidate == operand_id).then_some(operand),
    )
}

fn folded(constant: PrimitiveConstant) -> PrimitiveEvaluation {
    PrimitiveEvaluation::Constant(constant)
}

fn bitwise(
    operation: MirIntegerBitwiseOperation,
    operand: MirIntegerType,
) -> crate::mir::MirBinaryOperation {
    crate::mir::MirBinaryOperation::IntegerBitwise { operation, operand }
}

fn comparison(
    predicate: MirComparisonPredicate,
    operand: impl Into<MirComparisonOperand>,
) -> MirPrimitiveComparison {
    MirPrimitiveComparison {
        predicate,
        operand: operand.into(),
    }
}

fn expected_comparison<T: Ord>(predicate: MirComparisonPredicate, left: T, right: T) -> bool {
    match predicate {
        MirComparisonPredicate::Equal => left == right,
        MirComparisonPredicate::NotEqual => left != right,
        MirComparisonPredicate::LessThan => left < right,
        MirComparisonPredicate::LessEqual => left <= right,
        MirComparisonPredicate::GreaterThan => left > right,
        MirComparisonPredicate::GreaterEqual => left >= right,
    }
}

const PREDICATES: [MirComparisonPredicate; 6] = [
    MirComparisonPredicate::Equal,
    MirComparisonPredicate::NotEqual,
    MirComparisonPredicate::LessThan,
    MirComparisonPredicate::LessEqual,
    MirComparisonPredicate::GreaterThan,
    MirComparisonPredicate::GreaterEqual,
];

#[test]
fn literal_constants_retain_exact_type_and_payload() {
    let cases = [
        (
            MirRvalueKind::ConstantI64(i64::MIN),
            PrimitiveConstant::I64(i64::MIN),
            MirType::I64,
        ),
        (
            MirRvalueKind::ConstantU64(u64::MAX),
            PrimitiveConstant::U64(u64::MAX),
            MirType::U64,
        ),
        (
            MirRvalueKind::ConstantU8(u8::MAX),
            PrimitiveConstant::U8(u8::MAX),
            MirType::U8,
        ),
        (
            MirRvalueKind::ConstantBool(true),
            PrimitiveConstant::Bool(true),
            MirType::Bool,
        ),
    ];

    for (kind, constant, ty) in cases {
        assert_eq!(evaluate(&kind), folded(constant));
        assert_eq!(constant.ty(), ty);
        assert_eq!(constant.into_rvalue_kind(), kind);
    }
}

#[test]
fn floating_literals_and_floating_operations_are_observably_unsupported() {
    assert_eq!(
        evaluate(&MirRvalueKind::ConstantF64Bits(1.5_f64.to_bits())),
        PrimitiveEvaluation::Unsupported
    );
    assert_eq!(
        evaluate_unary(MirUnaryOperation::NegateF64, PrimitiveConstant::I64(1)),
        PrimitiveEvaluation::Unsupported
    );

    for operation in [
        crate::mir::MirBinaryOperation::AddF64,
        crate::mir::MirBinaryOperation::SubtractF64,
        crate::mir::MirBinaryOperation::MultiplyF64,
        crate::mir::MirBinaryOperation::DivideF64,
    ] {
        assert_eq!(
            evaluate_binary(
                operation,
                PrimitiveConstant::I64(1),
                PrimitiveConstant::I64(2)
            ),
            PrimitiveEvaluation::Unsupported
        );
    }

    assert_eq!(
        evaluate_comparison(
            comparison(MirComparisonPredicate::Equal, MirComparisonOperand::F64),
            PrimitiveConstant::U64(0),
            PrimitiveConstant::U64(0),
        ),
        PrimitiveEvaluation::Unsupported
    );
}

#[test]
fn unary_operations_have_exact_wrapping_width_and_boolean_semantics() {
    for value in [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX] {
        assert_eq!(
            evaluate_unary(MirUnaryOperation::NegateI64, PrimitiveConstant::I64(value)),
            folded(PrimitiveConstant::I64(value.wrapping_neg()))
        );
        assert_eq!(
            evaluate_unary(
                MirUnaryOperation::BitwiseComplement(MirIntegerType::I64),
                PrimitiveConstant::I64(value),
            ),
            folded(PrimitiveConstant::I64(!value))
        );
    }

    for value in [0, 1, u64::MAX - 1, u64::MAX] {
        assert_eq!(
            evaluate_unary(
                MirUnaryOperation::BitwiseComplement(MirIntegerType::U64),
                PrimitiveConstant::U64(value),
            ),
            folded(PrimitiveConstant::U64(!value))
        );
    }

    for value in u8::MIN..=u8::MAX {
        assert_eq!(
            evaluate_unary(
                MirUnaryOperation::BitwiseComplement(MirIntegerType::U8),
                PrimitiveConstant::U8(value),
            ),
            folded(PrimitiveConstant::U8(!value))
        );
    }

    for value in [false, true] {
        assert_eq!(
            evaluate_unary(
                MirUnaryOperation::LogicalNotBool,
                PrimitiveConstant::Bool(value)
            ),
            folded(PrimitiveConstant::Bool(!value))
        );
    }

    assert_eq!(
        evaluate_unary(MirUnaryOperation::NegateI64, PrimitiveConstant::U64(1)),
        PrimitiveEvaluation::Unsupported
    );
    assert_eq!(
        evaluate_unary(
            MirUnaryOperation::BitwiseComplement(MirIntegerType::U8),
            PrimitiveConstant::U64(1),
        ),
        PrimitiveEvaluation::Unsupported
    );
    assert_eq!(
        evaluate_unary(MirUnaryOperation::LogicalNotBool, PrimitiveConstant::U8(1)),
        PrimitiveEvaluation::Unsupported
    );
}

#[test]
fn i64_binary_operations_wrap_at_both_extrema() {
    let values = [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX];
    let operations = [
        (
            crate::mir::MirBinaryOperation::AddI64,
            i64::wrapping_add as fn(i64, i64) -> i64,
        ),
        (
            crate::mir::MirBinaryOperation::SubtractI64,
            i64::wrapping_sub as fn(i64, i64) -> i64,
        ),
        (
            crate::mir::MirBinaryOperation::MultiplyI64,
            i64::wrapping_mul as fn(i64, i64) -> i64,
        ),
    ];

    for (operation, expected) in operations {
        for left in values {
            for right in values {
                assert_eq!(
                    evaluate_binary(
                        operation,
                        PrimitiveConstant::I64(left),
                        PrimitiveConstant::I64(right),
                    ),
                    folded(PrimitiveConstant::I64(expected(left, right)))
                );
            }
        }
    }
}

#[test]
fn u64_binary_operations_wrap_at_both_extrema() {
    let values = [0, 1, u64::MAX - 1, u64::MAX];
    let operations = [
        (
            crate::mir::MirBinaryOperation::AddU64,
            u64::wrapping_add as fn(u64, u64) -> u64,
        ),
        (
            crate::mir::MirBinaryOperation::SubtractU64,
            u64::wrapping_sub as fn(u64, u64) -> u64,
        ),
        (
            crate::mir::MirBinaryOperation::MultiplyU64,
            u64::wrapping_mul as fn(u64, u64) -> u64,
        ),
    ];

    for (operation, expected) in operations {
        for left in values {
            for right in values {
                assert_eq!(
                    evaluate_binary(
                        operation,
                        PrimitiveConstant::U64(left),
                        PrimitiveConstant::U64(right),
                    ),
                    folded(PrimitiveConstant::U64(expected(left, right)))
                );
            }
        }
    }
}

#[test]
fn all_u8_binary_inputs_are_canonicalized_explicitly() {
    for left in u8::MIN..=u8::MAX {
        for right in u8::MIN..=u8::MAX {
            let left_bits = u64::from(left);
            let right_bits = u64::from(right);
            let arithmetic = [
                (
                    crate::mir::MirBinaryOperation::AddU8,
                    left_bits.wrapping_add(right_bits) as u8,
                ),
                (
                    crate::mir::MirBinaryOperation::SubtractU8,
                    left_bits.wrapping_sub(right_bits) as u8,
                ),
                (
                    crate::mir::MirBinaryOperation::MultiplyU8,
                    left_bits.wrapping_mul(right_bits) as u8,
                ),
            ];
            for (operation, expected) in arithmetic {
                assert_eq!(
                    evaluate_binary(
                        operation,
                        PrimitiveConstant::U8(left),
                        PrimitiveConstant::U8(right),
                    ),
                    folded(PrimitiveConstant::U8(expected))
                );
            }

            for operation in [
                MirIntegerBitwiseOperation::And,
                MirIntegerBitwiseOperation::Or,
                MirIntegerBitwiseOperation::Xor,
            ] {
                let expected = match operation {
                    MirIntegerBitwiseOperation::And => left & right,
                    MirIntegerBitwiseOperation::Or => left | right,
                    MirIntegerBitwiseOperation::Xor => left ^ right,
                };
                assert_eq!(
                    evaluate_binary(
                        bitwise(operation, MirIntegerType::U8),
                        PrimitiveConstant::U8(left),
                        PrimitiveConstant::U8(right),
                    ),
                    folded(PrimitiveConstant::U8(expected))
                );
            }
        }
    }
}

#[test]
fn i64_and_u64_bitwise_operations_preserve_all_bits() {
    for operation in [
        MirIntegerBitwiseOperation::And,
        MirIntegerBitwiseOperation::Or,
        MirIntegerBitwiseOperation::Xor,
    ] {
        for left in [i64::MIN, -1, 0, 1, i64::MAX] {
            for right in [i64::MIN, -1, 0, 1, i64::MAX] {
                let expected = match operation {
                    MirIntegerBitwiseOperation::And => left & right,
                    MirIntegerBitwiseOperation::Or => left | right,
                    MirIntegerBitwiseOperation::Xor => left ^ right,
                };
                assert_eq!(
                    evaluate_binary(
                        bitwise(operation, MirIntegerType::I64),
                        PrimitiveConstant::I64(left),
                        PrimitiveConstant::I64(right),
                    ),
                    folded(PrimitiveConstant::I64(expected))
                );
            }
        }

        for left in [0, 1, u64::MAX - 1, u64::MAX] {
            for right in [0, 1, u64::MAX - 1, u64::MAX] {
                let expected = match operation {
                    MirIntegerBitwiseOperation::And => left & right,
                    MirIntegerBitwiseOperation::Or => left | right,
                    MirIntegerBitwiseOperation::Xor => left ^ right,
                };
                assert_eq!(
                    evaluate_binary(
                        bitwise(operation, MirIntegerType::U64),
                        PrimitiveConstant::U64(left),
                        PrimitiveConstant::U64(right),
                    ),
                    folded(PrimitiveConstant::U64(expected))
                );
            }
        }
    }
}

#[test]
fn binary_operation_type_mismatches_are_unsupported() {
    assert_eq!(
        evaluate_binary(
            crate::mir::MirBinaryOperation::AddI64,
            PrimitiveConstant::I64(1),
            PrimitiveConstant::U64(2),
        ),
        PrimitiveEvaluation::Unsupported
    );
    assert_eq!(
        evaluate_binary(
            bitwise(MirIntegerBitwiseOperation::And, MirIntegerType::U64),
            PrimitiveConstant::I64(1),
            PrimitiveConstant::I64(2),
        ),
        PrimitiveEvaluation::Unsupported
    );
}

#[test]
fn integer_comparisons_use_the_encoded_signedness_and_width() {
    let signed = [i64::MIN, -1, 0, 1, i64::MAX];
    let unsigned = [0, 1, 1_u64 << 63, u64::MAX];

    for predicate in PREDICATES {
        for left in signed {
            for right in signed {
                assert_eq!(
                    evaluate_comparison(
                        comparison(predicate, MirIntegerType::I64),
                        PrimitiveConstant::I64(left),
                        PrimitiveConstant::I64(right),
                    ),
                    folded(PrimitiveConstant::Bool(expected_comparison(
                        predicate, left, right
                    )))
                );
            }
        }

        for left in unsigned {
            for right in unsigned {
                assert_eq!(
                    evaluate_comparison(
                        comparison(predicate, MirIntegerType::U64),
                        PrimitiveConstant::U64(left),
                        PrimitiveConstant::U64(right),
                    ),
                    folded(PrimitiveConstant::Bool(expected_comparison(
                        predicate, left, right
                    )))
                );
            }
        }
    }

    assert_eq!(
        evaluate_comparison(
            comparison(MirComparisonPredicate::LessThan, MirIntegerType::I64),
            PrimitiveConstant::I64(-1),
            PrimitiveConstant::I64(0),
        ),
        folded(PrimitiveConstant::Bool(true))
    );
    assert_eq!(
        evaluate_comparison(
            comparison(MirComparisonPredicate::LessThan, MirIntegerType::U64),
            PrimitiveConstant::U64(u64::MAX),
            PrimitiveConstant::U64(0),
        ),
        folded(PrimitiveConstant::Bool(false))
    );
}

#[test]
fn all_u8_comparison_inputs_use_unsigned_byte_ordering() {
    for predicate in PREDICATES {
        for left in u8::MIN..=u8::MAX {
            for right in u8::MIN..=u8::MAX {
                assert_eq!(
                    evaluate_comparison(
                        comparison(predicate, MirIntegerType::U8),
                        PrimitiveConstant::U8(left),
                        PrimitiveConstant::U8(right),
                    ),
                    folded(PrimitiveConstant::Bool(expected_comparison(
                        predicate, left, right
                    )))
                );
            }
        }
    }
}

#[test]
fn boolean_comparisons_are_canonical_and_reject_ordering() {
    for left in [false, true] {
        for right in [false, true] {
            assert_eq!(
                evaluate_comparison(
                    comparison(MirComparisonPredicate::Equal, MirComparisonOperand::Bool),
                    PrimitiveConstant::Bool(left),
                    PrimitiveConstant::Bool(right),
                ),
                folded(PrimitiveConstant::Bool(left == right))
            );
            assert_eq!(
                evaluate_comparison(
                    comparison(MirComparisonPredicate::NotEqual, MirComparisonOperand::Bool,),
                    PrimitiveConstant::Bool(left),
                    PrimitiveConstant::Bool(right),
                ),
                folded(PrimitiveConstant::Bool(left != right))
            );
        }
    }

    for predicate in [
        MirComparisonPredicate::LessThan,
        MirComparisonPredicate::LessEqual,
        MirComparisonPredicate::GreaterThan,
        MirComparisonPredicate::GreaterEqual,
    ] {
        assert_eq!(
            evaluate_comparison(
                comparison(predicate, MirComparisonOperand::Bool),
                PrimitiveConstant::Bool(false),
                PrimitiveConstant::Bool(true),
            ),
            PrimitiveEvaluation::Unsupported
        );
    }
}

#[test]
fn identity_and_integer_width_casts_preserve_exact_bits() {
    let identities = [
        (MirPrimitiveType::I64, PrimitiveConstant::I64(i64::MIN)),
        (MirPrimitiveType::U64, PrimitiveConstant::U64(u64::MAX)),
        (MirPrimitiveType::U8, PrimitiveConstant::U8(u8::MAX)),
        (MirPrimitiveType::Bool, PrimitiveConstant::Bool(true)),
    ];
    for (ty, operand) in identities {
        assert_eq!(
            evaluate_cast(MirPrimitiveCast::new(ty, ty), operand),
            folded(operand)
        );
    }

    let integer_cases = [
        (MirPrimitiveType::I64, PrimitiveConstant::I64(-1), u64::MAX),
        (
            MirPrimitiveType::U64,
            PrimitiveConstant::U64(0x8000_0000_0000_00a5),
            0x8000_0000_0000_00a5,
        ),
        (MirPrimitiveType::U8, PrimitiveConstant::U8(0xa5), 0xa5),
    ];
    for (source, operand, bits) in integer_cases {
        assert_eq!(
            evaluate_cast(
                MirPrimitiveCast::new(source, MirPrimitiveType::I64),
                operand
            ),
            folded(PrimitiveConstant::I64(bits as i64))
        );
        assert_eq!(
            evaluate_cast(
                MirPrimitiveCast::new(source, MirPrimitiveType::U64),
                operand
            ),
            folded(PrimitiveConstant::U64(bits))
        );
        assert_eq!(
            evaluate_cast(MirPrimitiveCast::new(source, MirPrimitiveType::U8), operand),
            folded(PrimitiveConstant::U8(bits as u8))
        );
    }
}

#[test]
fn boolean_integer_casts_use_zero_testing_and_canonical_zero_or_one() {
    let integer_cases = [
        (MirPrimitiveType::I64, PrimitiveConstant::I64(0), false),
        (MirPrimitiveType::I64, PrimitiveConstant::I64(-1), true),
        (MirPrimitiveType::U64, PrimitiveConstant::U64(0), false),
        (
            MirPrimitiveType::U64,
            PrimitiveConstant::U64(u64::MAX),
            true,
        ),
        (MirPrimitiveType::U8, PrimitiveConstant::U8(0), false),
        (MirPrimitiveType::U8, PrimitiveConstant::U8(u8::MAX), true),
    ];
    for (source, operand, expected) in integer_cases {
        assert_eq!(
            evaluate_cast(
                MirPrimitiveCast::new(source, MirPrimitiveType::Bool),
                operand
            ),
            folded(PrimitiveConstant::Bool(expected))
        );
    }

    for value in [false, true] {
        let expected = u64::from(value);
        assert_eq!(
            evaluate_cast(
                MirPrimitiveCast::new(MirPrimitiveType::Bool, MirPrimitiveType::I64),
                PrimitiveConstant::Bool(value),
            ),
            folded(PrimitiveConstant::I64(expected as i64))
        );
        assert_eq!(
            evaluate_cast(
                MirPrimitiveCast::new(MirPrimitiveType::Bool, MirPrimitiveType::U64),
                PrimitiveConstant::Bool(value),
            ),
            folded(PrimitiveConstant::U64(expected))
        );
        assert_eq!(
            evaluate_cast(
                MirPrimitiveCast::new(MirPrimitiveType::Bool, MirPrimitiveType::U8),
                PrimitiveConstant::Bool(value),
            ),
            folded(PrimitiveConstant::U8(expected as u8))
        );
    }
}

#[test]
fn cast_type_mismatches_and_every_floating_cast_family_are_unsupported() {
    assert_eq!(
        evaluate_cast(
            MirPrimitiveCast::new(MirPrimitiveType::I64, MirPrimitiveType::U64),
            PrimitiveConstant::U64(1),
        ),
        PrimitiveEvaluation::Unsupported
    );
    assert_eq!(
        evaluate_cast(
            MirPrimitiveCast::new(MirPrimitiveType::I64, MirPrimitiveType::F64),
            PrimitiveConstant::I64(1),
        ),
        PrimitiveEvaluation::Unsupported
    );
    assert_eq!(
        evaluate_cast(
            MirPrimitiveCast::new(MirPrimitiveType::Bool, MirPrimitiveType::F64),
            PrimitiveConstant::Bool(true),
        ),
        PrimitiveEvaluation::Unsupported
    );
    assert_eq!(
        evaluate_cast(
            MirPrimitiveCast::bit_reinterpretation(MirPrimitiveType::U64, MirPrimitiveType::F64,),
            PrimitiveConstant::U64(1.0_f64.to_bits()),
        ),
        PrimitiveEvaluation::Unsupported
    );
    assert_eq!(
        evaluate_cast(
            MirPrimitiveCast::new(MirPrimitiveType::F64, MirPrimitiveType::I64),
            PrimitiveConstant::I64(1),
        ),
        PrimitiveEvaluation::Unsupported
    );
}

#[test]
fn checked_and_semantic_rvalue_families_are_rejected_without_operand_lookup() {
    let left = value(0);
    let right = value(1);
    let mut lookup_count = 0;
    let mut assert_unsupported = |kind: MirRvalueKind| {
        assert_eq!(
            evaluate_rvalue(&kind, |_| {
                lookup_count += 1;
                Some(PrimitiveConstant::U64(1))
            }),
            PrimitiveEvaluation::Unsupported
        );
    };

    for kind in [
        MirIntegerDivisionKind::Quotient,
        MirIntegerDivisionKind::Remainder,
    ] {
        assert_unsupported(MirRvalueKind::IntegerDivision {
            operation: MirIntegerDivisionOperation {
                kind,
                operand: MirIntegerType::U64,
            },
            dividend: left,
            divisor: right,
        });
    }

    for direction in [MirShiftDirection::Left, MirShiftDirection::Right] {
        let operation = MirShiftOperation {
            direction,
            left: MirIntegerType::U64,
        };
        assert_unsupported(MirRvalueKind::Shift {
            operation,
            left,
            count: right,
        });
    }

    assert_unsupported(MirRvalueKind::CheckedF64ToInteger {
        relation: MirF64ToIntegerRange {
            target: MirIntegerType::I64,
        },
        operand: left,
    });
    assert_unsupported(MirRvalueKind::Load(MirPlace::base(storage(0))));
    assert_unsupported(MirRvalueKind::PathCondition(MirPathConditionValue {
        condition: PathConditionId::new(FunctionId::new(0), 0),
        activation: storage(0),
    }));
    assert_unsupported(MirRvalueKind::ArrayLength {
        source: MirPlace::base(storage(0)),
        array: ArrayTypeId::new(0),
    });

    assert_eq!(lookup_count, 0);
}

#[test]
fn missing_constant_facts_are_unsupported_without_guessing() {
    let operand = value(0);
    assert_eq!(
        evaluate_rvalue(
            &MirRvalueKind::Unary {
                operation: MirUnaryOperation::NegateI64,
                operand,
            },
            |_| None,
        ),
        PrimitiveEvaluation::Unsupported
    );
    assert_eq!(
        evaluate_rvalue(
            &MirRvalueKind::Binary {
                operation: crate::mir::MirBinaryOperation::AddI64,
                left: operand,
                right: value(1),
            },
            |candidate| (candidate == operand).then_some(PrimitiveConstant::I64(1)),
        ),
        PrimitiveEvaluation::Unsupported
    );
}
