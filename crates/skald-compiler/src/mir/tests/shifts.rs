use super::*;
use crate::hir::{
    dump_hir, HirCheckedShift, HirExpression, HirExpressionKind, HirFunctionDefinition,
    HirIntegerType, HirReturnValue, HirShiftDirection, HirShiftOperation, HirStatement, Type,
};

const SHIFT_OPERATIONS: [(HirShiftOperation, MirShiftOperation, &str); 6] = [
    shift_pair(
        HirIntegerType::I64,
        MirIntegerType::I64,
        HirShiftDirection::Left,
    ),
    shift_pair(
        HirIntegerType::I64,
        MirIntegerType::I64,
        HirShiftDirection::Right,
    ),
    shift_pair(
        HirIntegerType::U64,
        MirIntegerType::U64,
        HirShiftDirection::Left,
    ),
    shift_pair(
        HirIntegerType::U64,
        MirIntegerType::U64,
        HirShiftDirection::Right,
    ),
    shift_pair(
        HirIntegerType::U8,
        MirIntegerType::U8,
        HirShiftDirection::Left,
    ),
    shift_pair(
        HirIntegerType::U8,
        MirIntegerType::U8,
        HirShiftDirection::Right,
    ),
];

const fn shift_pair(
    hir_integer: HirIntegerType,
    mir_integer: MirIntegerType,
    direction: HirShiftDirection,
) -> (HirShiftOperation, MirShiftOperation, &'static str) {
    let hir = HirShiftOperation {
        direction,
        left: hir_integer,
    };
    let mir = MirShiftOperation {
        direction: match direction {
            HirShiftDirection::Left => MirShiftDirection::Left,
            HirShiftDirection::Right => MirShiftDirection::Right,
        },
        left: mir_integer,
    };
    (hir, mir, hir.mnemonic())
}

fn returned_expression_mut(definition: &mut HirFunctionDefinition) -> &mut HirExpression {
    let HirStatement::Return(statement) = definition.body.statements.last_mut().unwrap() else {
        panic!("expected final return statement");
    };
    let HirReturnValue::Scalar(expression) = statement.value.as_mut().unwrap() else {
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

fn manually_selected_shift_hir() -> crate::hir::HirProgram {
    let source = concat!(
        "fn shl_i64() -> i64 { return 0; }\n",
        "fn sar_i64() -> i64 { return 0; }\n",
        "fn shl_u64() -> u64 { return 0u; }\n",
        "fn shr_u64() -> u64 { return 0u; }\n",
        "fn shl_u8() -> u8 { return 0u8; }\n",
        "fn shr_u8() -> u8 { return 0u8; }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let mut hir = type_check_source(source).hir.unwrap();
    for (index, (operation, _, _)) in SHIFT_OPERATIONS.into_iter().enumerate() {
        let expression = returned_expression_mut(
            hir.definitions
                .get_mut_for_test(FunctionId::new(index))
                .unwrap(),
        );
        let span = expression.span;
        expression.kind = HirExpressionKind::CheckedShift(Box::new(HirCheckedShift::new(
            operation,
            integer_expression(operation.left, 0x81, span),
            HirExpression {
                kind: HirExpressionKind::U64(1),
                ty: Type::U64,
                span,
            },
        )));
        expression.ty = operation.result_type();
    }
    hir
}

#[test]
fn lowers_and_dumps_the_complete_checked_shift_matrix() {
    let hir = manually_selected_shift_hir();
    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump, dump_hir(&hir));
    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let mir_dump = dump_mir(&mir);
    assert_eq!(mir_dump, dump_mir(&mir));

    for (index, (hir_operation, mir_operation, mnemonic)) in
        SHIFT_OPERATIONS.into_iter().enumerate()
    {
        let type_name = match hir_operation.left {
            HirIntegerType::I64 => "i64",
            HirIntegerType::U64 => "u64",
            HirIntegerType::U8 => "u8",
        };
        assert_eq!(hir_operation.count_type(), Type::U64);
        assert_eq!(hir_operation.result_type(), hir_operation.left_type());
        assert_eq!(hir_operation.width(), mir_operation.width());
        assert!(hir_dump.contains(&format!(
            "CheckedShift {mnemonic}.{} count=u64 width={} failure=shift-count-out-of-range",
            type_name,
            hir_operation.width()
        )));

        let definition = mir.definitions.get(FunctionId::new(index)).unwrap();
        assert!(definition.body.blocks.iter().any(|block| matches!(
            block.terminator,
            Some(MirTerminator::ShiftCountCheck { check, .. }) if check.operation == mir_operation
        )));
        assert!(mir_dump.contains(&format!(
            "shift-count-check {mnemonic}.{}",
            mir_operation.left_type()
        )));
        assert!(mir_dump.contains(&format!("{mnemonic}.{}", mir_operation.left_type())));
    }
}

#[test]
fn lowering_secures_each_operand_after_its_control_affecting_evaluation() {
    let mut hir = type_check_source(concat!(
        "fn produce() -> u64 { return 129u; }\n",
        "fn combine(values: u64[]) -> u64 {\n",
        "  return produce() + values[0];\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();
    let expression = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(1))
            .unwrap(),
    );
    let HirExpressionKind::Binary { left, right, .. } = &expression.kind else {
        panic!("expected source binary expression");
    };
    expression.kind = HirExpressionKind::CheckedShift(Box::new(HirCheckedShift::new(
        HirShiftOperation {
            direction: HirShiftDirection::Right,
            left: HirIntegerType::U64,
        },
        (**left).clone(),
        (**right).clone(),
    )));

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let definition = mir.definitions.get(FunctionId::new(1)).unwrap();
    let (check_block, check, success_target) = definition
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::ShiftCountCheck {
                check,
                success_target,
                ..
            }) => Some((block.id, check, success_target)),
            _ => None,
        })
        .unwrap();
    let store_block = |storage| {
        definition
            .body
            .blocks
            .iter()
            .find(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(
                        instruction,
                        MirInstruction::Store(store)
                            if store.destination == MirPlace::base(storage)
                    )
                })
            })
            .unwrap()
            .id
    };
    let array_check = definition
        .body
        .blocks
        .iter()
        .find(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::ArrayPositionCheck { .. })
            )
        })
        .unwrap()
        .id;
    assert!(store_block(check.left).index() <= array_check.index());
    assert!(array_check.index() < store_block(check.count).index());
    assert_eq!(store_block(check.count), check_block);
    assert!(definition
        .block(success_target)
        .unwrap()
        .instructions
        .iter()
        .any(
            |instruction| matches!(instruction, MirInstruction::Assign(assignment)
            if matches!(assignment.rvalue.kind, MirRvalueKind::Shift { .. }))
        ));
}

#[test]
fn nested_checked_shift_count_is_lowered_before_the_outer_check() {
    let mut hir = manually_selected_shift_hir();
    let expression = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(2))
            .unwrap(),
    );
    let HirExpressionKind::CheckedShift(outer) = &mut expression.kind else {
        unreachable!();
    };
    let span = outer.count.span;
    *outer.count = HirExpression {
        kind: HirExpressionKind::CheckedShift(Box::new(HirCheckedShift::new(
            HirShiftOperation {
                direction: HirShiftDirection::Right,
                left: HirIntegerType::U64,
            },
            integer_expression(HirIntegerType::U64, 1, span),
            HirExpression {
                kind: HirExpressionKind::U64(0),
                ty: Type::U64,
                span,
            },
        ))),
        ty: Type::U64,
        span,
    };

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let definition = mir.definitions.get(FunctionId::new(2)).unwrap();
    let checks: Vec<_> = definition
        .body
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Some(MirTerminator::ShiftCountCheck { check, .. }) => Some(check.operation),
            _ => None,
        })
        .collect();
    assert_eq!(checks.len(), 2);
    assert_eq!(checks[0].direction, MirShiftDirection::Right);
    assert_eq!(checks[1].direction, MirShiftDirection::Left);
}

#[test]
fn verifier_rejects_every_broken_checked_shift_relationship_deterministically() {
    let operation = MirShiftOperation {
        direction: MirShiftDirection::Left,
        left: MirIntegerType::U64,
    };

    let mut wrong_carrier = fixture_checked_shift_program(operation, 1, 1, 2);
    wrong_carrier
        .definitions
        .get_mut_for_test(wrong_carrier.entry_function)
        .unwrap()
        .storage[1]
        .ty = MirType::I64;
    let errors = verify_mir(&wrong_carrier).unwrap_err().to_string();
    assert_eq!(errors, verify_mir(&wrong_carrier).unwrap_err().to_string());
    assert!(errors.contains("shift count carrier must be an exact `u64` scalar spill"));

    let mut wrong_failure = fixture_checked_shift_program(operation, 1, 1, 2);
    let definition = wrong_failure
        .definitions
        .get_mut_for_test(wrong_failure.entry_function)
        .unwrap();
    let span = definition.span;
    definition.body.blocks[2].terminator = Some(MirTerminator::Terminate {
        reason: MirTerminationReason::OptionalAccessFailure,
        span,
    });
    assert!(verify_mir(&wrong_failure)
        .unwrap_err()
        .to_string()
        .contains("shift failure edge must directly terminate"));

    let mut unchecked = fixture_checked_shift_program(operation, 1, 1, 2);
    let entry = unchecked.entry_function;
    let definition = unchecked.definitions.get_mut_for_test(entry).unwrap();
    let span = definition.span;
    definition.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: BlockId::new(entry, 1),
        span,
    });
    assert!(verify_mir(&unchecked)
        .unwrap_err()
        .to_string()
        .contains("shift operation is not protected by its matching count check"));

    let mut mismatched = fixture_checked_shift_program(operation, 1, 1, 2);
    let definition = mismatched
        .definitions
        .get_mut_for_test(mismatched.entry_function)
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut definition.body.blocks[1].instructions[2] else {
        unreachable!();
    };
    let MirRvalueKind::Shift { operation, .. } = &mut assignment.rvalue.kind else {
        unreachable!();
    };
    operation.direction = MirShiftDirection::Right;
    let errors = verify_mir(&mismatched).unwrap_err().to_string();
    assert!(errors.contains("shift success edge must load the secured operands"));
    assert!(errors.contains("shift operation is not protected by its matching count check"));
}

#[test]
fn source_shift_tokens_remain_disabled_until_bw3() {
    for source in [
        "fn main() -> i64 { return 1 << 1u; }",
        "fn main() -> i64 { return 1 >> 1u; }",
    ] {
        let (_, output) = crate::test_support::parse_source(source);
        assert!(output.has_errors());
    }
}
