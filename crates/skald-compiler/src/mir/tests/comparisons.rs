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
