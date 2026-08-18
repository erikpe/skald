use super::*;

fn interface(ast: &CompilationUnit, index: usize) -> &InterfaceDecl {
    let TopLevelDeclaration::Interface(interface) = &ast.declarations[index] else {
        panic!("expected interface");
    };
    interface
}

fn source_text(sources: &SourceDatabase, span: crate::source::Span) -> &str {
    sources
        .get(span.source_id())
        .and_then(|source| source.slice(span.range()))
        .expect("AST span must belong to the test source")
}

#[test]
fn parses_interface_requirements_and_ordered_class_claims() {
    let (_, output) = parse_text(
        "interface Readable { fn read(offset: u64) -> u8; }\n\
         interface Writable { mut fn write(value: u8) -> unit; }\n\
         class Buffer implements Readable, Writable {}",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let TopLevelDeclaration::Interface(readable) = &output.ast.declarations[0] else {
        panic!("expected interface");
    };
    assert_eq!(readable.requirements[0].name.text, "read");
    let TopLevelDeclaration::Class(buffer) = &output.ast.declarations[2] else {
        panic!("expected class");
    };
    assert_eq!(
        buffer
            .implemented_interfaces
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        ["Readable", "Writable"]
    );
}

#[test]
fn recovers_after_an_invalid_interface_member() {
    let (_, output) =
        parse_text("interface Broken { value: u64; fn ok() -> unit; }\nfn main() -> unit {}");
    assert!(output.has_errors());
    assert!(matches!(
        output.ast.declarations.last(),
        Some(TopLevelDeclaration::Function(_))
    ));
}

#[test]
fn generic_interface_headers_preserve_parameters_bounds_and_nested_types() {
    let (sources, output) = parse_text(concat!(
        "public interface Mapper<Input, Output> ",
        "where Input: api::Source<Outer<Output>>, Output: Marker { ",
        "fn map(value: Input) -> Result<Output>; }",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let mapper = interface(&output.ast, 0);
    let parameters = mapper.type_parameters.as_ref().expect("type parameters");
    assert_eq!(
        parameters
            .parameters
            .iter()
            .map(|parameter| parameter.text.as_str())
            .collect::<Vec<_>>(),
        ["Input", "Output"]
    );
    assert_eq!(source_text(&sources, parameters.span), "<Input, Output>");

    let clause = mapper.where_clause.as_ref().expect("where clause");
    assert_eq!(clause.requirements.len(), 2);
    let source_bound = &clause.requirements[0].interface;
    assert_eq!(source_bound.name.text, "api::Source");
    assert_eq!(
        source_text(&sources, source_bound.span),
        "api::Source<Outer<Output>>"
    );
    assert_eq!(source_bound.arguments.as_ref().unwrap().arguments.len(), 1);
    assert_eq!(clause.requirements[1].interface.name.text, "Marker");
    assert_eq!(
        source_text(&sources, mapper.requirements[0].return_type.span),
        "Result<Output>"
    );
}

#[test]
fn generic_interface_and_applied_claim_dump_preserve_source_shape() {
    let (_, output) = parse_text(concat!(
        "interface Source<T> where T: Marker<U> { fn read() -> T; }\n",
        "class Use<T> implements api::Source<Outer<T>>, Sink<T> {}",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(
        dump_ast(&output.ast),
        concat!(
            "CompilationUnit @0..116\n",
            "  Interface @0..58\n",
            "    Name \"Source\" @10..16\n",
            "    TypeParameters @16..19\n",
            "      LeftAngle @16..17\n",
            "      Parameter \"T\" @17..18\n",
            "      RightAngle @18..19\n",
            "    WhereClause @20..38\n",
            "      Where @20..25\n",
            "      Requirement @26..38\n",
            "        Parameter \"T\" @26..27\n",
            "        Colon @27..28\n",
            "        Interface @29..38\n",
            "          Name \"Marker\" @29..35\n",
            "          Arguments @35..38\n",
            "            LeftAngle @35..36\n",
            "            Type Named \"U\" @36..37\n",
            "            RightAngle @37..38\n",
            "    Requirements\n",
            "      Requirement ReadOnly @41..56\n",
            "        Name \"read\" @44..48\n",
            "        Parameters\n",
            "        Type Named \"T\" @54..55\n",
            "  Class @59..116\n",
            "    Name \"Use\" @65..68\n",
            "    TypeParameters @68..71\n",
            "      LeftAngle @68..69\n",
            "      Parameter \"T\" @69..70\n",
            "      RightAngle @70..71\n",
            "    Implements @83..104\n",
            "      Name \"api::Source\" @83..94\n",
            "        Component \"api\" @83..86\n",
            "        Separator @86..88\n",
            "        Component \"Source\" @88..94\n",
            "      Arguments @94..104\n",
            "        LeftAngle @94..95\n",
            "        Type Named @95..103\n",
            "          Name \"Outer\" @95..100\n",
            "          Arguments @100..103\n",
            "            LeftAngle @100..101\n",
            "            Type Named \"T\" @101..102\n",
            "            RightAngle @102..103\n",
            "        RightAngle @103..104\n",
            "    Implements @106..113\n",
            "      Name \"Sink\" @106..110\n",
            "      Arguments @110..113\n",
            "        LeftAngle @110..111\n",
            "        Type Named \"T\" @111..112\n",
            "        RightAngle @112..113\n",
            "    Members\n",
        )
    );
}

#[test]
fn generic_implements_and_bounds_preserve_nested_applications() {
    let (sources, output) = parse_text(
        "class Pipeline<T> implements api::Producer<T>, Consumer<Outer<T>> \
         where T: Constraint<Inner<T>> {}",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let TopLevelDeclaration::Class(class) = &output.ast.declarations[0] else {
        panic!("expected class");
    };
    assert_eq!(class.implemented_interfaces.len(), 2);
    assert_eq!(class.implemented_interfaces[0].name.text, "api::Producer");
    assert_eq!(
        source_text(&sources, class.implemented_interfaces[1].span),
        "Consumer<Outer<T>>"
    );
    assert_eq!(
        source_text(
            &sources,
            class.where_clause.as_ref().unwrap().requirements[0]
                .interface
                .span,
        ),
        "Constraint<Inner<T>>"
    );
}

#[test]
fn where_remains_an_identifier_outside_a_generic_interface_header() {
    let (_, output) = parse_text("interface Words { fn where(where: i64) -> i64; }");

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let words = interface(&output.ast, 0);
    assert!(words.type_parameters.is_none());
    assert!(words.where_clause.is_none());
    assert_eq!(words.requirements[0].name.text, "where");
    assert_eq!(words.requirements[0].parameters[0].name.text, "where");
}

#[test]
fn malformed_generic_interface_headers_recover_to_later_declarations() {
    let cases = [
        "interface Empty<> {}",
        "interface Trailing<T,> {}",
        "interface MissingComma<T U> {}",
        "interface BadWhere<T> where T Marker {}",
        "interface TrailingBound<T> where T: Marker, {}",
        "interface DuplicateWhere<T> where T: Marker where T: Marker {}",
        "interface MissingClose<T where T: Marker {}",
    ];

    for broken in cases {
        let source = format!("{broken}\nfn recovered() -> i64 {{ return 0; }}\n");
        let (_, output) = parse_text(&source);
        assert!(output.has_errors(), "source unexpectedly parsed: {broken}");
        assert!(
            output
                .ast
                .declarations
                .iter()
                .any(|declaration| declaration.name().text == "recovered"),
            "failed to recover after {broken}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn malformed_applied_claims_report_precisely_and_recover() {
    let (_, output) = parse_text(concat!(
        "class MissingComma<T> implements Source<T> Sink<T> {}\n",
        "class Trailing<T> implements Source<T,> {}\n",
        "fn recovered() -> i64 { return 0; }\n",
    ));

    assert!(output.has_errors());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_GENERIC_SYNTAX
            && diagnostic.message == "expected `,` between implemented interfaces"
    }));
    let TopLevelDeclaration::Class(missing_comma) = &output.ast.declarations[0] else {
        panic!("expected recovered class");
    };
    assert_eq!(missing_comma.implemented_interfaces.len(), 2);
    assert!(output
        .ast
        .declarations
        .iter()
        .any(|declaration| declaration.name().text == "recovered"));
}

#[test]
fn unmatched_split_generic_closer_is_diagnosed_without_leaking_into_later_syntax() {
    let (_, output) = parse_text(concat!(
        "interface Producer<T> where T: Marker<T>> {}\n",
        "fn recovered() -> i64 { return 0; }\n",
    ));

    assert!(output.has_errors());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_GENERIC_SYNTAX
            && diagnostic.message == "unexpected `>` after generic type arguments"
    }));
    assert!(output
        .ast
        .declarations
        .iter()
        .any(|declaration| declaration.name().text == "recovered"));
}

#[test]
fn nested_generic_interface_closers_do_not_change_expression_operators() {
    let (_, output) = parse_text(concat!(
        "interface Nested<T> where T: Bound<Outer<Inner<T>>> {}\n",
        "fn compare(value: u64) -> bool { return value >> 1u >= 2u; }",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let Expression::Binary(comparison) = return_value(function(&output.ast, 1)) else {
        panic!("expected comparison");
    };
    assert_eq!(comparison.operator, BinaryOperator::GreaterEqual);
    let Expression::Binary(shift) = comparison.left.as_ref() else {
        panic!("expected shift");
    };
    assert_eq!(shift.operator, BinaryOperator::ShiftRight);
}
