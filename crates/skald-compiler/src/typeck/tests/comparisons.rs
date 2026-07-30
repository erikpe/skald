use super::*;
use crate::hir::{
    HirComparisonOperand, HirComparisonPredicate, HirIntegerType, HirPrimitiveComparison,
};

const OPERATORS: &[(HirComparisonPredicate, &str)] = &[
    (HirComparisonPredicate::Equal, "=="),
    (HirComparisonPredicate::NotEqual, "!="),
    (HirComparisonPredicate::LessThan, "<"),
    (HirComparisonPredicate::LessEqual, "<="),
    (HirComparisonPredicate::GreaterThan, ">"),
    (HirComparisonPredicate::GreaterEqual, ">="),
];

const INTEGER_TYPES: &[(HirIntegerType, &str, &str, &str)] = &[
    (HirIntegerType::I64, "i64", "-1", "2"),
    (HirIntegerType::U64, "u64", "1u", "2u"),
    (HirIntegerType::U8, "u8", "1u8", "2u8"),
];

#[test]
fn checks_all_eighteen_exact_type_integer_comparisons() {
    for (operand, type_name, left, right) in INTEGER_TYPES {
        for (predicate, spelling) in OPERATORS {
            let source = format!(
                "fn compare() -> bool {{ return {left} {spelling} {right}; }} \
                 fn main() -> i64 {{ return 0; }}"
            );
            let output = check_text(&source);
            assert!(
                !output.has_errors(),
                "{type_name} comparison {spelling} failed"
            );
            let hir = output.hir.unwrap();
            let comparison = returned_expression(hir.definitions.get(FunctionId::new(0)).unwrap());
            let HirExpressionKind::PrimitiveComparison {
                operation,
                left,
                right,
            } = &comparison.kind
            else {
                panic!("expected typed integer comparison");
            };

            assert_eq!(
                *operation,
                HirPrimitiveComparison {
                    predicate: *predicate,
                    operand: HirComparisonOperand::Integer(*operand),
                }
            );
            assert_eq!(operation.operand_type(), operand.operand_type());
            assert_eq!(operation.result_type(), Type::Bool);
            assert_eq!(comparison.ty, Type::Bool);
            assert_eq!(left.ty, operand.operand_type());
            assert_eq!(right.ty, operand.operand_type());

            let dump = dump_hir(&hir);
            assert_eq!(dump, dump_hir(&hir));
            assert!(dump.contains(&format!(
                "IntegerComparison {}.{type_name} : bool",
                predicate.mnemonic()
            )));
        }
    }
}

#[test]
fn rejects_every_ordered_mixed_integer_comparison_with_both_actual_types() {
    for &(_, left_type, left, _) in INTEGER_TYPES {
        for &(_, right_type, _, right) in INTEGER_TYPES {
            if left_type == right_type {
                continue;
            }
            for &(_, spelling) in OPERATORS {
                let source = format!(
                    "fn compare() -> bool {{ return {left} {spelling} {right}; }} \
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
                    if matches!(spelling, "==" | "!=") {
                        "equality comparison requires operands of the same supported primitive type"
                    } else {
                        "ordering comparison requires operands of the same primitive integer type"
                    }
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
fn rejects_every_predicate_for_each_noninteger_operand_family_before_hir() {
    const SOURCES: &[&str] = &[
        "fn compare() -> bool { return 1.0 {operator} 2.0; } fn main() -> i64 { return 0; }",
        "fn compare(left: i64?, right: i64?) -> bool { return left {operator} right; } fn main() -> i64 { return 0; }",
        "fn notify() -> unit {} fn compare() -> bool { return notify() {operator} notify(); } fn main() -> i64 { return 0; }",
        "class Item { init() {} } fn compare(ref left: Item, ref right: Item) -> bool { return left {operator} right; } fn main() -> i64 { return 0; }",
        "class Item { init() {} } fn compare(left: shared Item, right: shared Item) -> bool { return left {operator} right; } fn main() -> i64 { return 0; }",
        "fn compare(ref left: Obj, ref right: Obj) -> bool { return left {operator} right; } fn main() -> i64 { return 0; }",
        "fn compare(left: i64[], right: i64[]) -> bool { return left {operator} right; } fn main() -> i64 { return 0; }",
    ];

    for template in SOURCES {
        for &(_, spelling) in OPERATORS {
            let source = template.replace("{operator}", spelling);
            let output = check_text(&source);
            assert!(output.has_errors(), "{source}");
            assert!(output.hir.is_none(), "{source}");
        }
    }
}

#[test]
fn rejects_boolean_ordering_before_hir_with_both_actual_types() {
    for &(_, spelling) in &OPERATORS[2..] {
        let source = format!(
            "fn compare() -> bool {{ return true {spelling} false; }} \
             fn main() -> i64 {{ return 0; }}"
        );
        let output = check_text(&source);
        assert!(output.hir.is_none());
        let diagnostic = output
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == TYPE_MISMATCH)
            .unwrap();
        assert_eq!(
            diagnostic.message,
            "ordering comparison requires operands of the same primitive integer type"
        );
        assert_eq!(
            diagnostic
                .labels
                .iter()
                .filter(|label| label.message.contains("operand has type `bool`"))
                .count(),
            2
        );
    }
}
