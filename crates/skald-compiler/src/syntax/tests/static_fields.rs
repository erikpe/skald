use super::*;

fn class(ast: &CompilationUnit, index: usize) -> &ClassDecl {
    let TopLevelDeclaration::Class(class) = &ast.declarations[index] else {
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
fn parses_static_fields_as_distinct_source_shaped_members() {
    let (sources, output) = parse_text(concat!(
        "class State {\n",
        "  static count: i64;\n",
        "  value: u8;\n",
        "  private static ready: bool;\n",
        "  static static: u64;\n",
        "  static: f64;\n",
        "  static nested: (shared? State)[][];\n",
        "  init() {}\n",
        "  static fn reset() -> unit {}\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let state = class(&output.ast, 0);
    assert_eq!(state.members.len(), 8);

    let ClassMember::StaticField(count) = &state.members[0] else {
        panic!("expected public static field");
    };
    assert_eq!(source_text(&sources, count.span), "static count: i64;");
    assert_eq!(source_text(&sources, count.static_span), "static");
    assert_eq!(count.name.text, "count");
    assert_eq!(count.visibility, MemberVisibility::Public);

    assert!(matches!(state.members[1], ClassMember::Field(_)));
    let ClassMember::StaticField(ready) = &state.members[2] else {
        panic!("expected private static field");
    };
    assert_eq!(
        source_text(&sources, ready.span),
        "private static ready: bool;"
    );
    assert!(matches!(ready.visibility, MemberVisibility::Private { .. }));

    let ClassMember::StaticField(contextual) = &state.members[3] else {
        panic!("expected static field named static");
    };
    assert_eq!(contextual.name.text, "static");
    let ClassMember::Field(instance_contextual) = &state.members[4] else {
        panic!("expected instance field named static");
    };
    assert_eq!(instance_contextual.name.text, "static");

    let dump = dump_ast(&output.ast);
    assert!(dump.contains("StaticField"));
    assert!(dump.contains("Static @"));
    assert!(dump.contains("Name \"nested\""));
}

#[test]
fn static_remains_an_identifier_outside_the_exact_member_prefix() {
    let (_, output) = parse_text(concat!(
        "class Names {\n",
        "  private static: i64;\n",
        "  init() { self.static = 0; }\n",
        "  fn static(static: i64) -> i64 { return static; }\n",
        "}\n",
        "fn static(static: i64) -> i64 {\n",
        "  var value: i64 = static;\n",
        "  return value;\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let names = class(&output.ast, 0);
    assert!(matches!(names.members[0], ClassMember::Field(_)));
    assert!(matches!(names.members[2], ClassMember::Method(_)));
    assert_eq!(function(&output.ast, 1).name.text, "static");
}

#[test]
fn malformed_static_fields_recover_to_later_members() {
    let (_, output) = parse_text(concat!(
        "class Broken {\n",
        "  static missing_type: ;\n",
        "  static missing_colon i64;\n",
        "  static missing_semicolon: u64\n",
        "  after_missing_semicolon: bool;\n",
        "  static private misordered: i64;\n",
        "  static value: i64 = 1;\n",
        "  private private static duplicate_private: u8;\n",
        "  static recovered: f64;\n",
        "  fn after() -> unit {}\n",
        "}\n",
    ));

    assert!(output.has_errors());
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == EXPECTED_TOKEN
            || diagnostic.code == INVALID_CLASS_MEMBER));

    let broken = class(&output.ast, 0);
    assert!(broken.members.iter().any(|member| {
        matches!(member, ClassMember::Field(field) if field.name.text == "after_missing_semicolon")
    }));
    assert!(broken.members.iter().any(|member| {
        matches!(member, ClassMember::StaticField(field) if field.name.text == "duplicate_private")
    }));
    assert!(broken.members.iter().any(|member| {
        matches!(member, ClassMember::StaticField(field) if field.name.text == "recovered")
    }));
    assert!(broken.members.iter().any(|member| {
        matches!(member, ClassMember::Method(method) if method.name.text == "after")
    }));
}
