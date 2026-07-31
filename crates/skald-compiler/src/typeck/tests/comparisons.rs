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
fn checks_all_six_exact_floating_comparisons() {
    for (predicate, spelling) in OPERATORS {
        let source = format!(
            "fn compare(left: f64, right: f64) -> bool {{ return left {spelling} right; }} \
             fn main() -> i64 {{ return 0; }}"
        );
        let output = check_text(&source);
        assert!(
            !output.has_errors(),
            "floating comparison {spelling} failed"
        );
        let hir = output.hir.unwrap();
        let comparison = returned_expression(hir.definitions.get(FunctionId::new(0)).unwrap());
        let HirExpressionKind::PrimitiveComparison {
            operation,
            left,
            right,
        } = &comparison.kind
        else {
            panic!("expected typed floating comparison");
        };

        assert_eq!(
            *operation,
            HirPrimitiveComparison {
                predicate: *predicate,
                operand: HirComparisonOperand::F64,
            }
        );
        assert_eq!(operation.operand_type(), Type::F64);
        assert_eq!(operation.result_type(), Type::Bool);
        assert_eq!(comparison.ty, Type::Bool);
        assert_eq!(left.ty, Type::F64);
        assert_eq!(right.ty, Type::F64);

        let dump = dump_hir(&hir);
        assert_eq!(dump, dump_hir(&hir));
        assert!(dump.contains(&format!(
            "FloatingComparison {}.f64 : bool",
            predicate.mnemonic()
        )));
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
                        format!("binary `{spelling}` requires operands of the same supported primitive type")
                    } else {
                        format!("binary `{spelling}` requires operands of the same numeric type")
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
fn rejects_every_predicate_for_each_unsupported_operand_family_before_hir() {
    const SOURCES: &[&str] = &[
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
            format!("binary `{spelling}` requires operands of the same numeric type")
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

#[test]
fn rejects_mixed_floating_comparisons_with_exact_actual_types() {
    let cases = [
        ("left", "integer", "f64", "i64"),
        ("integer", "left", "i64", "f64"),
        ("left", "unsigned", "f64", "u64"),
        ("byte", "left", "u8", "f64"),
        ("left", "flag", "f64", "bool"),
    ];
    for (left, right, left_type, right_type) in cases {
        for &(_, spelling) in OPERATORS {
            let source = format!(
                "fn invalid(left: f64, integer: i64, unsigned: u64, byte: u8, flag: bool) -> bool {{ \
                   return {left} {spelling} {right}; \
                 }} \
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
                    format!("binary `{spelling}` requires operands of the same supported primitive type")
                } else {
                    format!("binary `{spelling}` requires operands of the same numeric type")
                }
            );
            assert!(diagnostic.labels.iter().any(|label| label
                .message
                .contains(&format!("left operand has type `{left_type}`"))));
            assert!(diagnostic.labels.iter().any(|label| label
                .message
                .contains(&format!("right operand has type `{right_type}`"))));
            assert!(diagnostic.notes.iter().any(|note| note.contains("`f64`")));
        }
    }
}

#[test]
fn valid_floating_comparison_operands_are_checked_once_in_source_order() {
    let output = check_text(concat!(
        "fn consume(value: f64) -> f64 { return value; }\n",
        "fn invalid() -> bool { return consume(true) < consume(1); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.hir.is_none());
    let starts: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == TYPE_MISMATCH)
        .filter_map(|diagnostic| diagnostic.labels.first())
        .map(|label| label.span.range().start())
        .collect();
    assert_eq!(starts.len(), 2);
    assert!(starts[0] < starts[1]);
}

#[test]
fn floating_comparisons_compose_with_existing_operands_and_consumers() {
    let output = check_text(concat!(
        "class Metric {\n",
        "  value: f64; flag: bool;\n",
        "  init(value: f64) { self.value = value; self.flag = false; }\n",
        "  fn read() -> f64 { return self.value; }\n",
        "  destroy {}\n",
        "}\n",
        "fn make(value: f64) -> shared Metric { return new Metric(value); }\n",
        "fn consume(value: bool) -> bool { return value; }\n",
        "fn evaluate(mut ref metric: Metric, values: f64[], optional: f64?, flags: bool[]) -> bool {\n",
        "  var result: bool = consume(metric.value / 2.0 < values[0]);\n",
        "  result = !(optional! >= metric.read()) == false;\n",
        "  flags[0] = result && make(3.0)->read() != 0.0;\n",
        "  metric.flag = flags[0] || values[1] <= optional!;\n",
        "  if (metric.flag) { result = values[0] > values[1]; }\n",
        "  elif (values[0] == values[1]) { result = false; }\n",
        "  while (result && values[0] >= 0.0) { return result; }\n",
        "  return result;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let dump = dump_hir(&hir);
    assert!(dump.matches("FloatingComparison").count() >= 7);
    assert!(dump.contains("Logical And"));
    assert!(dump.contains("Logical Or"));
    assert!(dump.contains("While"));
}

#[test]
fn source_floating_comparisons_have_deterministic_hir_mir_and_assembly() {
    let output = check_text(concat!(
        "fn compare(left: f64, right: f64) -> bool {\n",
        "  return left / right < right / left || left == right;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump, dump_hir(&hir));
    assert_eq!(hir_dump.matches("FloatingComparison").count(), 2);

    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).unwrap();
    let mir_dump = crate::mir::dump_mir(&mir);
    assert_eq!(mir_dump, crate::mir::dump_mir(&mir));
    assert!(mir_dump.contains("lt.f64"));
    assert!(mir_dump.contains("eq.f64"));

    let assembly = crate::backend::emit_assembly(crate::backend::Target::X86_64SysV, &mir).unwrap();
    assert_eq!(
        assembly,
        crate::backend::emit_assembly(crate::backend::Target::X86_64SysV, &mir).unwrap()
    );
    assert_eq!(assembly.matches("ucomisd xmm14, xmm15").count(), 2);
}
