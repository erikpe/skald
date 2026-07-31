use super::*;

const OPERATORS: &[(ResolvedBinaryOperator, &str)] = &[
    (ResolvedBinaryOperator::Equal, "=="),
    (ResolvedBinaryOperator::NotEqual, "!="),
    (ResolvedBinaryOperator::LessThan, "<"),
    (ResolvedBinaryOperator::LessEqual, "<="),
    (ResolvedBinaryOperator::GreaterThan, ">"),
    (ResolvedBinaryOperator::GreaterEqual, ">="),
];

#[test]
fn resolution_preserves_every_comparison_without_target_selection() {
    for (expected, spelling) in OPERATORS {
        let source = format!(
            "fn compare(left: u64, right: u64) -> bool {{ return left {spelling} right; }} \
             fn main() -> i64 {{ return 0; }}"
        );
        let output = resolve_text(&source);
        assert!(!output.has_errors(), "{spelling}");
        let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
        let ResolvedExpression::Binary(comparison) = return_value(&definition.body.statements[0])
        else {
            panic!("expected resolved comparison for {spelling}");
        };

        assert_eq!(comparison.operator, *expected, "{spelling}");
        assert!(matches!(
            comparison.left.as_ref(),
            ResolvedExpression::Binding(_)
        ));
        assert!(matches!(
            comparison.right.as_ref(),
            ResolvedExpression::Binding(_)
        ));

        let dump = dump_resolved(&output.program);
        assert_eq!(dump, dump_resolved(&output.program));
        assert!(dump.contains(&format!("Binary {}", operator_dump_name(*expected))));
    }
}

#[test]
fn resolution_preserves_logical_negation_and_boolean_comparison_shapes() {
    let output = resolve_text(concat!(
        "fn compare(left: bool, right: bool) -> bool { return !left == !!right; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());
    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::Binary(comparison) = return_value(&definition.body.statements[0])
    else {
        panic!("expected resolved boolean comparison");
    };
    assert_eq!(comparison.operator, ResolvedBinaryOperator::Equal);
    let ResolvedExpression::Unary(left) = comparison.left.as_ref() else {
        panic!("expected logical negation on the left");
    };
    assert_eq!(left.operator, ResolvedUnaryOperator::LogicalNot);
    let ResolvedExpression::Unary(right) = comparison.right.as_ref() else {
        panic!("expected outer logical negation on the right");
    };
    assert_eq!(right.operator, ResolvedUnaryOperator::LogicalNot);
    assert!(matches!(
        right.operand.as_ref(),
        ResolvedExpression::Unary(ResolvedUnaryExpr {
            operator: ResolvedUnaryOperator::LogicalNot,
            ..
        })
    ));

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(dump.contains("Binary Equal"));
    assert_eq!(dump.matches("Unary LogicalNot").count(), 3);
}

const fn operator_dump_name(operator: ResolvedBinaryOperator) -> &'static str {
    match operator {
        ResolvedBinaryOperator::Equal => "Equal",
        ResolvedBinaryOperator::NotEqual => "NotEqual",
        ResolvedBinaryOperator::LessThan => "LessThan",
        ResolvedBinaryOperator::LessEqual => "LessEqual",
        ResolvedBinaryOperator::GreaterThan => "GreaterThan",
        ResolvedBinaryOperator::GreaterEqual => "GreaterEqual",
        ResolvedBinaryOperator::Add
        | ResolvedBinaryOperator::Subtract
        | ResolvedBinaryOperator::Multiply
        | ResolvedBinaryOperator::ShiftLeft
        | ResolvedBinaryOperator::ShiftRight
        | ResolvedBinaryOperator::BitwiseAnd
        | ResolvedBinaryOperator::BitwiseOr
        | ResolvedBinaryOperator::BitwiseXor => {
            panic!("comparison test received an arithmetic operator")
        }
    }
}
