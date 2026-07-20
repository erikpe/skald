use super::*;

#[test]
fn resolved_dump_is_deterministic_and_exposes_only_ids_at_uses() {
    let output = resolve_text("fn main(value: i64) -> i64 { return value; }");

    assert_eq!(
        dump_resolved(&output.program),
        concat!(
            "ResolvedProgram @0..44\n",
            "  Entry f0\n",
            "  Declarations\n",
            "    Declaration f0 \"main\" internal @0..44\n",
            "      Parameters\n",
            "        Parameter f0:p0 \"value\" @8..18\n",
            "          Type I64 @15..18\n",
            "      ReturnType\n",
            "        Type I64 @23..26\n",
            "  Definitions\n",
            "    Definition f0 @0..44\n",
            "      Locals\n",
            "      Block @27..44\n",
            "        Return @29..42\n",
            "          Binding f0:p0 @36..41\n",
        )
    );
}

#[test]
fn parsed_source_ast_still_contains_names_before_resolution() {
    // This compile-time shape check documents the phase boundary: M3 reads
    // source names, while resolved uses are represented only by BindingId
    // or FunctionId.
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("test.ska", "fn main() -> i64 { return name; }");
    let source = sources.get(source_id).unwrap();
    let tokens = lex(source).tokens;
    let ast = parse(source, &tokens).ast;
    let syntax::TopLevelDeclaration::Function(function) = &ast.declarations[0] else {
        panic!("expected function definition");
    };
    let Statement::Return(statement) = &function.body.statements[0] else {
        panic!("expected return");
    };
    assert!(matches!(
        statement.value,
        Some(syntax::Expression::Identifier(_))
    ));
}
