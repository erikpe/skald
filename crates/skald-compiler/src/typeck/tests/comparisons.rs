use super::*;
use crate::hir::{HirComparisonPredicate, HirIntegerComparison, HirIntegerType};

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
            let HirExpressionKind::IntegerComparison {
                operation,
                left,
                right,
            } = &comparison.kind
            else {
                panic!("expected typed integer comparison");
            };

            assert_eq!(
                *operation,
                HirIntegerComparison {
                    predicate: *predicate,
                    operand: *operand,
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
fn mixed_integer_comparisons_report_both_actual_types() {
    let output =
        check_text("fn compare() -> bool { return 1 < 2u; } fn main() -> i64 { return 0; }");
    assert!(output.hir.is_none());
    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == TYPE_MISMATCH)
        .unwrap();

    assert_eq!(
        diagnostic.message,
        "integer comparison requires operands of the same primitive integer type"
    );
    assert!(diagnostic
        .labels
        .iter()
        .any(|label| label.message.contains("left operand has type `i64`")));
    assert!(diagnostic
        .labels
        .iter()
        .any(|label| label.message.contains("right operand has type `u64`")));
}

#[test]
fn rejects_noninteger_comparison_families_before_hir() {
    for source in [
        "fn compare() -> bool { return true == false; } fn main() -> i64 { return 0; }",
        "fn compare() -> bool { return 1.0 < 2.0; } fn main() -> i64 { return 0; }",
        "fn compare(left: i64?, right: i64?) -> bool { return left == right; } fn main() -> i64 { return 0; }",
        "fn notify() -> unit {} fn compare() -> bool { return notify() == notify(); } fn main() -> i64 { return 0; }",
        "class Item { init() {} } fn compare(ref left: Item, ref right: Item) -> bool { return left == right; } fn main() -> i64 { return 0; }",
        "fn compare(left: i64[], right: i64[]) -> bool { return left == right; } fn main() -> i64 { return 0; }",
    ] {
        let output = check_text(source);
        assert!(output.has_errors(), "{source}");
        assert!(output.hir.is_none(), "{source}");
    }
}
