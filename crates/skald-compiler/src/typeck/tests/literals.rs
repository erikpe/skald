use super::*;

#[test]
fn checks_boolean_values_without_conflating_them_with_i64() {
    let output = check_text(concat!(
        "extern fn external_flag(value: bool) -> bool;\n",
        "fn identity(value: bool) -> bool { var result: bool = value; return result; }\n",
        "fn main() -> i64 { var flag: bool = external_flag(true); var echoed: bool = identity(flag); return 0; }\n",
    ));

    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    let external = hir.declarations.get(FunctionId::new(0)).unwrap();
    assert_eq!(external.parameters[0].ty, Type::Bool);
    assert_eq!(external.return_type, Type::Bool);
    let identity = hir.declarations.get(FunctionId::new(1)).unwrap();
    assert_eq!(identity.parameters[0].ty, Type::Bool);
    assert_eq!(identity.return_type, Type::Bool);
    let main = hir.definitions.get(hir.entry_function).unwrap();
    assert!(main.locals.iter().all(|local| local.ty == Type::Bool));

    let dump = dump_hir(&hir);
    assert!(dump.contains("Boolean true : bool"));
    assert!(dump.contains("DirectCall f0 : bool"));
}

#[test]
fn rejects_boolean_i64_mismatches_and_boolean_arithmetic() {
    for source in [
        "fn flag() -> bool { return 1; } fn main() -> i64 { return 0; }",
        "fn flag(value: bool) -> bool { return value; } fn main() -> i64 { flag(1); return 0; }",
        "fn main() -> i64 { var flag: bool = true; return flag + 1; }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));
    }
}

#[test]
fn rejects_discarded_i64_calls_and_non_call_expression_statements() {
    for (source, message) in [
        (
            "fn value() -> i64 { return 1; } fn main() -> i64 { value(); return 0; }",
            "returning `unit`",
        ),
        (
            "fn main() -> i64 { 1 + 2; return 0; }",
            "only function calls",
        ),
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == INVALID_CALL_STATEMENT && diagnostic.message.contains(message)
        }));
    }
}

#[test]
fn main_must_remain_i64_returning() {
    for source in [
        "fn main() -> unit {}",
        "fn main() -> bool { return false; }",
        "fn main() -> u64 { return 0u; }",
        "fn main() -> f64 { return 0.0; }",
    ] {
        let output = check_text(source);

        assert!(output.hir.is_none());
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == INVALID_ENTRY_POINT
                && diagnostic.message.contains("fn main() -> i64")
        }));
    }
}

#[test]
fn every_i64_function_must_return_a_value() {
    let output = check_text("fn main() -> i64 { var value: i64 = 0; }");

    assert!(output.hir.is_none());
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        MISSING_RETURN
    );
}

#[test]
fn positive_i64_maximum_is_accepted() {
    let output = check_text("fn main() -> i64 { return 9223372036854775807; }");
    let hir = output.hir.unwrap();

    assert!(matches!(
        returned_expression(hir.definitions.get(hir.entry_function).unwrap()).kind,
        HirExpressionKind::I64(i64::MAX)
    ));
}

#[test]
fn hexadecimal_literals_convert_to_existing_typed_integer_constants() {
    let output = check_text(concat!(
        "fn signed() -> i64 { return 0X7fffffffffffffff; }\n",
        "fn unsigned() -> u64 { return 0xffffffffffffffffu; }\n",
        "fn byte() -> u8 { return 0Xffu8; }\n",
        "fn main() -> i64 { return 0x0; }\n",
    ));
    let hir = output
        .hir
        .expect("in-range hexadecimal literals must produce HIR");

    assert!(matches!(
        returned_expression(hir.definitions.get(FunctionId::new(0)).unwrap()).kind,
        HirExpressionKind::I64(i64::MAX)
    ));
    assert!(matches!(
        returned_expression(hir.definitions.get(FunctionId::new(1)).unwrap()).kind,
        HirExpressionKind::U64(u64::MAX)
    ));
    assert!(matches!(
        returned_expression(hir.definitions.get(FunctionId::new(2)).unwrap()).kind,
        HirExpressionKind::U8(u8::MAX)
    ));
    assert!(matches!(
        returned_expression(hir.definitions.get(FunctionId::new(3)).unwrap()).kind,
        HirExpressionKind::I64(0)
    ));
}

#[test]
fn checks_u64_literals_signatures_and_typed_arithmetic() {
    let output = check_text(concat!(
        "extern fn observe(value: u64) -> unit;\n",
        "fn calculate(left: u64, right: u64) -> u64 { return left + right * 2u - 1u; }\n",
        "fn maximum() -> u64 { return 18446744073709551615u; }\n",
        "fn main() -> i64 { var value: u64 = calculate(maximum(), 1u); observe(value); return 0; }\n",
    ));
    let hir = output.hir.expect("valid u64 program must produce HIR");
    let calculate = hir.definitions.get(FunctionId::new(1)).unwrap();
    let expression = returned_expression(calculate);

    assert_eq!(expression.ty, Type::U64);
    assert!(matches!(
        expression.kind,
        HirExpressionKind::Binary {
            operation: HirBinaryOperation::SubtractU64,
            ..
        }
    ));
    let maximum = hir.definitions.get(FunctionId::new(2)).unwrap();
    assert!(matches!(
        returned_expression(maximum).kind,
        HirExpressionKind::U64(u64::MAX)
    ));
}

#[test]
fn rejects_implicit_i64_u64_conversions_and_unsigned_negation() {
    for (source, expected) in [
        (
            "fn value() -> u64 { return 1; } fn main() -> i64 { return 0; }",
            "return value has type `i64` but `u64` is required",
        ),
        (
            "fn value() -> i64 { return 1u; } fn main() -> i64 { return 0; }",
            "return value has type `u64` but `i64` is required",
        ),
        (
            "fn value() -> u64 { return 1u + 2; } fn main() -> i64 { return 0; }",
            "binary arithmetic requires operands of the same numeric type",
        ),
        (
            "fn value() -> i64 { return 1 + 2u; } fn main() -> i64 { return 0; }",
            "binary arithmetic requires operands of the same numeric type",
        ),
        (
            "fn main() -> i64 { var value: i64 = 1u; return 0; }",
            "local initializer has type `u64` but `i64` is required",
        ),
        (
            "fn take(value: u64) -> unit {} fn main() -> i64 { take(1); return 0; }",
            "call argument has type `i64` but `u64` is required",
        ),
        (
            "fn take(value: i64) -> unit {} fn main() -> i64 { take(1u); return 0; }",
            "call argument has type `u64` but `i64` is required",
        ),
        (
            "fn value() -> u64 { return -1u; } fn main() -> i64 { return 0; }",
            "unary negation requires an `i64` or `f64` operand",
        ),
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == expected));
    }
}

#[test]
fn checks_u8_bounds_signatures_and_typed_arithmetic() {
    let output = check_text(concat!(
        "extern fn observe(value: u8) -> unit;\n",
        "fn calculate(left: u8, right: u8) -> u8 { return left + right * 2u8 - 1u8; }\n",
        "fn bounds() -> u8 { var zero: u8 = 0u8; return 255u8; }\n",
        "fn main() -> i64 { observe(calculate(bounds(), 1u8)); return 0; }\n",
    ));
    let hir = output.hir.expect("valid u8 program must produce HIR");
    let calculate = hir.definitions.get(FunctionId::new(1)).unwrap();
    let expression = returned_expression(calculate);

    assert_eq!(expression.ty, Type::U8);
    assert!(matches!(
        expression.kind,
        HirExpressionKind::Binary {
            operation: HirBinaryOperation::SubtractU8,
            ..
        }
    ));
    let bounds = hir.definitions.get(FunctionId::new(2)).unwrap();
    assert!(matches!(
        returned_expression(bounds).kind,
        HirExpressionKind::U8(u8::MAX)
    ));
}

#[test]
fn rejects_implicit_u8_conversions_and_unsigned_negation() {
    for (source, expected) in [
        (
            "fn value() -> u8 { return 1; } fn main() -> i64 { return 0; }",
            "return value has type `i64` but `u8` is required",
        ),
        (
            "fn value() -> u64 { return 1u8; } fn main() -> i64 { return 0; }",
            "return value has type `u8` but `u64` is required",
        ),
        (
            "fn value() -> u8 { return 1u8 + 2u; } fn main() -> i64 { return 0; }",
            "binary arithmetic requires operands of the same numeric type",
        ),
        (
            "fn main() -> i64 { var value: i64 = 1u8; return 0; }",
            "local initializer has type `u8` but `i64` is required",
        ),
        (
            "fn take(value: u8) -> unit {} fn main() -> i64 { take(1u); return 0; }",
            "call argument has type `u64` but `u8` is required",
        ),
        (
            "fn value() -> u8 { return -1u8; } fn main() -> i64 { return 0; }",
            "unary negation requires an `i64` or `f64` operand",
        ),
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == expected));
    }
}

#[test]
fn checks_f64_raw_bits_signatures_and_typed_arithmetic() {
    let output = check_text(concat!(
        "extern fn observe(value: f64) -> unit;\n",
        "fn calculate(left: f64, right: f64) -> f64 { return -(left + right * 2.0 - 1.0); }\n",
        "fn main() -> i64 { var value: f64 = calculate(1.5, 2e0); observe(value); return 0; }\n",
    ));
    let hir = output.hir.expect("valid f64 program must produce HIR");
    let calculate = hir.definitions.get(FunctionId::new(1)).unwrap();
    let expression = returned_expression(calculate);

    assert_eq!(expression.ty, Type::F64);
    assert!(matches!(
        expression.kind,
        HirExpressionKind::Unary {
            operation: crate::hir::HirUnaryOperation::NegateF64,
            ..
        }
    ));
    let dump = dump_hir(&hir);
    assert!(dump.contains("F64 0x3ff8000000000000 : f64"));
    assert!(dump.contains("MultiplyF64"));
    assert!(dump.contains("SubtractF64"));
}

#[test]
fn converts_f64_boundaries_once_to_exact_raw_bits() {
    for (spelling, expected_bits) in [
        ("0.0", 0_u64),
        (
            "1.00000000000000011102230246251565404236316680908203125",
            1.0_f64.to_bits(),
        ),
        ("4.9406564584124654e-324", 1_u64),
        ("1e-400", 0_u64),
        ("1.7976931348623157e308", f64::MAX.to_bits()),
    ] {
        let output = check_text(&format!(
            "fn value() -> f64 {{ return {spelling}; }} fn main() -> i64 {{ return 0; }}"
        ));
        let hir = output.hir.expect("finite f64 literal must type-check");
        let value = hir.definitions.get(FunctionId::new(0)).unwrap();
        assert!(matches!(
            returned_expression(value).kind,
            HirExpressionKind::F64Bits(bits) if bits == expected_bits
        ));
    }
}

#[test]
fn rejects_implicit_f64_conversions_and_truthiness() {
    for (source, expected) in [
        (
            "fn value() -> f64 { return 1; } fn main() -> i64 { return 0; }",
            "return value has type `i64` but `f64` is required",
        ),
        (
            "fn value() -> i64 { return 1.0; } fn main() -> i64 { return 0; }",
            "return value has type `f64` but `i64` is required",
        ),
        (
            "fn value() -> f64 { return 1.0 + 2; } fn main() -> i64 { return 0; }",
            "binary arithmetic requires operands of the same numeric type",
        ),
        (
            "fn take(value: f64) -> unit {} fn main() -> i64 { take(1); return 0; }",
            "call argument has type `i64` but `f64` is required",
        ),
        (
            "fn main() -> i64 { if (1.0) { return 1; } return 0; }",
            "conditional condition has type `f64` but `bool` is required",
        ),
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none(), "{source}");
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message == expected),
            "{source}"
        );
    }
}

#[test]
fn unary_minus_admits_the_i64_minimum_boundary() {
    let output = check_text("fn main() -> i64 { return -9223372036854775808; }");
    let hir = output.hir.unwrap();
    let expression = returned_expression(hir.definitions.get(hir.entry_function).unwrap());

    assert_eq!(expression.ty, Type::I64);
    assert!(matches!(expression.kind, HirExpressionKind::I64(i64::MIN)));
}

#[test]
fn grouping_does_not_break_the_i64_minimum_boundary() {
    let output = check_text("fn main() -> i64 { return -(9223372036854775808); }");

    assert!(output.hir.is_some());
    assert!(output.diagnostics.is_empty());
}

#[test]
fn unary_minus_admits_the_grouped_hexadecimal_i64_minimum_boundary() {
    let output = check_text("fn main() -> i64 { return -((0X8000000000000000)); }");
    let hir = output
        .hir
        .expect("grouped hexadecimal i64 minimum must type-check");

    assert!(matches!(
        returned_expression(hir.definitions.get(hir.entry_function).unwrap()).kind,
        HirExpressionKind::I64(i64::MIN)
    ));
}

#[test]
fn hexadecimal_integer_extrema_use_existing_range_diagnostics() {
    for (source, expected_code) in [
        (
            "fn main() -> i64 { return 0x8000000000000000; }",
            INTEGER_LITERAL_OUT_OF_RANGE,
        ),
        (
            "fn main() -> i64 { return -0x8000000000000001; }",
            INTEGER_LITERAL_OUT_OF_RANGE,
        ),
        (
            "fn value() -> u64 { return 0x10000000000000000u; } fn main() -> i64 { return 0; }",
            U64_LITERAL_OUT_OF_RANGE,
        ),
        (
            "fn value() -> u8 { return 0x100u8; } fn main() -> i64 { return 0; }",
            U8_LITERAL_OUT_OF_RANGE,
        ),
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none(), "{source}");
        assert_eq!(output.diagnostics.len(), 1, "{source}");
        let diagnostic = output.diagnostics.iter().next().unwrap();
        assert_eq!(diagnostic.code, expected_code, "{source}");
        assert!(diagnostic.message.contains("0x"), "{source}");
    }
}

#[test]
fn positive_and_negative_out_of_range_literals_are_diagnosed() {
    for source in [
        "fn main() -> i64 { return 9223372036854775808; }",
        "fn main() -> i64 { return -9223372036854775809; }",
        "fn main() -> i64 { return 999999999999999999999999999999999999999; }",
        "fn main() -> i64 { return -999999999999999999999999999999999999999; }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(
            output.diagnostics.iter().next().unwrap().code,
            INTEGER_LITERAL_OUT_OF_RANGE
        );
    }
}
