use super::*;

fn class(ast: &CompilationUnit) -> &ClassDecl {
    let TopLevelDeclaration::Class(class) = &ast.declarations[0] else {
        panic!("expected class declaration");
    };
    class
}

fn source_text(sources: &SourceDatabase, span: crate::source::Span) -> &str {
    sources
        .get(span.source_id())
        .and_then(|source| source.slice(span.range()))
        .expect("AST span must belong to the test source")
}

#[test]
fn parses_private_cell_fields_without_reserving_contextual_names() {
    let (sources, output) = parse_text(concat!(
        "class Cache {\n",
        "  private cell cached: u64?;\n",
        "  private cell cell: i64;\n",
        "  cell: u8;\n",
        "  private: bool;\n",
        "  private cell: f64;\n",
        "  private static cell: u64;\n",
        "  init() {}\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let cache = class(&output.ast);
    let ClassMember::Field(cached) = &cache.members[0] else {
        panic!("expected cell field");
    };
    assert_eq!(
        source_text(&sources, cached.span),
        "private cell cached: u64?;"
    );
    assert_eq!(source_text(&sources, cached.cell_span.unwrap()), "cell");
    assert!(matches!(
        cached.visibility,
        MemberVisibility::Private { .. }
    ));

    let ClassMember::Field(named_cell) = &cache.members[1] else {
        panic!("expected cell field named cell");
    };
    assert_eq!(named_cell.name.text, "cell");
    assert!(named_cell.cell_span.is_some());

    for member in &cache.members[2..5] {
        let ClassMember::Field(field) = member else {
            panic!("expected ordinary contextual-name field");
        };
        assert!(field.cell_span.is_none());
    }
    assert!(
        matches!(cache.members[5], ClassMember::StaticField(ref field) if field.name.text == "cell")
    );

    let dump = dump_ast(&output.ast);
    assert_eq!(dump.matches("Cell @").count(), 2, "{dump}");
    assert_eq!(dump, dump_ast(&output.ast));
}

#[test]
fn diagnoses_invalid_cell_modifier_placement_and_recovers() {
    let (_, output) = parse_text(concat!(
        "class Broken {\n",
        "  cell exposed: i64;\n",
        "  cell private reordered: i64;\n",
        "  private cell cell repeated: i64;\n",
        "  private cell static from_cell: i64;\n",
        "  private static cell from_static: i64;\n",
        "  private cell fn recovered() -> unit {}\n",
        "  private cell init() {}\n",
        "  ok: i64;\n",
        "  init() {}\n",
        "}\n",
    ));

    assert!(output.has_errors());
    let messages = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    for expected in [
        "cell fields must be private",
        "`private` must precede `cell`",
        "a field cannot repeat `cell`",
        "cell fields cannot be static",
        "`cell` modifies only an instance field",
    ] {
        assert!(
            messages.contains(&expected),
            "missing {expected:?}: {messages:?}"
        );
    }

    let broken = class(&output.ast);
    assert!(broken
        .members
        .iter()
        .any(|member| matches!(member, ClassMember::Field(field) if field.name.text == "ok")));
    assert!(broken.members.iter().any(
        |member| matches!(member, ClassMember::Method(method) if method.name.text == "recovered")
    ));
}
