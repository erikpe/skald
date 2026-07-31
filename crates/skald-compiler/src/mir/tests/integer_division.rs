use super::*;
use crate::{
    backend::{emit_assembly, Target},
    hir::{
        dump_hir, HirCheckedIntegerDivision, HirExpression, HirExpressionKind,
        HirFunctionDefinition, HirIntegerDivisionKind as HirDivisionKind,
        HirIntegerDivisionOperation as HirDivisionOperation, HirIntegerType, HirReturnValue,
        HirStatement, Type,
    },
};

const DIVISION_OPERATIONS: [(HirDivisionOperation, MirIntegerDivisionOperation); 6] = [
    division_pair(HirIntegerType::I64, HirDivisionKind::Quotient),
    division_pair(HirIntegerType::I64, HirDivisionKind::Remainder),
    division_pair(HirIntegerType::U64, HirDivisionKind::Quotient),
    division_pair(HirIntegerType::U64, HirDivisionKind::Remainder),
    division_pair(HirIntegerType::U8, HirDivisionKind::Quotient),
    division_pair(HirIntegerType::U8, HirDivisionKind::Remainder),
];

const fn division_pair(
    hir_integer: HirIntegerType,
    kind: HirDivisionKind,
) -> (HirDivisionOperation, MirIntegerDivisionOperation) {
    (
        HirDivisionOperation {
            kind,
            operand: hir_integer,
        },
        MirIntegerDivisionOperation {
            kind: match kind {
                HirDivisionKind::Quotient => MirIntegerDivisionKind::Quotient,
                HirDivisionKind::Remainder => MirIntegerDivisionKind::Remainder,
            },
            operand: match hir_integer {
                HirIntegerType::I64 => MirIntegerType::I64,
                HirIntegerType::U64 => MirIntegerType::U64,
                HirIntegerType::U8 => MirIntegerType::U8,
            },
        },
    )
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

fn manually_selected_division_hir() -> crate::hir::HirProgram {
    let source = concat!(
        "fn div_i64() -> i64 { return 0; }\n",
        "fn rem_i64() -> i64 { return 0; }\n",
        "fn div_u64() -> u64 { return 0u; }\n",
        "fn rem_u64() -> u64 { return 0u; }\n",
        "fn div_u8() -> u8 { return 0u8; }\n",
        "fn rem_u8() -> u8 { return 0u8; }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let mut hir = type_check_source(source).hir.unwrap();
    for (index, (operation, _)) in DIVISION_OPERATIONS.into_iter().enumerate() {
        let expression = returned_expression_mut(
            hir.definitions
                .get_mut_for_test(FunctionId::new(index))
                .unwrap(),
        );
        let span = expression.span;
        expression.kind =
            HirExpressionKind::CheckedIntegerDivision(Box::new(HirCheckedIntegerDivision::new(
                operation,
                integer_expression(operation.operand, 17, span),
                integer_expression(operation.operand, 5, span),
            )));
        expression.ty = operation.result_type();
    }
    hir
}

fn model_division_rvalue(kind: MirIntegerDivisionKind, operand: MirIntegerType) -> MirProgram {
    let mut mir = lower_text("fn main() -> i64 { return 8 + 3; }\n");
    let definition = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let assignment = definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(assignment.rvalue.kind, MirRvalueKind::Binary { .. }) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .unwrap();
    let MirRvalueKind::Binary { left, right, .. } = assignment.rvalue.kind else {
        unreachable!();
    };
    assignment.rvalue.kind = MirRvalueKind::IntegerDivision {
        operation: MirIntegerDivisionOperation { kind, operand },
        dividend: left,
        divisor: right,
    };
    assignment.rvalue.ty = operand.operand_type();
    mir
}

#[test]
fn dumps_model_only_integer_division_and_remainder_operations() {
    let quotient = model_division_rvalue(MirIntegerDivisionKind::Quotient, MirIntegerType::I64);
    let remainder = model_division_rvalue(MirIntegerDivisionKind::Remainder, MirIntegerType::I64);

    let quotient_dump = dump_mir(&quotient);
    let remainder_dump = dump_mir(&remainder);
    assert_eq!(quotient_dump, dump_mir(&quotient));
    assert_eq!(remainder_dump, dump_mir(&remainder));
    assert!(quotient_dump.contains("div.i64"));
    assert!(remainder_dump.contains("rem.i64"));

    for model in [&quotient, &remainder] {
        let errors = verify_mir(model).unwrap_err();
        assert!(errors.iter().any(|error| {
            error.message
                == "integer division or remainder operation is not protected by its matching divisor check"
        }));
    }
}

#[test]
fn lowers_and_dumps_the_complete_checked_integer_division_matrix() {
    let hir = manually_selected_division_hir();
    assert_eq!(dump_hir(&hir), dump_hir(&hir));
    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let dump = dump_mir(&mir);
    assert_eq!(dump, dump_mir(&mir));

    for (index, (_, operation)) in DIVISION_OPERATIONS.into_iter().enumerate() {
        let definition = mir.definitions.get(FunctionId::new(index)).unwrap();
        let (check, success, failure) = definition
            .body
            .blocks
            .iter()
            .find_map(|block| match block.terminator {
                Some(MirTerminator::IntegerDivisorCheck {
                    check,
                    success_target,
                    failure_target,
                    ..
                }) if check.operation == operation => Some((check, success_target, failure_target)),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            definition.storage(check.dividend).unwrap().ty,
            operation.operand_type()
        );
        assert_eq!(
            definition.storage(check.divisor).unwrap().ty,
            operation.operand_type()
        );
        assert_eq!(
            definition.storage(check.result).unwrap().ty,
            operation.result_type()
        );
        assert!(definition.block(success).unwrap().instructions.iter().any(
            |instruction| matches!(instruction, MirInstruction::Assign(assignment)
                if matches!(assignment.rvalue.kind, MirRvalueKind::IntegerDivision {
                    operation: actual, ..
                } if actual == operation))
        ));
        assert!(matches!(
            definition.block(failure).unwrap().terminator,
            Some(MirTerminator::Terminate { reason, .. })
                if reason == operation.failure_reason()
        ));
        assert!(dump.contains(&format!(
            "integer-divisor-check {}.{}",
            operation.mnemonic(),
            operation.operand.name()
        )));
        assert!(dump.contains(&format!(
            "terminate {}",
            operation.failure_reason().mnemonic()
        )));
    }

    let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    assert!(assembly.contains("idiv rcx"));
    assert!(assembly.contains("div rcx"));
    assert!(assembly.contains("integer division by zero"));
    assert!(assembly.contains("integer remainder by zero"));
}

#[test]
fn lowering_secures_operands_in_source_order_after_control_affecting_evaluation() {
    let mut hir = type_check_source(concat!(
        "fn produce() -> u64 { return 129u; }\n",
        "fn combine(values: u64[]) -> u64 { return produce() + values[0]; }\n",
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
    expression.kind =
        HirExpressionKind::CheckedIntegerDivision(Box::new(HirCheckedIntegerDivision::new(
            HirDivisionOperation {
                kind: HirDivisionKind::Quotient,
                operand: HirIntegerType::U64,
            },
            (**left).clone(),
            (**right).clone(),
        )));

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let definition = mir.definitions.get(FunctionId::new(1)).unwrap();
    let (check_block, check) = definition
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::IntegerDivisorCheck { check, .. }) => Some((block.id, check)),
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
                    matches!(instruction, MirInstruction::Store(store)
                        if store.destination == MirPlace::base(storage))
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
    assert!(store_block(check.dividend).index() <= array_check.index());
    assert!(array_check.index() < store_block(check.divisor).index());
    assert_eq!(store_block(check.divisor), check_block);
    assert_eq!(
        definition
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(
                |instruction| matches!(instruction, MirInstruction::Call(call)
                if call.target == MirCallTarget::Direct(FunctionId::new(0)))
            )
            .count(),
        1
    );
}

#[test]
fn nested_checked_operations_finish_before_their_enclosing_divisor_check() {
    let mut hir = manually_selected_division_hir();
    let expression = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(2))
            .unwrap(),
    );
    let HirExpressionKind::CheckedIntegerDivision(outer) = &mut expression.kind else {
        unreachable!();
    };
    let span = outer.divisor.span;
    *outer.divisor = HirExpression {
        kind: HirExpressionKind::CheckedIntegerDivision(Box::new(HirCheckedIntegerDivision::new(
            HirDivisionOperation {
                kind: HirDivisionKind::Remainder,
                operand: HirIntegerType::U64,
            },
            integer_expression(HirIntegerType::U64, 9, span),
            integer_expression(HirIntegerType::U64, 4, span),
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
        .filter_map(|block| match block.terminator {
            Some(MirTerminator::IntegerDivisorCheck { check, .. }) => Some(check.operation.kind),
            _ => None,
        })
        .collect();
    assert_eq!(
        checks,
        [
            MirIntegerDivisionKind::Remainder,
            MirIntegerDivisionKind::Quotient
        ]
    );

    let mut with_shift = type_check_source(concat!(
        "fn calculate() -> u64 { return (8u >> 1u) + 2u; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();
    let expression = returned_expression_mut(
        with_shift
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap(),
    );
    let HirExpressionKind::Binary { left, right, .. } = &expression.kind else {
        panic!("expected binary expression");
    };
    expression.kind =
        HirExpressionKind::CheckedIntegerDivision(Box::new(HirCheckedIntegerDivision::new(
            HirDivisionOperation {
                kind: HirDivisionKind::Quotient,
                operand: HirIntegerType::U64,
            },
            (**left).clone(),
            (**right).clone(),
        )));
    let mir = lower_hir(&with_shift);
    verify_mir(&mir).unwrap();
    let definition = mir.definitions.get(FunctionId::new(0)).unwrap();
    let shift = definition
        .body
        .blocks
        .iter()
        .position(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::ShiftCountCheck { .. })
            )
        })
        .unwrap();
    let division = definition
        .body
        .blocks
        .iter()
        .position(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::IntegerDivisorCheck { .. })
            )
        })
        .unwrap();
    assert!(shift < division);
}

#[test]
fn checked_integer_division_composes_with_eager_and_short_circuit_consumers() {
    let mut eager = type_check_source(concat!(
        "fn calculate() -> u64 { return 1u + 8u; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();
    let expression = returned_expression_mut(
        eager
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap(),
    );
    let HirExpressionKind::Binary { right, .. } = &mut expression.kind else {
        panic!("expected eager binary expression");
    };
    let span = right.span;
    **right = HirExpression {
        kind: HirExpressionKind::CheckedIntegerDivision(Box::new(HirCheckedIntegerDivision::new(
            HirDivisionOperation {
                kind: HirDivisionKind::Quotient,
                operand: HirIntegerType::U64,
            },
            integer_expression(HirIntegerType::U64, 8, span),
            integer_expression(HirIntegerType::U64, 2, span),
        ))),
        ty: Type::U64,
        span,
    };
    let eager_mir = lower_hir(&eager);
    verify_mir(&eager_mir).unwrap();
    let eager_dump = dump_mir(&eager_mir);
    assert!(eager_dump.contains("integer-divisor-check div.u64"));
    assert!(eager_dump.contains("add.u64"));

    let mut logical = type_check_source(concat!(
        "fn decide() -> bool { return 8u + 2u == 4u && true; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();
    let expression = returned_expression_mut(
        logical
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap(),
    );
    let HirExpressionKind::Logical(logical_expression) = &mut expression.kind else {
        panic!("expected logical expression");
    };
    let HirExpressionKind::PrimitiveComparison { left, .. } = &mut logical_expression.left.kind
    else {
        panic!("expected comparison left operand");
    };
    let HirExpressionKind::Binary {
        left: dividend,
        right: divisor,
        ..
    } = &left.kind
    else {
        panic!("expected arithmetic comparison operand");
    };
    let operation = HirDivisionOperation {
        kind: HirDivisionKind::Quotient,
        operand: HirIntegerType::U64,
    };
    left.kind = HirExpressionKind::CheckedIntegerDivision(Box::new(
        HirCheckedIntegerDivision::new(operation, (**dividend).clone(), (**divisor).clone()),
    ));
    left.ty = operation.result_type();

    let logical_mir = lower_hir(&logical);
    verify_mir(&logical_mir).unwrap();
    let logical_dump = dump_mir(&logical_mir);
    assert!(logical_dump.contains("integer-divisor-check div.u64"));
    assert!(logical_dump.contains("\n        and condition"));
}

#[test]
fn allocation_backed_operands_cleanup_only_after_the_successful_result_join() {
    let mut hir = type_check_source(concat!(
        "class Trace {\n",
        "  value: u64;\n",
        "  init(value: u64) { self.value = value; }\n",
        "  fn read() -> u64 { return self.value; }\n",
        "  destroy {}\n",
        "}\n",
        "fn make(value: u64) -> shared Trace { return new Trace(value); }\n",
        "fn calculate() -> u64 { return make(8u)->read() + make(2u)->read(); }\n",
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
        panic!("expected binary expression");
    };
    expression.kind =
        HirExpressionKind::CheckedIntegerDivision(Box::new(HirCheckedIntegerDivision::new(
            HirDivisionOperation {
                kind: HirDivisionKind::Quotient,
                operand: HirIntegerType::U64,
            },
            (**left).clone(),
            (**right).clone(),
        )));

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let dump = dump_mir(&mir);
    assert_eq!(dump, dump_mir(&mir));
    assert!(dump.contains("integer-divisor-check div.u64"));
    assert!(dump.contains("terminate integer-division-by-zero"));
    assert!(dump.matches("shared-release").count() >= 2, "{dump}");

    let definition = mir.definitions.get(FunctionId::new(1)).unwrap();
    let (_, failure) = definition
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::IntegerDivisorCheck {
                success_target,
                failure_target,
                ..
            }) => Some((success_target, failure_target)),
            _ => None,
        })
        .unwrap();
    assert!(definition.block(failure).unwrap().instructions.is_empty());
}

fn verifier_errors(program: &MirProgram) -> String {
    let errors = verify_mir(program).unwrap_err().to_string();
    assert_eq!(errors, verify_mir(program).unwrap_err().to_string());
    errors
}

fn division_block_indices(
    program: &MirProgram,
    function: FunctionId,
) -> (usize, usize, usize, usize) {
    let definition = program.definitions.get(function).unwrap();
    let (check_index, success, failure) = definition
        .body
        .blocks
        .iter()
        .enumerate()
        .find_map(|(index, block)| match block.terminator {
            Some(MirTerminator::IntegerDivisorCheck {
                success_target,
                failure_target,
                ..
            }) => Some((index, success_target.index(), failure_target.index())),
            _ => None,
        })
        .unwrap();
    let join = match definition.body.blocks[success].terminator {
        Some(MirTerminator::Goto { target, .. }) => target.index(),
        _ => panic!("expected success join"),
    };
    (check_index, success, failure, join)
}

#[test]
fn verifier_rejects_broken_integer_division_relationships_deterministically() {
    let valid = lower_hir(&manually_selected_division_hir());
    verify_mir(&valid).unwrap();
    let function = FunctionId::new(2);

    let mut wrong_carrier = valid.clone();
    let (check_index, _, _, _) = division_block_indices(&wrong_carrier, function);
    let definition = wrong_carrier
        .definitions
        .get_mut_for_test(function)
        .unwrap();
    let Some(MirTerminator::IntegerDivisorCheck { check, .. }) =
        definition.body.blocks[check_index].terminator
    else {
        unreachable!();
    };
    definition.storage[check.divisor.index()].ty = MirType::I64;
    assert!(verifier_errors(&wrong_carrier)
        .contains("integer division divisor carrier must be an exact `u64` scalar spill"));

    let mut missing_write = valid.clone();
    let (check_index, _, _, _) = division_block_indices(&missing_write, function);
    let definition = missing_write
        .definitions
        .get_mut_for_test(function)
        .unwrap();
    let Some(MirTerminator::IntegerDivisorCheck { check, .. }) =
        definition.body.blocks[check_index].terminator
    else {
        unreachable!();
    };
    for block in &mut definition.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(instruction, MirInstruction::Store(store)
                if store.destination == MirPlace::base(check.dividend))
        });
    }
    assert!(verifier_errors(&missing_write)
        .contains("integer division dividend carrier must have one write dominating"));

    let mut wrong_failure = valid.clone();
    let (_, _, failure, _) = division_block_indices(&wrong_failure, function);
    let definition = wrong_failure
        .definitions
        .get_mut_for_test(function)
        .unwrap();
    definition.body.blocks[failure].terminator = Some(MirTerminator::Terminate {
        reason: MirTerminationReason::IntegerRemainderByZero,
        span: definition.span,
    });
    assert!(verifier_errors(&wrong_failure).contains(
        "integer divisor failure edge must directly terminate with `integer-division-by-zero`"
    ));

    let mut unchecked = valid.clone();
    let (check_index, success, _, _) = division_block_indices(&unchecked, function);
    let definition = unchecked.definitions.get_mut_for_test(function).unwrap();
    definition.body.blocks[check_index].terminator = Some(MirTerminator::Goto {
        target: BlockId::new(function, success),
        span: definition.span,
    });
    assert!(verifier_errors(&unchecked).contains(
        "integer division or remainder operation is not protected by its matching divisor check"
    ));

    let mut mismatched_operation = valid.clone();
    let (_, success, _, _) = division_block_indices(&mismatched_operation, function);
    let definition = mismatched_operation
        .definitions
        .get_mut_for_test(function)
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut definition.body.blocks[success].instructions[2]
    else {
        unreachable!();
    };
    let MirRvalueKind::IntegerDivision { operation, .. } = &mut assignment.rvalue.kind else {
        unreachable!();
    };
    operation.kind = MirIntegerDivisionKind::Remainder;
    let errors = verifier_errors(&mismatched_operation);
    assert!(errors.contains("integer divisor success edge must load the secured operands"));
    assert!(errors.contains("operation is not protected by its matching divisor check"));

    let mut swapped_edges = valid.clone();
    let (check_index, _, _, _) = division_block_indices(&swapped_edges, function);
    let definition = swapped_edges
        .definitions
        .get_mut_for_test(function)
        .unwrap();
    let Some(MirTerminator::IntegerDivisorCheck {
        success_target,
        failure_target,
        ..
    }) = &mut definition.body.blocks[check_index].terminator
    else {
        unreachable!();
    };
    std::mem::swap(success_target, failure_target);
    let errors = verifier_errors(&swapped_edges);
    assert!(errors.contains("integer divisor failure edge must directly terminate"));
    assert!(errors.contains("integer divisor success edge must load the secured operands"));

    let mut reachable_failure = valid.clone();
    let (_, _, failure, join) = division_block_indices(&reachable_failure, function);
    let definition = reachable_failure
        .definitions
        .get_mut_for_test(function)
        .unwrap();
    definition.body.blocks[failure].terminator = Some(MirTerminator::Goto {
        target: BlockId::new(function, join),
        span: definition.span,
    });
    let errors = verifier_errors(&reachable_failure);
    assert!(errors.contains("integer divisor failure edge must directly terminate"));
    assert!(errors.contains("result join must be reached only from its success block"));

    let mut use_before_definition = valid.clone();
    let (_, success, _, _) = division_block_indices(&use_before_definition, function);
    let definition = use_before_definition
        .definitions
        .get_mut_for_test(function)
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut definition.body.blocks[success].instructions[2]
    else {
        unreachable!();
    };
    let MirRvalueKind::IntegerDivision { dividend, .. } = &mut assignment.rvalue.kind else {
        unreachable!();
    };
    *dividend = ValueId::new(function, 0);
    assert!(verifier_errors(&use_before_definition).contains("is used before it is defined"));
}

#[test]
fn dumps_distinct_zero_divisor_termination_reasons() {
    for (reason, mnemonic) in [
        (
            MirTerminationReason::IntegerDivisionByZero,
            "integer-division-by-zero",
        ),
        (
            MirTerminationReason::IntegerRemainderByZero,
            "integer-remainder-by-zero",
        ),
    ] {
        let mut mir = lower_text("fn main() -> i64 { return 0; }\n");
        let definition = mir
            .definitions
            .get_mut_for_test(mir.entry_function)
            .unwrap();
        definition.body.blocks[0].terminator = Some(MirTerminator::Terminate {
            reason,
            span: definition.span,
        });
        let dump = dump_mir(&mir);
        assert_eq!(dump, dump_mir(&mir));
        assert!(dump.contains(&format!("terminate {mnemonic}")));
    }
}

fn floor_div_rem(dividend: i64, divisor: i64) -> (i64, i64) {
    if dividend == i64::MIN && divisor == -1 {
        return (i64::MIN, 0);
    }
    let mut quotient = dividend / divisor;
    let mut remainder = dividend % divisor;
    if remainder != 0 && (remainder < 0) != (divisor < 0) {
        quotient -= 1;
        remainder += divisor;
    }
    (quotient, remainder)
}

#[test]
fn signed_contract_records_sign_matrix_and_minimum_pair_results() {
    for (dividend, divisor, quotient, remainder) in [
        (7, 3, 2, 1),
        (-7, 3, -3, 2),
        (7, -3, -3, -2),
        (-7, -3, 2, -1),
        (i64::MIN, -1, i64::MIN, 0),
    ] {
        assert_eq!(floor_div_rem(dividend, divisor), (quotient, remainder));
    }

    let quotient = MirIntegerDivisionOperation {
        kind: MirIntegerDivisionKind::Quotient,
        operand: MirIntegerType::I64,
    };
    let remainder = MirIntegerDivisionOperation {
        kind: MirIntegerDivisionKind::Remainder,
        operand: MirIntegerType::I64,
    };
    assert_eq!(
        quotient.signed_semantics().unwrap().minimum_pair_result,
        MirSignedMinimumPairResult::Minimum
    );
    assert_eq!(
        remainder.signed_semantics().unwrap().minimum_pair_result,
        MirSignedMinimumPairResult::Zero
    );
}
