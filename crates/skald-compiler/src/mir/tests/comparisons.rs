use super::*;
use crate::hir::{
    HirComparisonOperand, HirExpression, HirExpressionKind, HirFunctionDefinition, HirReturnValue,
    HirStatement, HirUnaryOperation, Type,
};

const OPERATORS: &[(MirComparisonPredicate, &str, &str)] = &[
    (MirComparisonPredicate::Equal, "==", "eq"),
    (MirComparisonPredicate::NotEqual, "!=", "ne"),
    (MirComparisonPredicate::LessThan, "<", "lt"),
    (MirComparisonPredicate::LessEqual, "<=", "le"),
    (MirComparisonPredicate::GreaterThan, ">", "gt"),
    (MirComparisonPredicate::GreaterEqual, ">=", "ge"),
];

const INTEGER_TYPES: &[(MirIntegerType, &str, &str, &str)] = &[
    (MirIntegerType::I64, "i64", "1", "2"),
    (MirIntegerType::U64, "u64", "1u", "2u"),
    (MirIntegerType::U8, "u8", "1u8", "2u8"),
];

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

fn lower_manually_selected_floating_comparison(
    spelling: &str,
) -> (MirProgram, MirComparisonPredicate) {
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

    let predicate = match operation.predicate {
        crate::hir::HirComparisonPredicate::Equal => MirComparisonPredicate::Equal,
        crate::hir::HirComparisonPredicate::NotEqual => MirComparisonPredicate::NotEqual,
        crate::hir::HirComparisonPredicate::LessThan => MirComparisonPredicate::LessThan,
        crate::hir::HirComparisonPredicate::LessEqual => MirComparisonPredicate::LessEqual,
        crate::hir::HirComparisonPredicate::GreaterThan => MirComparisonPredicate::GreaterThan,
        crate::hir::HirComparisonPredicate::GreaterEqual => MirComparisonPredicate::GreaterEqual,
    };
    let mir = lower_hir(&hir);
    (mir, predicate)
}

fn lower_manually_selected_eager_boolean_operations() -> MirProgram {
    let mut hir = type_check_source(concat!(
        "fn invert() -> bool { return false; }\n",
        "fn compare() -> bool { return 1 != 2; }\n",
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

    lower_hir(&hir)
}

#[test]
fn lowers_and_verifies_every_integer_comparison_operation() {
    for &(integer, type_name, left, right) in INTEGER_TYPES {
        for &(predicate, spelling, mnemonic) in OPERATORS {
            let source = format!(
                "fn compare() -> bool {{ return {left} {spelling} {right}; }} \
                 fn main() -> i64 {{ return 0; }}"
            );
            let mir = lower_text(&source);
            verify_mir(&mir).unwrap();
            let comparison = mir
                .definitions
                .get(FunctionId::new(0))
                .unwrap()
                .body
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .find_map(|instruction| match instruction {
                    MirInstruction::Assign(MirAssignment {
                        rvalue:
                            MirRvalue {
                                kind:
                                    MirRvalueKind::PrimitiveComparison {
                                        operation,
                                        left,
                                        right,
                                    },
                                ty,
                            },
                        ..
                    }) => Some((*operation, *left, *right, *ty)),
                    _ => None,
                })
                .expect("comparison source must lower to a comparison rvalue");

            assert_eq!(
                comparison.0,
                MirPrimitiveComparison {
                    predicate,
                    operand: MirComparisonOperand::Integer(integer),
                }
            );
            assert_eq!(comparison.0.operand_type(), integer.operand_type());
            assert_eq!(comparison.0.result_type(), MirType::Bool);
            assert_eq!(comparison.3, MirType::Bool);
            assert_ne!(comparison.1, comparison.2);
            assert!(
                dump_mir(&mir).contains(&format!("{mnemonic}.{type_name}")),
                "{type_name} comparison {spelling} has an unstable MIR dump"
            );
        }
    }
}

#[test]
fn lowers_and_verifies_every_floating_comparison_as_a_pure_scalar_rvalue() {
    for &(_, spelling, mnemonic) in OPERATORS {
        let (mir, predicate) = lower_manually_selected_floating_comparison(spelling);
        verify_mir(&mir).unwrap();
        let function = mir.definitions.get(FunctionId::new(0)).unwrap();
        assert_eq!(function.body.blocks.len(), 1);

        let operation = function.body.blocks[0]
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                MirInstruction::Assign(MirAssignment {
                    rvalue:
                        MirRvalue {
                            kind: MirRvalueKind::PrimitiveComparison { operation, .. },
                            ty: MirType::Bool,
                        },
                    ..
                }) => Some(*operation),
                _ => None,
            })
            .expect("floating comparison must lower to one scalar rvalue");
        assert_eq!(
            operation,
            MirPrimitiveComparison {
                predicate,
                operand: MirComparisonOperand::F64,
            }
        );
        assert_eq!(operation.operand_type(), MirType::F64);
        assert_eq!(operation.result_type(), MirType::Bool);

        let dump = dump_mir(&mir);
        assert_eq!(dump, dump_mir(&mir));
        assert!(dump.contains(&format!("{mnemonic}.f64")));
    }
}

#[test]
fn floating_comparison_lowers_nested_operands_once_in_source_order() {
    let mut hir =
        type_check_source("fn compare() -> bool { return 1 < 2; } fn main() -> i64 { return 0; }")
            .hir
            .unwrap();
    let comparison = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap(),
    );
    let span = comparison.span;
    let HirExpressionKind::PrimitiveComparison {
        operation,
        left,
        right,
    } = &mut comparison.kind
    else {
        panic!("expected comparison expression");
    };
    operation.operand = HirComparisonOperand::F64;
    let division = |dividend: f64, divisor: f64| HirExpression {
        kind: HirExpressionKind::Binary {
            operation: crate::hir::HirBinaryOperation::DivideF64,
            left: Box::new(HirExpression {
                kind: HirExpressionKind::F64Bits(dividend.to_bits()),
                ty: Type::F64,
                span,
            }),
            right: Box::new(HirExpression {
                kind: HirExpressionKind::F64Bits(divisor.to_bits()),
                ty: Type::F64,
                span,
            }),
        },
        ty: Type::F64,
        span,
    };
    **left = division(8.0, 2.0);
    **right = division(9.0, 3.0);

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let dump = dump_mir(&mir);
    let operations: Vec<_> = dump
        .lines()
        .filter_map(|line| {
            if line.contains("div.f64") {
                Some("div")
            } else if line.contains("lt.f64") {
                Some("compare")
            } else {
                None
            }
        })
        .collect();
    assert_eq!(operations, ["div", "div", "compare"]);
    assert_eq!(
        mir.definitions
            .get(FunctionId::new(0))
            .unwrap()
            .body
            .blocks
            .len(),
        1,
        "pure floating operands and comparison must not introduce control flow"
    );
}

#[test]
fn lowers_manually_selected_eager_boolean_operations_as_pure_scalar_rvalues() {
    let mir = lower_manually_selected_eager_boolean_operations();
    verify_mir(&mir).unwrap();

    let invert = mir.definitions.get(FunctionId::new(0)).unwrap();
    assert_eq!(invert.body.blocks.len(), 1);
    assert!(invert.body.blocks[0]
        .instructions
        .iter()
        .any(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(MirAssignment {
                    rvalue: MirRvalue {
                        kind: MirRvalueKind::Unary {
                            operation: MirUnaryOperation::LogicalNotBool,
                            ..
                        },
                        ty: MirType::Bool,
                    },
                    ..
                })
            )
        }));

    let compare = mir.definitions.get(FunctionId::new(1)).unwrap();
    assert_eq!(compare.body.blocks.len(), 1);
    assert!(compare.body.blocks[0]
        .instructions
        .iter()
        .any(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(MirAssignment {
                    rvalue: MirRvalue {
                        kind: MirRvalueKind::PrimitiveComparison {
                            operation: MirPrimitiveComparison {
                                predicate: MirComparisonPredicate::NotEqual,
                                operand: MirComparisonOperand::Bool,
                            },
                            ..
                        },
                        ty: MirType::Bool,
                    },
                    ..
                })
            )
        }));

    let dump = dump_mir(&mir);
    assert_eq!(dump, dump_mir(&mir));
    assert!(dump.contains("not.bool"));
    assert!(dump.contains("ne.bool"));
}

#[test]
fn lowers_source_selected_eager_boolean_operations_deterministically() {
    let mir = lower_text(concat!(
        "fn invert(value: bool) -> bool { return !value; }\n",
        "fn compare(left: bool, right: bool) -> bool { return left == !right; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_mir(&mir).unwrap();

    let dump = dump_mir(&mir);
    assert_eq!(dump, dump_mir(&mir));
    assert!(dump.contains("not.bool"));
    assert!(dump.contains("eq.bool"));
    for function in [FunctionId::new(0), FunctionId::new(1)] {
        assert_eq!(
            mir.definitions.get(function).unwrap().body.blocks.len(),
            1,
            "eager boolean scalar operations must not introduce control flow"
        );
    }
}

#[test]
fn spills_the_left_operand_before_a_control_affecting_right_operand() {
    let mir = lower_text(concat!(
        "fn compare(left: u64, values: u64[]) -> bool {\n",
        "  return left < values[0];\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_mir(&mir).unwrap();

    let function = mir.definitions.get(FunctionId::new(0)).unwrap();
    let spill = function
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::ScalarSpill)
        .expect("the left operand must survive the right operand's block split");
    assert_eq!(spill.ty, MirType::U64);

    let dump = dump_mir(&mir);
    let spill_store = dump
        .find(&format!("store {}", spill.id))
        .expect("left operand must be stored before checked array access");
    let position_check = dump
        .find("array-position-check")
        .expect("right operand must retain checked array access");
    let spill_reload = dump
        .rfind(&format!("load {}", spill.id))
        .expect("left operand must be reloaded in the continuation block");
    let comparison = dump
        .find("lt.u64")
        .expect("continuation must perform the comparison");

    assert!(spill_store < position_check);
    assert!(position_check < spill_reload);
    assert!(spill_reload < comparison);
    assert_eq!(dump, dump_mir(&mir));
}
