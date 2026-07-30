use super::*;
use crate::{
    hir::{HirLogicalOperation, HirReturnValue, HirStatement},
    typeck::TYPE_MISMATCH,
};

#[test]
fn exact_boolean_operands_select_structured_logical_hir() {
    for (operator, expected) in [
        ("&&", HirLogicalOperation::And),
        ("||", HirLogicalOperation::Or),
    ] {
        let source = format!(
            "fn evaluate(left: bool, right: bool) -> bool {{ return left {operator} right; }} \
             fn main() -> i64 {{ return 0; }}"
        );
        let output = check_text(&source);
        assert!(!output.has_errors(), "{source}");
        let hir = output.hir.unwrap();
        let definition = hir.definitions.get(FunctionId::new(0)).unwrap();
        let HirStatement::Return(statement) = &definition.body.statements[0] else {
            panic!("expected return statement");
        };
        let HirReturnValue::Scalar(expression) = statement.value.as_ref().unwrap() else {
            panic!("expected scalar return");
        };
        let HirExpressionKind::Logical(logical) = &expression.kind else {
            panic!("expected structured logical HIR");
        };
        assert_eq!(logical.operation, expected);
        assert_eq!(logical.left.ty, Type::Bool);
        assert_eq!(logical.right.ty, Type::Bool);
        assert_eq!(expression.ty, Type::Bool);
    }
}

#[test]
fn logical_operators_reject_every_non_boolean_type_family_without_conversion() {
    const CASES: &[(&str, &str, &str)] = &[
        ("", "value: i64", "i64"),
        ("", "value: u64", "u64"),
        ("", "value: u8", "u8"),
        ("", "value: f64", "f64"),
        ("", "value: i64?", "i64?"),
        ("class Item { init() {} }", "ref value: Item", "class c0"),
        (
            "class Item { init() {} }",
            "value: shared Item",
            "shared class c0",
        ),
        ("interface View {}", "ref value: View", "interface i0"),
        ("", "value: i64[]", "array a0"),
    ];

    for operator in ["&&", "||"] {
        for &(declarations, parameter, actual_type) in CASES {
            for (left, right, expected_left, expected_right) in [
                ("value", "true", actual_type, "bool"),
                ("true", "value", "bool", actual_type),
            ] {
                let source = format!(
                    "{declarations} fn invalid({parameter}) -> bool {{ return {left} {operator} \
                     {right}; }} fn main() -> i64 {{ return 0; }}"
                );
                let output = check_text(&source);
                assert!(output.hir.is_none(), "{source}");
                let diagnostic = output
                    .diagnostics
                    .iter()
                    .find(|diagnostic| {
                        diagnostic.code == TYPE_MISMATCH
                            && diagnostic.message.contains("logical operator")
                    })
                    .unwrap();
                assert_eq!(
                    diagnostic.message,
                    format!("logical operator `{operator}` requires exact `bool` operands")
                );
                assert!(diagnostic.labels.iter().any(|label| label
                    .message
                    .contains(&format!("left operand has type `{expected_left}`"))));
                assert!(diagnostic.labels.iter().any(|label| label
                    .message
                    .contains(&format!("right operand has type `{expected_right}`"))));
                assert!(diagnostic
                    .notes
                    .iter()
                    .any(|note| note.contains("does not perform implicit conversion")));
            }
        }
    }
}

#[test]
fn logical_operators_report_unit_operands_by_their_actual_type() {
    for expression in ["notify() && true", "true || notify()"] {
        let source = format!(
            "fn notify() -> unit {{}} fn invalid() -> bool {{ return {expression}; }} \
             fn main() -> i64 {{ return 0; }}"
        );
        let output = check_text(&source);
        let diagnostic = output
            .diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.code == TYPE_MISMATCH && diagnostic.message.contains("logical operator")
            })
            .unwrap();
        assert!(diagnostic
            .labels
            .iter()
            .any(|label| label.message.contains("operand has type `unit`")));
    }
}

#[test]
fn both_operands_are_checked_in_source_order_before_operator_selection() {
    let output = check_text(concat!(
        "fn invalid() -> bool { return !1 && !2u; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let mismatches: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == TYPE_MISMATCH)
        .collect();
    assert_eq!(mismatches.len(), 3);
    assert_eq!(
        mismatches[0].message,
        "logical negation requires a `bool` operand"
    );
    assert_eq!(
        mismatches[1].message,
        "logical negation requires a `bool` operand"
    );
    assert_eq!(
        mismatches[2].message,
        "logical operator `&&` requires exact `bool` operands"
    );
    assert!(
        mismatches[0].labels[0].span.range().start() < mismatches[1].labels[0].span.range().start()
    );
}
