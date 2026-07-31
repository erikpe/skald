use super::*;
use crate::hir::{
    dump_hir, HirBinaryOperation, HirExpression, HirExpressionKind, HirFunctionDefinition,
    HirIntegerBitwiseOperation, HirIntegerType, HirReturnValue, HirStatement, HirUnaryOperation,
    Type,
};

const INTEGER_TYPES: [(HirIntegerType, MirIntegerType, Type, &str); 3] = [
    (HirIntegerType::I64, MirIntegerType::I64, Type::I64, "i64"),
    (HirIntegerType::U64, MirIntegerType::U64, Type::U64, "u64"),
    (HirIntegerType::U8, MirIntegerType::U8, Type::U8, "u8"),
];

const BINARY_OPERATIONS: [(HirIntegerBitwiseOperation, MirIntegerBitwiseOperation, &str); 3] = [
    (
        HirIntegerBitwiseOperation::And,
        MirIntegerBitwiseOperation::And,
        "and",
    ),
    (
        HirIntegerBitwiseOperation::Or,
        MirIntegerBitwiseOperation::Or,
        "or",
    ),
    (
        HirIntegerBitwiseOperation::Xor,
        MirIntegerBitwiseOperation::Xor,
        "xor",
    ),
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

fn integer_expression(
    integer: HirIntegerType,
    bits: u64,
    span: crate::source::Span,
) -> HirExpression {
    let (kind, ty) = match integer {
        HirIntegerType::I64 => (HirExpressionKind::I64(bits as i64), Type::I64),
        HirIntegerType::U64 => (HirExpressionKind::U64(bits), Type::U64),
        HirIntegerType::U8 => (HirExpressionKind::U8(bits as u8), Type::U8),
    };
    HirExpression { kind, ty, span }
}

fn manually_selected_bitwise_hir() -> crate::hir::HirProgram {
    let mut source = String::new();
    for (_, _, _, type_name) in INTEGER_TYPES {
        source.push_str(&format!(
            "fn complement_{type_name}() -> {type_name} {{ return 0{suffix}; }}\n",
            suffix = match type_name {
                "i64" => "",
                "u64" => "u",
                "u8" => "u8",
                _ => unreachable!(),
            }
        ));
        for (_, _, mnemonic) in BINARY_OPERATIONS {
            source.push_str(&format!(
                "fn {mnemonic}_{type_name}() -> {type_name} {{ return 0{suffix}; }}\n",
                suffix = match type_name {
                    "i64" => "",
                    "u64" => "u",
                    "u8" => "u8",
                    _ => unreachable!(),
                }
            ));
        }
    }
    source.push_str("fn main() -> i64 { return 0; }\n");

    let mut hir = type_check_source(&source).hir.unwrap();
    let mut function_index = 0;
    for (integer, _, ty, _) in INTEGER_TYPES {
        let expression = returned_expression_mut(
            hir.definitions
                .get_mut_for_test(FunctionId::new(function_index))
                .unwrap(),
        );
        function_index += 1;
        let span = expression.span;
        expression.kind = HirExpressionKind::Unary {
            operation: HirUnaryOperation::BitwiseComplement(integer),
            operand: Box::new(integer_expression(integer, 0x55, span)),
        };
        expression.ty = ty;

        for (operation, _, _) in BINARY_OPERATIONS {
            let expression = returned_expression_mut(
                hir.definitions
                    .get_mut_for_test(FunctionId::new(function_index))
                    .unwrap(),
            );
            function_index += 1;
            let span = expression.span;
            expression.kind = HirExpressionKind::Binary {
                operation: HirBinaryOperation::IntegerBitwise {
                    operation,
                    operand: integer,
                },
                left: Box::new(integer_expression(integer, 0xf0, span)),
                right: Box::new(integer_expression(integer, 0x5a, span)),
            };
            expression.ty = ty;
        }
    }
    hir
}

fn manually_selected_bitwise_mir() -> MirProgram {
    lower_hir(&manually_selected_bitwise_hir())
}

#[test]
fn lowers_and_verifies_the_complete_integer_bitwise_matrix_as_pure_rvalues() {
    let hir = manually_selected_bitwise_hir();
    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump, dump_hir(&hir));

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let mir_dump = dump_mir(&mir);
    assert_eq!(mir_dump, dump_mir(&mir));

    let mut function_index = 0;
    for (hir_integer, mir_integer, _, type_name) in INTEGER_TYPES {
        let definition = mir
            .definitions
            .get(FunctionId::new(function_index))
            .unwrap();
        function_index += 1;
        assert_eq!(definition.body.blocks.len(), 1);
        assert!(definition.body.path_conditions.is_empty());
        assert!(definition.body.logical_expressions.is_empty());
        assert!(definition.body.blocks[0]
            .instructions
            .iter()
            .any(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Assign(MirAssignment {
                        rvalue: MirRvalue {
                            kind: MirRvalueKind::Unary {
                                operation: MirUnaryOperation::BitwiseComplement(integer),
                                ..
                            },
                            ..
                        },
                        ..
                    }) if *integer == mir_integer
                )
            }));
        assert!(hir_dump.contains(&format!("BitwiseComplement.{type_name}")));
        assert!(mir_dump.contains(&format!("not.{type_name}")));
        assert_eq!(
            HirUnaryOperation::BitwiseComplement(hir_integer).result_type(),
            hir_integer.operand_type()
        );

        for (hir_operation, mir_operation, mnemonic) in BINARY_OPERATIONS {
            let definition = mir
                .definitions
                .get(FunctionId::new(function_index))
                .unwrap();
            function_index += 1;
            assert_eq!(definition.body.blocks.len(), 1);
            assert!(definition.body.path_conditions.is_empty());
            assert!(definition.body.logical_expressions.is_empty());
            assert!(definition.body.blocks[0].instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Assign(MirAssignment {
                        rvalue: MirRvalue {
                            kind: MirRvalueKind::Binary {
                                operation: MirBinaryOperation::IntegerBitwise { operation, operand },
                                ..
                            },
                            ..
                        },
                        ..
                    }) if *operation == mir_operation && *operand == mir_integer
                )
            }));
            assert!(hir_dump.contains(&format!(
                "Bitwise{}.{}",
                match hir_operation {
                    HirIntegerBitwiseOperation::And => "And",
                    HirIntegerBitwiseOperation::Or => "Or",
                    HirIntegerBitwiseOperation::Xor => "Xor",
                },
                type_name
            )));
            assert!(mir_dump.contains(&format!("{mnemonic}.{type_name}")));
        }
    }
}

#[test]
fn bitwise_lowering_spills_the_left_operand_before_control_affecting_right_evaluation() {
    let mut hir = type_check_source(concat!(
        "fn combine(left: u64, values: u64[]) -> u64 {\n",
        "  return left + values[0];\n",
        "}\n",
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
    *operation = HirBinaryOperation::IntegerBitwise {
        operation: HirIntegerBitwiseOperation::Or,
        operand: HirIntegerType::U64,
    };

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let function = mir.definitions.get(FunctionId::new(0)).unwrap();
    let spill = function
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::ScalarSpill)
        .expect("left operand must survive checked right-operand evaluation");
    assert_eq!(spill.ty, MirType::U64);

    let dump = dump_mir(&mir);
    let spill_store = dump.find(&format!("store {}", spill.id)).unwrap();
    let check = dump.find("array-position-check").unwrap();
    let spill_load = dump.rfind(&format!("load {}", spill.id)).unwrap();
    let operation = dump.find("or.u64").unwrap();
    assert!(spill_store < check && check < spill_load && spill_load < operation);
}

#[test]
fn verifier_rejects_corrupted_bitwise_types_and_definition_order_deterministically() {
    let mut unary = manually_selected_bitwise_mir();
    let definition = unary
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut definition.body.blocks[0].instructions[1] else {
        panic!("expected unary assignment");
    };
    let MirRvalueKind::Unary { operation, .. } = &mut assignment.rvalue.kind else {
        panic!("expected unary rvalue");
    };
    *operation = MirUnaryOperation::BitwiseComplement(MirIntegerType::U64);
    let errors = verify_mir(&unary).unwrap_err().to_string();
    assert_eq!(errors, verify_mir(&unary).unwrap_err().to_string());
    assert!(errors.contains("unary operation result type mismatch"));
    assert!(errors.contains("unary operand is not `u64`"));

    let mut binary = manually_selected_bitwise_mir();
    let definition = binary
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let operation_index = definition.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(MirAssignment {
                    rvalue: MirRvalue {
                        kind: MirRvalueKind::Binary { .. },
                        ..
                    },
                    ..
                })
            )
        })
        .unwrap();
    let MirInstruction::Assign(assignment) =
        &mut definition.body.blocks[0].instructions[operation_index]
    else {
        unreachable!();
    };
    let MirRvalueKind::Binary { operation, .. } = &mut assignment.rvalue.kind else {
        unreachable!();
    };
    *operation = MirBinaryOperation::IntegerBitwise {
        operation: MirIntegerBitwiseOperation::And,
        operand: MirIntegerType::U8,
    };
    let errors = verify_mir(&binary).unwrap_err().to_string();
    assert!(errors.contains("binary operation result type mismatch"));
    assert!(errors.contains("binary operand is not `u8`"));

    let mut noncanonical_byte = manually_selected_bitwise_mir();
    let definition = noncanonical_byte
        .definitions
        .get_mut_for_test(FunctionId::new(8))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut definition.body.blocks[0].instructions[0] else {
        panic!("expected byte constant assignment");
    };
    assignment.rvalue.kind = MirRvalueKind::ConstantU64(0x100);
    let errors = verify_mir(&noncanonical_byte).unwrap_err().to_string();
    assert!(errors.contains("u64 constant is not `u64`"));

    let mut order = manually_selected_bitwise_mir();
    let definition = order
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let operation_index = definition.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(MirAssignment {
                    rvalue: MirRvalue {
                        kind: MirRvalueKind::Binary { .. },
                        ..
                    },
                    ..
                })
            )
        })
        .unwrap();
    definition.body.blocks[0]
        .instructions
        .swap(operation_index - 1, operation_index);
    assert!(verify_mir(&order)
        .unwrap_err()
        .to_string()
        .contains("used before it is defined"));
}
