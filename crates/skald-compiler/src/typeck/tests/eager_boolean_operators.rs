use super::*;
use crate::hir::{
    HirComparisonOperand, HirComparisonPredicate, HirPrimitiveComparison, HirUnaryOperation,
};

#[test]
fn selects_exact_boolean_negation_equality_and_inequality() {
    let output = check_text(concat!(
        "fn invert(value: bool) -> bool { return !value; }\n",
        "fn equal(left: bool, right: bool) -> bool { return left == right; }\n",
        "fn different(left: bool, right: bool) -> bool { return left != right; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());
    let hir = output.hir.unwrap();

    let invert = returned_expression(hir.definitions.get(FunctionId::new(0)).unwrap());
    assert!(matches!(
        invert.kind,
        HirExpressionKind::Unary {
            operation: HirUnaryOperation::LogicalNotBool,
            ..
        }
    ));
    assert_eq!(invert.ty, Type::Bool);

    for (function, predicate) in [
        (FunctionId::new(1), HirComparisonPredicate::Equal),
        (FunctionId::new(2), HirComparisonPredicate::NotEqual),
    ] {
        let comparison = returned_expression(hir.definitions.get(function).unwrap());
        assert!(matches!(
            comparison.kind,
            HirExpressionKind::PrimitiveComparison {
                operation: HirPrimitiveComparison {
                    predicate: selected,
                    operand: HirComparisonOperand::Bool,
                },
                ..
            } if selected == predicate
        ));
        assert_eq!(comparison.ty, Type::Bool);
    }

    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains("Unary LogicalNotBool : bool"));
    assert!(dump.contains("BooleanComparison eq.bool : bool"));
    assert!(dump.contains("BooleanComparison ne.bool : bool"));
}

#[test]
fn rejects_every_boolean_numeric_equality_direction_without_conversion() {
    for (numeric_type, numeric_literal) in [("i64", "1"), ("u64", "1u"), ("u8", "1u8")] {
        for operator in ["==", "!="] {
            for (left, right, left_type, right_type) in [
                ("true", numeric_literal, "bool", numeric_type),
                (numeric_literal, "true", numeric_type, "bool"),
            ] {
                let source = format!(
                    "fn invalid() -> bool {{ return {left} {operator} {right}; }} \
                     fn main() -> i64 {{ return 0; }}"
                );
                let output = check_text(&source);
                assert!(output.hir.is_none(), "{source}");
                let diagnostic = output
                    .diagnostics
                    .iter()
                    .find(|diagnostic| diagnostic.code == TYPE_MISMATCH)
                    .unwrap();

                assert_eq!(
                    diagnostic.message,
                    "equality comparison requires operands of the same supported primitive type"
                );
                assert!(diagnostic.labels.iter().any(|label| label
                    .message
                    .contains(&format!("left operand has type `{left_type}`"))));
                assert!(diagnostic.labels.iter().any(|label| label
                    .message
                    .contains(&format!("right operand has type `{right_type}`"))));
            }
        }
    }
}

#[test]
fn logical_negation_rejects_every_unsupported_operand_family_with_its_actual_type() {
    const CASES: &[(&str, &str)] = &[
        ("fn invalid(value: i64) -> bool { return !value; }", "i64"),
        ("fn invalid(value: u64) -> bool { return !value; }", "u64"),
        ("fn invalid(value: u8) -> bool { return !value; }", "u8"),
        ("fn invalid(value: f64) -> bool { return !value; }", "f64"),
        ("fn invalid(value: i64?) -> bool { return !value; }", "i64?"),
        (
            "class Item { init() {} } fn invalid(ref value: Item) -> bool { return !value; }",
            "class c0",
        ),
        (
            "class Item { init() {} } fn invalid(value: shared Item) -> bool { return !value; }",
            "shared class c0",
        ),
        (
            "fn invalid(ref value: Obj) -> bool { return !value; }",
            "Obj",
        ),
        (
            "fn invalid(value: i64[]) -> bool { return !value; }",
            "array a0",
        ),
        (
            "fn notify() -> unit {} fn invalid() -> bool { return !notify(); }",
            "unit",
        ),
    ];

    for &(declarations, actual_type) in CASES {
        let source = format!("{declarations} fn main() -> i64 {{ return 0; }}");
        let output = check_text(&source);
        assert!(output.hir.is_none(), "{source}");
        let diagnostic = output
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == TYPE_MISMATCH)
            .unwrap();
        assert_eq!(
            diagnostic.message,
            "logical negation requires a `bool` operand"
        );
        assert!(diagnostic.labels.iter().any(|label| label
            .message
            .contains(&format!("operand has type `{actual_type}`"))));
    }
}
