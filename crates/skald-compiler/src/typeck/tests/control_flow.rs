use super::*;

#[test]
fn checks_typed_conditionals_and_preserves_ordered_arms_in_hir() {
    let output = check_text(concat!(
        "fn choose(first: bool, second: bool) -> i64 {\n",
        "  if (first) { return 1; }\n",
        "  elif (second) { return 2; }\n",
        "  else { return 3; }\n",
        "}\n",
        "fn main() -> i64 { return choose(false, true); }\n",
    ));

    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    let choose = hir.definitions.get(FunctionId::new(0)).unwrap();
    let HirStatement::Conditional(conditional) = &choose.body.statements[0] else {
        panic!("expected typed conditional");
    };
    assert_eq!(conditional.arms.len(), 2);
    assert!(conditional
        .arms
        .iter()
        .all(|arm| arm.condition.ty == Type::Bool));
    assert!(conditional.else_block.is_some());
    let dump = dump_hir(&hir);
    assert_eq!(dump.matches("ElifArm").count(), 1);
    assert!(dump.contains("Binding f0:p0 : bool"));
    assert!(dump.contains("Binding f0:p1 : bool"));
}

#[test]
fn rejects_non_boolean_conditional_conditions() {
    let output = check_text("fn main() -> i64 { if (1) { return 1; } else { return 0; } }");

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == TYPE_MISMATCH
            && diagnostic
                .message
                .contains("conditional condition has type `i64` but `bool` is required")
    }));
}

#[test]
fn exhaustive_all_returning_conditionals_satisfy_definite_return() {
    for source in [
        "fn main() -> i64 { if (true) { return 1; } else { return 2; } }",
        "fn main() -> i64 { if (false) { return 1; } elif (true) { return 2; } else { return 3; } }",
        "fn flag(value: bool) -> bool { if (value) { return true; } else { return false; } } fn main() -> i64 { return 0; }",
    ] {
        let output = check_text(source);
        assert!(!output.has_errors(), "source should type-check: {source}");
    }
}

#[test]
fn non_exhaustive_or_fallthrough_conditionals_do_not_guarantee_return() {
    for source in [
        "fn main() -> i64 { if (true) { return 1; } }",
        "fn main() -> i64 { if (true) { return 1; } else {} }",
        "fn main() -> i64 { if (true) {} else { return 1; } }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == MISSING_RETURN));
    }
}

#[test]
fn a_return_after_a_non_exhaustive_conditional_remains_definite() {
    let output = check_text("fn main() -> i64 { if (false) { return 1; } return 2; }");

    assert!(!output.has_errors());
}

#[test]
fn nested_unconditional_block_can_supply_the_return() {
    let output = check_text("fn main() -> i64 { { return 7; } }");

    assert!(!output.has_errors());
    assert!(output.hir.is_some());
}

#[test]
fn diagnoses_u64_literal_overflow() {
    let output = check_text(
        "fn too_large() -> u64 { return 18446744073709551616u; } fn main() -> i64 { return 0; }",
    );

    assert!(output.hir.is_none());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, U64_LITERAL_OUT_OF_RANGE);
    assert_eq!(
        diagnostic.message,
        "integer literal `18446744073709551616u` is out of range for `u64`"
    );
}

#[test]
fn diagnoses_u8_literal_overflow() {
    let output =
        check_text("fn too_large() -> u8 { return 256u8; } fn main() -> i64 { return 0; }");

    assert!(output.hir.is_none());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, U8_LITERAL_OUT_OF_RANGE);
    assert_eq!(
        diagnostic.message,
        "integer literal `256u8` is out of range for `u8`"
    );
}

#[test]
fn diagnoses_f64_literal_overflow() {
    let output = check_text(
        "fn value() -> f64 { return 1.7976931348623159e308; } fn main() -> i64 { return 0; }",
    );

    assert!(output.hir.is_none());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, F64_LITERAL_OUT_OF_RANGE);
    assert!(diagnostic.message.contains("out of range for `f64`"));
}
