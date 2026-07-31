use super::*;
use crate::hir::{HirIntegerBitwiseOperation, HirIntegerType, HirUnaryOperation};

const INTEGER_TYPES: &[(HirIntegerType, &str, &str, &str)] = &[
    (HirIntegerType::I64, "i64", "-1", "42"),
    (HirIntegerType::U64, "u64", "1u", "42u"),
    (HirIntegerType::U8, "u8", "1u8", "42u8"),
];

const BINARY_OPERATORS: &[(HirIntegerBitwiseOperation, &str)] = &[
    (HirIntegerBitwiseOperation::And, "&"),
    (HirIntegerBitwiseOperation::Or, "|"),
    (HirIntegerBitwiseOperation::Xor, "^"),
];

#[test]
fn selects_the_complete_exact_type_bitwise_matrix() {
    for &(integer, type_name, left, right) in INTEGER_TYPES {
        let source = format!(
            "fn complement(value: {type_name}) -> {type_name} {{ return ~value; }} \
             fn main() -> i64 {{ return 0; }}"
        );
        let output = check_text(&source);
        assert!(!output.has_errors(), "{source}");
        let hir = output.hir.unwrap();
        let expression = returned_expression(hir.definitions.get(FunctionId::new(0)).unwrap());
        assert!(matches!(
            expression.kind,
            HirExpressionKind::Unary {
                operation: HirUnaryOperation::BitwiseComplement(selected),
                ..
            } if selected == integer
        ));
        assert_eq!(expression.ty, integer.operand_type());

        for &(operation, spelling) in BINARY_OPERATORS {
            let source = format!(
                "fn combine() -> {type_name} {{ return {left} {spelling} {right}; }} \
                 fn main() -> i64 {{ return 0; }}"
            );
            let output = check_text(&source);
            assert!(!output.has_errors(), "{source}");
            let hir = output.hir.unwrap();
            let expression = returned_expression(hir.definitions.get(FunctionId::new(0)).unwrap());
            assert!(matches!(
                expression.kind,
                HirExpressionKind::Binary {
                    operation: HirBinaryOperation::IntegerBitwise {
                        operation: selected,
                        operand,
                    },
                    ..
                } if selected == operation && operand == integer
            ));
            assert_eq!(expression.ty, integer.operand_type());
        }
    }
}

#[test]
fn rejects_every_ordered_mixed_integer_pair_without_conversion() {
    for &(_, left_type, left, _) in INTEGER_TYPES {
        for &(_, right_type, _, right) in INTEGER_TYPES {
            if left_type == right_type {
                continue;
            }
            for &(_, spelling) in BINARY_OPERATORS {
                let source = format!(
                    "fn invalid() -> {left_type} {{ return {left} {spelling} {right}; }} \
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
                    format!(
                        "bitwise `{spelling}` requires operands of the same primitive integer type"
                    )
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
fn rejects_every_noninteger_family_with_focused_actual_type_diagnostics() {
    const CASES: &[(&str, &str, &str)] = &[
        ("value: bool", "value", "bool"),
        ("value: f64", "value", "f64"),
        ("value: i64?", "value", "i64?"),
        ("value: i64[]", "value", "array a0"),
        ("ref value: Item", "value", "class c0"),
        ("value: shared Item", "value", "shared class c0"),
        ("ref value: Obj", "value", "Obj"),
    ];

    for &(parameter, operand, actual_type) in CASES {
        let prefix = if parameter.contains("Item") {
            "class Item { init() {} } "
        } else {
            ""
        };
        let mut expressions = vec![format!("~{operand}")];
        for &(_, spelling) in BINARY_OPERATORS {
            expressions.extend([
                format!("{operand} {spelling} {operand}"),
                format!("1 {spelling} {operand}"),
                format!("{operand} {spelling} 1"),
            ]);
        }
        for expression in expressions {
            let source = format!(
                "{prefix}fn invalid({parameter}) -> i64 {{ var result: i64 = {expression}; return result; }} \
                 fn main() -> i64 {{ return 0; }}"
            );
            let output = check_text(&source);
            assert!(output.hir.is_none(), "{source}");
            let diagnostic = output
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.code == TYPE_MISMATCH)
                .unwrap();
            assert!(diagnostic.message.starts_with("bitwise "));
            assert!(diagnostic
                .labels
                .iter()
                .any(|label| label.message.contains(&format!("type `{actual_type}`"))));
        }
    }

    let mut unit_expressions = vec!["~notify()".to_owned()];
    for &(_, spelling) in BINARY_OPERATORS {
        unit_expressions.extend([
            format!("notify() {spelling} notify()"),
            format!("1 {spelling} notify()"),
            format!("notify() {spelling} 1"),
        ]);
    }
    for expression in unit_expressions {
        let source = format!(
            "fn notify() -> unit {{}} fn invalid() -> i64 {{ var result: i64 = {expression}; return result; }} \
             fn main() -> i64 {{ return 0; }}"
        );
        let output = check_text(&source);
        assert!(output.hir.is_none(), "{source}");
        assert!(output.diagnostics.iter().any(|diagnostic| diagnostic
            .labels
            .iter()
            .any(|label| label.message.contains("type `unit`"))));
    }
}

#[test]
fn source_bitwise_composition_has_deterministic_exact_hir_and_mir_dumps() {
    let output = check_text(concat!(
        "fn mix(left: u8, right: u8) -> bool {\n",
        "  return (~left + 1u8 & (u8) right ^ left | right) == 255u8 && true;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump, dump_hir(&hir));
    assert!(hir_dump.contains("BitwiseComplement.u8"));
    assert!(hir_dump.contains("BitwiseAnd.u8"));
    assert!(hir_dump.contains("BitwiseXor.u8"));
    assert!(hir_dump.contains("BitwiseOr.u8"));

    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).unwrap();
    let mir_dump = crate::mir::dump_mir(&mir);
    assert_eq!(mir_dump, crate::mir::dump_mir(&mir));
    for operation in ["not.u8", "and.u8", "xor.u8", "or.u8"] {
        assert!(mir_dump.contains(operation));
    }
}
