use super::*;

#[test]
fn resolution_preserves_numeric_classification_and_source_spelling() {
    let output = resolve_text("fn main() -> i64 { return 007; }");
    let main = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::NumericLiteral(literal) = return_value(&main.body.statements[0]) else {
        panic!("expected a resolved numeric literal");
    };

    assert_eq!(literal.kind, NumericLiteralKind::I64(IntegerRadix::Decimal));
    assert_eq!(literal.spelling, "007");
    assert_eq!(literal.span.range().len(), 3);
}

#[test]
fn resolution_preserves_arbitrary_precision_decimal_magnitudes_without_conversion() {
    let spelling = "9".repeat(200);
    let output = resolve_text(&format!("fn main() -> i64 {{ return {spelling}; }}"));
    let main = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::NumericLiteral(literal) = return_value(&main.body.statements[0]) else {
        panic!("expected a resolved numeric literal");
    };

    assert_eq!(literal.kind, NumericLiteralKind::I64(IntegerRadix::Decimal));
    assert_eq!(literal.spelling, spelling);
}

#[test]
fn resolution_preserves_u64_types_and_literal_magnitude() {
    let output = resolve_text(
        "fn identity(value: u64) -> u64 { return 18446744073709551615u; } fn main() -> i64 { return 0; }",
    );
    let declaration = output.program.declarations.get(FunctionId::new(0)).unwrap();
    assert_eq!(
        declaration.parameters[0].type_syntax.kind,
        ResolvedTypeKind::U64
    );
    assert_eq!(declaration.return_type.kind, ResolvedTypeKind::U64);

    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::NumericLiteral(literal) = return_value(&definition.body.statements[0])
    else {
        panic!("expected a resolved u64 literal");
    };
    assert_eq!(literal.kind, NumericLiteralKind::U64(IntegerRadix::Decimal));
    assert_eq!(literal.spelling, "18446744073709551615u");
    assert!(dump_resolved(&output.program).contains("U64 \"18446744073709551615u\""));
}

#[test]
fn resolution_preserves_u8_types_and_literal_magnitude() {
    let output = resolve_text(
        "fn identity(value: u8) -> u8 { return 255u8; } fn main() -> i64 { return 0; }",
    );
    let declaration = output.program.declarations.get(FunctionId::new(0)).unwrap();
    assert_eq!(
        declaration.parameters[0].type_syntax.kind,
        ResolvedTypeKind::U8
    );
    assert_eq!(declaration.return_type.kind, ResolvedTypeKind::U8);

    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::NumericLiteral(literal) = return_value(&definition.body.statements[0])
    else {
        panic!("expected a resolved u8 literal");
    };
    assert_eq!(literal.kind, NumericLiteralKind::U8(IntegerRadix::Decimal));
    assert_eq!(literal.spelling, "255u8");
    assert!(dump_resolved(&output.program).contains("U8 \"255u8\""));
}

#[test]
fn resolution_preserves_f64_types_and_literal_spelling() {
    let output = resolve_text(
        "fn identity(value: f64) -> f64 { return 6.25e-1; } fn main() -> i64 { return 0; }",
    );
    let declaration = output.program.declarations.get(FunctionId::new(0)).unwrap();
    assert_eq!(
        declaration.parameters[0].type_syntax.kind,
        ResolvedTypeKind::F64
    );
    assert_eq!(declaration.return_type.kind, ResolvedTypeKind::F64);

    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::NumericLiteral(literal) = return_value(&definition.body.statements[0])
    else {
        panic!("expected a resolved f64 literal");
    };
    assert_eq!(literal.kind, NumericLiteralKind::F64);
    assert_eq!(literal.spelling, "6.25e-1");
    assert!(dump_resolved(&output.program).contains("F64 \"6.25e-1\""));
}

#[test]
fn resolves_call_statements_through_the_same_stable_function_identity() {
    let output = resolve_text(concat!(
        "fn notify(value: i64) -> unit {}\n",
        "fn main() -> i64 { (notify(7)); return 0; }\n",
    ));

    assert!(!output.has_errors());
    let main = output
        .program
        .definitions
        .get(output.program.entry_function.unwrap())
        .unwrap();
    let ResolvedStatement::Expression(statement) = &main.body.statements[0] else {
        panic!("expected resolved expression statement");
    };
    let ResolvedExpression::Grouped(grouped) = &statement.expression else {
        panic!("expected source grouping to be preserved");
    };
    let ResolvedExpression::DirectCall(call) = grouped.expression.as_ref() else {
        panic!("expected resolved direct call");
    };
    assert_eq!(call.function.index(), 0);
    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(dump.contains("ExpressionStatement"));
    assert!(dump.contains("DirectCall f0"));
}
