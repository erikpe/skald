use super::*;

#[test]
fn selects_exact_floating_division_without_integer_failure_semantics() {
    let output = check_text(concat!(
        "fn divide(left: f64, right: f64) -> f64 { return left / right; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    let expression = returned_expression(hir.definitions.get(FunctionId::new(0)).unwrap());
    let HirExpressionKind::Binary {
        operation,
        left,
        right,
    } = &expression.kind
    else {
        panic!("expected ordinary floating binary expression");
    };
    assert_eq!(*operation, HirBinaryOperation::DivideF64);
    assert_eq!(left.ty, Type::F64);
    assert_eq!(right.ty, Type::F64);
    assert_eq!(expression.ty, Type::F64);

    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains("Binary DivideF64 : f64"));
    assert!(!dump.contains("failure=integer-division-by-zero"));
}

#[test]
fn rejects_mixed_floating_division_with_exact_actual_types() {
    let cases = [
        ("1.0", "1", "f64", "i64"),
        ("1", "1.0", "i64", "f64"),
        ("1.0", "1u", "f64", "u64"),
        ("1u8", "1.0", "u8", "f64"),
        ("true", "1.0", "bool", "f64"),
    ];
    for (left, right, left_type, right_type) in cases {
        let source = format!(
            "fn invalid() -> f64 {{ return {left} / {right}; }} \
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
            "binary `/` requires operands of the same numeric type"
        );
        assert!(diagnostic.labels.iter().any(|label| label
            .message
            .contains(&format!("left operand has type `{left_type}`"))));
        assert!(diagnostic.labels.iter().any(|label| label
            .message
            .contains(&format!("right operand has type `{right_type}`"))));
        assert!(diagnostic
            .notes
            .iter()
            .any(|note| note.contains("`i64`, `u64`, `u8`, or `f64`")));
    }
}

#[test]
fn floating_division_operands_are_checked_once_in_source_order() {
    let output = check_text(concat!(
        "fn consume(value: f64) -> f64 { return value; }\n",
        "fn invalid() -> f64 { return consume(true) / consume(1); }\n",
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
fn floating_division_composes_across_existing_operands_and_consumers() {
    let output = check_text(concat!(
        "class Box {\n",
        "  value: f64;\n",
        "  init(value: f64) { self.value = value; }\n",
        "  mut fn scale(divisor: f64) -> f64 {\n",
        "    self.value = self.value / divisor;\n",
        "    return self.value;\n",
        "  }\n",
        "}\n",
        "fn consume(value: f64) -> f64 { return value; }\n",
        "fn calculate(mut ref box: Box, values: f64[], optional: f64?) -> f64 {\n",
        "  var result: f64 = consume(box.value / values[0]);\n",
        "  result = result / optional!;\n",
        "  box.value = box.value / 2.0;\n",
        "  values[1] = result / box.scale(1.0);\n",
        "  return values[1] / box.value;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let dump = dump_hir(&hir);
    assert_eq!(dump.matches("Binary DivideF64").count(), 6);
    assert!(dump.contains("ArrayElement"));
    assert!(dump.contains("OptionalUnwrap"));
}

#[test]
fn source_floating_division_has_deterministic_hir_mir_and_assembly() {
    let output = check_text(concat!(
        "fn divide(left: f64, right: f64) -> f64 { return (left / right) / 2.0; }\n",
        "fn main() -> i64 { var result: f64 = divide(9.0, 3.0); return 0; }\n",
    ));
    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump, dump_hir(&hir));
    assert_eq!(hir_dump.matches("Binary DivideF64").count(), 2);

    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).unwrap();
    let mir_dump = crate::mir::dump_mir(&mir);
    assert_eq!(mir_dump, crate::mir::dump_mir(&mir));
    assert_eq!(mir_dump.matches("div.f64").count(), 2);
    assert!(!mir_dump.contains("integer-divisor-check"));

    let assembly = crate::test_support::emit_assembly_without_runtime_trace(
        crate::backend::Target::X86_64SysV,
        &mir,
    )
    .unwrap();
    assert_eq!(
        assembly,
        crate::test_support::emit_assembly_without_runtime_trace(
            crate::backend::Target::X86_64SysV,
            &mir,
        )
        .unwrap()
    );
    assert_eq!(assembly.matches("divsd xmm14, xmm15").count(), 2);
}
