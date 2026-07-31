use super::*;

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
            error.message == "integer division or remainder requires a verified divisor check"
        }));
    }
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
