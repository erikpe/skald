use crate::{
    identity::{FunctionId, LoopId},
    test_support::type_check_source,
};

use super::{
    dump_hir, HirBinaryOperation, HirBlock, HirCheckedIntegerDivision, HirComparisonOperand,
    HirControlEffects, HirExpression, HirExpressionKind, HirFunctionDefinition,
    HirIntegerBitwiseOperation, HirIntegerDivisionKind, HirIntegerDivisionOperation,
    HirIntegerType, HirLogicalExpression, HirLogicalOperation, HirPrimitiveCast,
    HirPrimitiveCastKind, HirPrimitiveComparison, HirPrimitiveType, HirReturnValue, HirStatement,
    HirUnaryOperation, HirWhile, Type,
};

fn returned_expression_mut(definition: &mut HirFunctionDefinition) -> &mut HirExpression {
    let HirStatement::Return(statement) = definition.body.statements.last_mut().unwrap() else {
        panic!("expected final return statement");
    };
    let HirReturnValue::Scalar(expression) =
        statement.value.as_mut().expect("expected return value")
    else {
        panic!("expected scalar return value");
    };
    expression
}

#[test]
fn dumps_manually_constructed_structured_while_deterministically() {
    let mut hir = type_check_source("fn main() -> i64 { return 0; }\n")
        .hir
        .unwrap();
    let entry = hir.entry_function;
    let definition = hir.definitions.get_mut_for_test(entry).unwrap();
    let span = definition.body.span;
    let loop_id = LoopId::new(entry, 0);
    let body = HirBlock {
        statements: vec![],
        effects: HirControlEffects::fallthrough(),
        span,
    };
    definition.body.statements.insert(
        0,
        HirStatement::While(HirWhile::new(
            loop_id,
            HirExpression {
                kind: HirExpressionKind::Boolean(true),
                ty: Type::Bool,
                span,
            },
            body,
            span,
        )),
    );

    let dump = dump_hir(&hir);
    let lines: Vec<_> = dump
        .lines()
        .filter(|line| {
            line.contains("While ")
                || line.trim_start().starts_with("Condition ")
                || line.trim_start().starts_with("Boolean ")
        })
        .map(|line| line.split(" @").next().unwrap().trim())
        .collect();
    assert_eq!(
        lines,
        ["While f0:loop0", "Condition", "Boolean true : bool",]
    );
}

#[test]
fn dumps_manually_constructed_eager_boolean_operations_deterministically() {
    let mut hir = type_check_source(concat!(
        "fn invert() -> bool { return false; }\n",
        "fn compare() -> bool { return 1 == 2; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();

    let invert = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap(),
    );
    let span = invert.span;
    invert.kind = HirExpressionKind::Unary {
        operation: HirUnaryOperation::LogicalNotBool,
        operand: Box::new(HirExpression {
            kind: HirExpressionKind::Boolean(false),
            ty: Type::Bool,
            span,
        }),
    };
    invert.ty = Type::Bool;

    let comparison = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(1))
            .unwrap(),
    );
    let HirExpressionKind::PrimitiveComparison {
        operation,
        left,
        right,
    } = &mut comparison.kind
    else {
        panic!("expected comparison expression");
    };
    operation.operand = HirComparisonOperand::Bool;
    left.kind = HirExpressionKind::Boolean(true);
    left.ty = Type::Bool;
    right.kind = HirExpressionKind::Boolean(false);
    right.ty = Type::Bool;

    assert_eq!(HirUnaryOperation::LogicalNotBool.operand_type(), Type::Bool);
    assert_eq!(HirUnaryOperation::LogicalNotBool.result_type(), Type::Bool);
    assert!(HirPrimitiveComparison {
        predicate: operation.predicate,
        operand: HirComparisonOperand::Bool,
    }
    .is_valid());

    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains("Unary LogicalNotBool : bool"));
    assert!(dump.contains("BooleanComparison eq.bool : bool"));
    assert!(!dump.contains("BooleanComparison lt.bool"));
}

#[test]
fn represents_every_floating_comparison_with_one_exact_operand_flavor() {
    for (spelling, mnemonic) in [
        ("==", "eq"),
        ("!=", "ne"),
        ("<", "lt"),
        ("<=", "le"),
        (">", "gt"),
        (">=", "ge"),
    ] {
        let mut hir = type_check_source(format!(
            "fn compare() -> bool {{ return 1 {spelling} 2; }} fn main() -> i64 {{ return 0; }}"
        ))
        .hir
        .unwrap();
        let comparison = returned_expression_mut(
            hir.definitions
                .get_mut_for_test(FunctionId::new(0))
                .unwrap(),
        );
        let HirExpressionKind::PrimitiveComparison {
            operation,
            left,
            right,
        } = &mut comparison.kind
        else {
            panic!("expected comparison expression");
        };
        operation.operand = HirComparisonOperand::F64;
        left.kind = HirExpressionKind::F64Bits(1.0_f64.to_bits());
        left.ty = Type::F64;
        right.kind = HirExpressionKind::F64Bits(2.0_f64.to_bits());
        right.ty = Type::F64;

        assert!(operation.is_valid());
        assert_eq!(operation.operand_type(), Type::F64);
        assert_eq!(operation.result_type(), Type::Bool);
        assert_eq!(comparison.ty, Type::Bool);

        let dump = dump_hir(&hir);
        assert_eq!(dump, dump_hir(&hir));
        assert!(dump.contains(&format!("FloatingComparison {mnemonic}.f64 : bool")));
    }
}

#[test]
fn dumps_manually_constructed_logical_expression_shape_deterministically() {
    let mut hir = type_check_source(concat!(
        "fn evaluate() -> bool { return false; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();
    let expression = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap(),
    );
    let span = expression.span;
    let boolean = |value| HirExpression {
        kind: HirExpressionKind::Boolean(value),
        ty: Type::Bool,
        span,
    };
    expression.kind = HirExpressionKind::Logical(Box::new(HirLogicalExpression::new(
        HirLogicalOperation::And,
        boolean(true),
        boolean(false),
    )));
    expression.ty = HirLogicalOperation::And.result_type();

    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains("Logical And : bool"));
    assert!(dump.contains("Left\n"));
    assert!(dump.contains("Right\n"));
}

#[test]
fn integer_bitwise_operations_retain_exact_types_and_dump_vocabulary() {
    for integer in [HirIntegerType::I64, HirIntegerType::U64, HirIntegerType::U8] {
        let complement = HirUnaryOperation::BitwiseComplement(integer);
        assert_eq!(complement.operand_type(), integer.operand_type());
        assert_eq!(complement.result_type(), integer.operand_type());

        for operation in [
            HirIntegerBitwiseOperation::And,
            HirIntegerBitwiseOperation::Or,
            HirIntegerBitwiseOperation::Xor,
        ] {
            let binary = HirBinaryOperation::IntegerBitwise {
                operation,
                operand: integer,
            };
            assert_eq!(binary.operand_type(), integer.operand_type());
            assert_eq!(binary.result_type(), integer.operand_type());
            assert!(matches!(operation.mnemonic(), "and" | "or" | "xor"));
        }
    }

    let mut hir = type_check_source(concat!(
        "fn complement() -> u8 { return 85u8; }\n",
        "fn combine() -> u8 { return 240u8 + 15u8; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();

    let complement = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap(),
    );
    let span = complement.span;
    complement.kind = HirExpressionKind::Unary {
        operation: HirUnaryOperation::BitwiseComplement(HirIntegerType::U8),
        operand: Box::new(HirExpression {
            kind: HirExpressionKind::U8(0x55),
            ty: Type::U8,
            span,
        }),
    };

    let combine = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(1))
            .unwrap(),
    );
    let HirExpressionKind::Binary { operation, .. } = &mut combine.kind else {
        panic!("expected binary expression");
    };
    *operation = HirBinaryOperation::IntegerBitwise {
        operation: HirIntegerBitwiseOperation::Or,
        operand: HirIntegerType::U8,
    };

    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains("Unary BitwiseComplement.u8 : u8"));
    assert!(dump.contains("Binary BitwiseOr.u8 : u8"));
}

#[test]
fn primitive_casts_classify_the_complete_frozen_matrix() {
    use HirPrimitiveCastKind::{
        CheckedF64ToInteger as Checked, FromBool, Identity, IntegerBits, ToBool, ToF64,
    };

    let types = [
        HirPrimitiveType::I64,
        HirPrimitiveType::U64,
        HirPrimitiveType::U8,
        HirPrimitiveType::F64,
        HirPrimitiveType::Bool,
    ];
    let expected = [
        [Identity, IntegerBits, IntegerBits, ToF64, ToBool],
        [IntegerBits, Identity, IntegerBits, ToF64, ToBool],
        [IntegerBits, IntegerBits, Identity, ToF64, ToBool],
        [Checked, Checked, Checked, Identity, ToBool],
        [FromBool, FromBool, FromBool, ToF64, Identity],
    ];

    for (source_index, source) in types.into_iter().enumerate() {
        for (target_index, target) in types.into_iter().enumerate() {
            let operation = HirPrimitiveCast::new(source, target);
            let expected = expected[source_index][target_index];

            assert_eq!(
                operation.kind(),
                expected,
                "{} to {}",
                source.name(),
                target.name()
            );
            assert_eq!(operation.source_type(), source.value_type());
            assert_eq!(operation.result_type(), target.value_type());
            assert_eq!(operation.may_terminate(), expected == Checked);
        }
    }
}

#[test]
fn floating_division_retains_exact_type_and_dump_vocabulary() {
    let operation = HirBinaryOperation::DivideF64;
    assert_eq!(operation.operand_type(), Type::F64);
    assert_eq!(operation.result_type(), Type::F64);

    let mut hir = type_check_source(concat!(
        "fn divide(left: f64, right: f64) -> f64 { return left * right; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();
    let expression = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap(),
    );
    let HirExpressionKind::Binary { operation, .. } = &mut expression.kind else {
        panic!("expected binary expression");
    };
    *operation = HirBinaryOperation::DivideF64;

    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains("Binary DivideF64 : f64"));
}

#[test]
fn dumps_manually_constructed_integer_division_semantics_deterministically() {
    let mut hir = type_check_source(concat!(
        "fn quotient() -> i64 { return 7 + -3; }\n",
        "fn remainder() -> u8 { return 7u8 + 3u8; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();

    for (function, kind, operand) in [
        (
            FunctionId::new(0),
            HirIntegerDivisionKind::Quotient,
            HirIntegerType::I64,
        ),
        (
            FunctionId::new(1),
            HirIntegerDivisionKind::Remainder,
            HirIntegerType::U8,
        ),
    ] {
        let expression =
            returned_expression_mut(hir.definitions.get_mut_for_test(function).unwrap());
        let HirExpressionKind::Binary { left, right, .. } = &expression.kind else {
            panic!("expected source binary expression");
        };
        let operation = HirIntegerDivisionOperation { kind, operand };
        expression.kind = HirExpressionKind::CheckedIntegerDivision(Box::new(
            HirCheckedIntegerDivision::new(operation, (**left).clone(), (**right).clone()),
        ));
        expression.ty = operation.result_type();
    }

    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains(concat!(
        "CheckedIntegerDivision div.i64 signed-quotient=floor ",
        "signed-remainder-sign=divisor minimum-pair=minimum ",
        "failure=integer-division-by-zero : i64"
    )));
    assert!(dump.contains("CheckedIntegerDivision rem.u8 failure=integer-remainder-by-zero : u8"));
    assert!(dump.contains("Dividend\n"));
    assert!(dump.contains("Divisor\n"));
}
