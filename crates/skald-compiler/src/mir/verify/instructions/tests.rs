use crate::{
    identity::FunctionId,
    mir::{verify_mir, MirBinaryOperation, MirInstruction, MirIntegerType, MirRvalueKind, MirType},
    test_support::lower_source_to_mir,
};

#[test]
fn arithmetic_corruption_accumulates_errors_in_deterministic_order() {
    let mut program =
        lower_source_to_mir("fn add() -> u64 { return 1u + 2u; } fn main() -> i64 { return 0; }");
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[2] else {
        panic!("expected binary assignment");
    };
    let MirRvalueKind::Binary { operation, .. } = &mut assignment.rvalue.kind else {
        panic!("expected binary rvalue");
    };
    *operation = MirBinaryOperation::AddI64;

    assert_eq!(
        verify_mir(&program)
            .unwrap_err()
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>(),
        [
            "binary operation result type mismatch",
            "arithmetic operand is not `i64`",
            "arithmetic operand is not `i64`",
        ]
    );
}

#[test]
fn integer_cast_corruption_accumulates_errors_in_deterministic_order() {
    let mut program =
        lower_source_to_mir("fn cast() -> u8 { return (u8) 1u; } fn main() -> i64 { return 0; }");
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let assignment = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(assignment.rvalue.kind, MirRvalueKind::IntegerCast { .. }) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .expect("expected integer cast assignment");
    let MirRvalueKind::IntegerCast { operation, .. } = &mut assignment.rvalue.kind else {
        unreachable!()
    };
    operation.source = MirIntegerType::I64;
    assignment.rvalue.ty = MirType::U64;

    assert_eq!(
        verify_mir(&program)
            .unwrap_err()
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>(),
        [
            "assignment type does not match value f0:v1",
            "integer cast result type mismatch",
            "integer cast source is not `i64`",
        ]
    );
}

#[test]
fn comparison_corruption_accumulates_errors_in_deterministic_order() {
    let mut program =
        lower_source_to_mir("fn less() -> bool { return 1u < 2u; } fn main() -> i64 { return 0; }");
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[2] else {
        panic!("expected comparison assignment");
    };
    let MirRvalueKind::IntegerComparison { operation, .. } = &mut assignment.rvalue.kind else {
        panic!("expected integer comparison rvalue");
    };
    operation.operand = MirIntegerType::I64;
    assignment.rvalue.ty = MirType::U64;

    assert_eq!(
        verify_mir(&program)
            .unwrap_err()
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>(),
        [
            "assignment type does not match value f0:v2",
            "integer comparison result must be `bool`",
            "comparison operand is not `i64`",
            "comparison operand is not `i64`",
        ]
    );
}
