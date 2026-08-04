use super::*;

#[test]
fn numeric_literals_preserve_their_lexical_kind_spelling_and_span() {
    let (sources, output) = parse_text("fn main() -> i64 { return 007; }");
    let Expression::NumericLiteral(literal) = return_value(function(&output.ast, 0)) else {
        panic!("expected a numeric literal");
    };

    assert_eq!(literal.kind, NumericLiteralKind::I64(IntegerRadix::Decimal));
    assert_eq!(literal.spelling, "007");
    assert_eq!(
        sources
            .get(literal.span.source_id())
            .unwrap()
            .slice(literal.span.range()),
        Some("007")
    );
}

#[test]
fn string_literals_preserve_decoded_bytes_full_span_and_stable_dump() {
    let (sources, output) = parse_text("fn value() -> i64 { return \"A\\n\\x42\\0\\\"\\\\\"; }");
    let Expression::StringLiteral(literal) = return_value(function(&output.ast, 0)) else {
        panic!("expected a string literal");
    };

    assert_eq!(literal.bytes, b"A\nB\0\"\\");
    assert_eq!(
        sources
            .get(literal.span.source_id())
            .unwrap()
            .slice(literal.span.range()),
        Some("\"A\\n\\x42\\0\\\"\\\\\"")
    );
    assert!(dump_ast(&output.ast).contains("StringBytes \"410a4200225c\""));
    assert_eq!(dump_ast(&output.ast), dump_ast(&output.ast));
}

#[test]
fn parses_u64_types_and_preserves_suffixed_literal_spelling() {
    let (_, output) = parse_text(
        "fn identity(value: u64) -> u64 { var result: u64 = value; return 42u; } fn main() -> i64 { return 0; }",
    );
    let identity = function(&output.ast, 0);

    assert_eq!(identity.parameters[0].type_syntax.kind, TypeKind::U64);
    assert_eq!(identity.return_type.kind, TypeKind::U64);
    let Expression::NumericLiteral(literal) = return_value(identity) else {
        panic!("expected a u64 literal");
    };
    assert_eq!(literal.kind, NumericLiteralKind::U64(IntegerRadix::Decimal));
    assert_eq!(literal.spelling, "42u");
    assert!(dump_ast(&output.ast).contains("U64 \"42u\""));
}

#[test]
fn parses_u8_types_and_preserves_suffixed_literal_spelling() {
    let (_, output) = parse_text(
        "fn identity(value: u8) -> u8 { var result: u8 = value; return 255u8; } fn main() -> i64 { return 0; }",
    );
    let identity = function(&output.ast, 0);

    assert_eq!(identity.parameters[0].type_syntax.kind, TypeKind::U8);
    assert_eq!(identity.return_type.kind, TypeKind::U8);
    let Expression::NumericLiteral(literal) = return_value(identity) else {
        panic!("expected a u8 literal");
    };
    assert_eq!(literal.kind, NumericLiteralKind::U8(IntegerRadix::Decimal));
    assert_eq!(literal.spelling, "255u8");
    assert!(dump_ast(&output.ast).contains("U8 \"255u8\""));
}

#[test]
fn parses_f64_types_and_preserves_decimal_literal_spelling() {
    let (_, output) = parse_text(
        "fn identity(value: f64) -> f64 { var result: f64 = value; return 6.25e-1; } fn main() -> i64 { return 0; }",
    );
    let identity = function(&output.ast, 0);

    assert_eq!(identity.parameters[0].type_syntax.kind, TypeKind::F64);
    assert_eq!(identity.return_type.kind, TypeKind::F64);
    let Expression::NumericLiteral(literal) = return_value(identity) else {
        panic!("expected an f64 literal");
    };
    assert_eq!(literal.kind, NumericLiteralKind::F64);
    assert_eq!(literal.spelling, "6.25e-1");
    assert!(dump_ast(&output.ast).contains("F64 \"6.25e-1\""));
}

#[test]
fn parses_boolean_types_and_literals_in_all_supported_positions() {
    let (_, output) = parse_text(concat!(
        "extern fn emit(value: bool) -> bool;\n",
        "fn identity(value: bool) -> bool { var result: bool = value; return result; }\n",
        "fn main() -> i64 { var value: bool = true; emit(false); return 0; }\n",
    ));

    assert!(!output.has_errors());
    let TopLevelDeclaration::ExternalFunction(external) = &output.ast.declarations[0] else {
        panic!("expected external declaration");
    };
    assert_eq!(external.parameters[0].type_syntax.kind, TypeKind::Bool);
    assert_eq!(external.return_type.kind, TypeKind::Bool);
    let main = function(&output.ast, 2);
    let Statement::Local(local) = &main.body.statements[0] else {
        panic!("expected boolean local");
    };
    assert_eq!(local.type_syntax.kind, TypeKind::Bool);
    assert!(matches!(
        local.initializer,
        Expression::Boolean(BooleanExpr { value: true, .. })
    ));

    let dump = dump_ast(&output.ast);
    assert!(dump.contains("Type Bool"));
    assert!(dump.contains("Boolean true"));
    assert!(dump.contains("Boolean false"));
}

#[test]
fn precedence_and_associativity_are_explicit() {
    let (_, output) = parse_text("fn main() -> i64 { return -a * b + c - d; }");
    assert!(!output.has_errors());

    let Expression::Binary(subtract) = return_value(function(&output.ast, 0)) else {
        panic!("outer expression must be subtraction");
    };
    assert_eq!(subtract.operator, BinaryOperator::Subtract);
    let Expression::Binary(add) = subtract.left.as_ref() else {
        panic!("subtraction left side must be addition");
    };
    assert_eq!(add.operator, BinaryOperator::Add);
    let Expression::Binary(multiply) = add.left.as_ref() else {
        panic!("addition left side must be multiplication");
    };
    assert_eq!(multiply.operator, BinaryOperator::Multiply);
    assert!(matches!(
        multiply.left.as_ref(),
        Expression::Unary(UnaryExpr {
            operator: UnaryOperator::Negate,
            ..
        })
    ));
}

#[test]
fn grouping_overrides_binary_precedence_and_preserves_its_span() {
    let (_, output) = parse_text("fn main() -> i64 { return (1 + 2) * 3; }");
    let Expression::Binary(multiply) = return_value(function(&output.ast, 0)) else {
        panic!("expected multiplication");
    };
    let Expression::Grouped(grouped) = multiply.left.as_ref() else {
        panic!("expected grouped left operand");
    };
    assert_eq!(grouped.span.range().start(), 26);
    assert_eq!(grouped.span.range().end(), 33);
    assert!(matches!(
        grouped.expression.as_ref(),
        Expression::Binary(BinaryExpr {
            operator: BinaryOperator::Add,
            ..
        })
    ));
}

#[test]
fn parser_does_not_perform_semantic_name_lookup() {
    let (_, output) =
        parse_text("fn main() -> i64 { var value: i64 = unknown(missing); return value; }");

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.ast.declarations.len(), 1);
}
