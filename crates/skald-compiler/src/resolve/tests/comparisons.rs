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
        | ResolvedBinaryOperator::Multiply => {
            panic!("comparison test received an arithmetic operator")
        }
    }
}
