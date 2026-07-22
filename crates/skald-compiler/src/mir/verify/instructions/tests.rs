use crate::{
    identity::FunctionId,
    mir::{verify_mir, MirBinaryOperation, MirInstruction, MirRvalueKind},
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
