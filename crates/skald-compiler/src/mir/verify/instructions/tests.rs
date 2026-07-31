use crate::{
    identity::FunctionId,
    mir::{
        test_fixtures::{assign as fixture_assign, value as fixture_value},
        verify_mir, MirBinaryOperation, MirComparisonOperand, MirComparisonPredicate,
        MirInstruction, MirIntegerType, MirPrimitiveComparison, MirProgram, MirRvalueKind,
        MirTerminator, MirType, MirUnaryOperation, ValueId,
    },
    test_support::lower_source_to_mir,
};

fn eager_boolean_mir() -> MirProgram {
    let mut program = lower_source_to_mir(concat!(
        "fn invert() -> bool { return true; }\n",
        "fn compare() -> bool { return true; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let invert = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let span = invert.span;
    let source = ValueId::new(invert.function, 0);
    let result = ValueId::new(invert.function, 1);
    invert.values = vec![
        fixture_value(source, MirType::Bool, span),
        fixture_value(result, MirType::Bool, span),
    ];
    invert.body.blocks[0].instructions = vec![
        fixture_assign(
            source,
            MirRvalueKind::ConstantBool(false),
            MirType::Bool,
            span,
        ),
        fixture_assign(
            result,
            MirRvalueKind::Unary {
                operation: MirUnaryOperation::LogicalNotBool,
                operand: source,
            },
            MirType::Bool,
            span,
        ),
    ];
    invert.body.blocks[0].terminator = Some(MirTerminator::Return {
        value: Some(result),
        span,
    });

    let compare = program
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let span = compare.span;
    let left = ValueId::new(compare.function, 0);
    let right = ValueId::new(compare.function, 1);
    let result = ValueId::new(compare.function, 2);
    compare.values = vec![
        fixture_value(left, MirType::Bool, span),
        fixture_value(right, MirType::Bool, span),
        fixture_value(result, MirType::Bool, span),
    ];
    compare.body.blocks[0].instructions = vec![
        fixture_assign(left, MirRvalueKind::ConstantBool(true), MirType::Bool, span),
        fixture_assign(
            right,
            MirRvalueKind::ConstantBool(false),
            MirType::Bool,
            span,
        ),
        fixture_assign(
            result,
            MirRvalueKind::PrimitiveComparison {
                operation: MirPrimitiveComparison {
                    predicate: MirComparisonPredicate::NotEqual,
                    operand: MirComparisonOperand::Bool,
                },
                left,
                right,
            },
            MirType::Bool,
            span,
        ),
    ];
    compare.body.blocks[0].terminator = Some(MirTerminator::Return {
        value: Some(result),
        span,
    });

    program
}

fn floating_comparison_mir(predicate: MirComparisonPredicate) -> MirProgram {
    let mut program = lower_source_to_mir(
        "fn compare() -> bool { return 1 < 2; } fn main() -> i64 { return 0; }",
    );
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    function.values[0].ty = MirType::F64;
    function.values[1].ty = MirType::F64;

    let MirInstruction::Assign(left) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected left assignment");
    };
    left.rvalue.kind = MirRvalueKind::ConstantF64Bits(1.0_f64.to_bits());
    left.rvalue.ty = MirType::F64;
    let MirInstruction::Assign(right) = &mut function.body.blocks[0].instructions[1] else {
        panic!("expected right assignment");
    };
    right.rvalue.kind = MirRvalueKind::ConstantF64Bits(2.0_f64.to_bits());
    right.rvalue.ty = MirType::F64;
    let MirInstruction::Assign(comparison) = &mut function.body.blocks[0].instructions[2] else {
        panic!("expected comparison assignment");
    };
    let MirRvalueKind::PrimitiveComparison { operation, .. } = &mut comparison.rvalue.kind else {
        panic!("expected comparison rvalue");
    };
    *operation = MirPrimitiveComparison {
        predicate,
        operand: MirComparisonOperand::F64,
    };
    program
}

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
            "binary operand is not `i64`",
            "binary operand is not `i64`",
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
    let MirRvalueKind::PrimitiveComparison { operation, .. } = &mut assignment.rvalue.kind else {
        panic!("expected integer comparison rvalue");
    };
    operation.operand = MirComparisonOperand::Integer(MirIntegerType::I64);
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

#[test]
fn verifier_accepts_every_exact_floating_comparison() {
    for predicate in [
        MirComparisonPredicate::Equal,
        MirComparisonPredicate::NotEqual,
        MirComparisonPredicate::LessThan,
        MirComparisonPredicate::LessEqual,
        MirComparisonPredicate::GreaterThan,
        MirComparisonPredicate::GreaterEqual,
    ] {
        verify_mir(&floating_comparison_mir(predicate)).unwrap();
    }
}

#[test]
fn floating_comparison_corruption_accumulates_errors_deterministically() {
    let mut program = floating_comparison_mir(MirComparisonPredicate::LessThan);
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    function.values[0].ty = MirType::I64;
    let MirInstruction::Assign(left) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected left assignment");
    };
    left.rvalue.kind = MirRvalueKind::ConstantI64(1);
    left.rvalue.ty = MirType::I64;
    let MirInstruction::Assign(comparison) = &mut function.body.blocks[0].instructions[2] else {
        panic!("expected comparison assignment");
    };
    comparison.rvalue.ty = MirType::U64;

    assert_eq!(
        verify_mir(&program)
            .unwrap_err()
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>(),
        [
            "assignment type does not match value f0:v2",
            "floating comparison result must be `bool`",
            "comparison operand is not `f64`",
        ]
    );
}

#[test]
fn verifier_rejects_floating_comparison_use_before_definition() {
    let mut program = floating_comparison_mir(MirComparisonPredicate::Equal);
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    function.body.blocks[0].instructions.swap(0, 2);

    assert!(verify_mir(&program)
        .unwrap_err()
        .iter()
        .any(|error| error.message.contains("used before it is defined")));
}

#[test]
fn verifier_accepts_exact_eager_boolean_operations() {
    verify_mir(&eager_boolean_mir()).unwrap();
}

#[test]
fn verifier_rejects_boolean_ordering() {
    let mut program = eager_boolean_mir();
    let compare = program
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut compare.body.blocks[0].instructions[2] else {
        panic!("expected comparison assignment");
    };
    let MirRvalueKind::PrimitiveComparison { operation, .. } = &mut assignment.rvalue.kind else {
        panic!("expected primitive comparison rvalue");
    };
    operation.predicate = MirComparisonPredicate::LessThan;

    assert_eq!(
        verify_mir(&program)
            .unwrap_err()
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>(),
        ["comparison predicate `lt` is not valid for `bool`"]
    );
}

#[test]
fn eager_boolean_corruption_accumulates_errors_in_deterministic_order() {
    let mut program = eager_boolean_mir();
    let invert = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    invert.values[0].ty = MirType::I64;
    let MirInstruction::Assign(source) = &mut invert.body.blocks[0].instructions[0] else {
        panic!("expected source assignment");
    };
    source.rvalue.kind = MirRvalueKind::ConstantI64(0);
    source.rvalue.ty = MirType::I64;
    let MirInstruction::Assign(operation) = &mut invert.body.blocks[0].instructions[1] else {
        panic!("expected unary assignment");
    };
    operation.rvalue.ty = MirType::U64;

    assert_eq!(
        verify_mir(&program)
            .unwrap_err()
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>(),
        [
            "assignment type does not match value f0:v1",
            "unary operation result type mismatch",
            "unary operand is not `bool`",
        ]
    );
}

#[test]
fn eager_boolean_comparison_corruption_accumulates_errors_in_deterministic_order() {
    let mut program = eager_boolean_mir();
    let compare = program
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    compare.values[0].ty = MirType::I64;
    let MirInstruction::Assign(left) = &mut compare.body.blocks[0].instructions[0] else {
        panic!("expected left assignment");
    };
    left.rvalue.kind = MirRvalueKind::ConstantI64(1);
    left.rvalue.ty = MirType::I64;
    let MirInstruction::Assign(operation) = &mut compare.body.blocks[0].instructions[2] else {
        panic!("expected comparison assignment");
    };
    operation.rvalue.ty = MirType::U64;

    assert_eq!(
        verify_mir(&program)
            .unwrap_err()
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>(),
        [
            "assignment type does not match value f1:v2",
            "boolean comparison result must be `bool`",
            "comparison operand is not `bool`",
        ]
    );
}

#[test]
fn verifier_rejects_eager_boolean_use_before_definition() {
    let mut program = eager_boolean_mir();
    let invert = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    invert.body.blocks[0].instructions.swap(0, 1);

    assert!(verify_mir(&program)
        .unwrap_err()
        .iter()
        .any(|error| error.message.contains("used before it is defined")));
}
