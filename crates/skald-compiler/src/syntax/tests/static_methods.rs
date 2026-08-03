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
fn parses_static_and_private_static_methods_with_exact_spans() {
    let (sources, output) = parse_text(concat!(
        "class Tools {\n",
        "  static: i64;\n",
        "  init() { self.static = 0; }\n",
        "  static fn answer(value: i64) -> i64 { return value; }\n",
        "  private static fn helper() -> unit {}\n",
        "  fn static(static: i64) -> i64 { return static; }\n",
        "}\n",
        "fn static(static: i64) -> i64 { return static; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let tools = class(&output.ast, 0);
    let ClassMember::Field(field) = &tools.members[0] else {
        panic!("expected contextual field named static");
    };
    assert_eq!(field.name.text, "static");

    let ClassMember::Method(answer) = &tools.members[2] else {
        panic!("expected public static method");
    };
    assert_eq!(
        source_text(&sources, answer.span),
        "static fn answer(value: i64) -> i64 { return value; }"
    );
    assert_eq!(source_text(&sources, answer.static_span.unwrap()), "static");
    assert_eq!(answer.visibility, MemberVisibility::Public);
    assert!(answer.modifier.is_none());
    assert!(answer.mut_span.is_none());

    let ClassMember::Method(helper) = &tools.members[3] else {
        panic!("expected private static method");
    };
    assert_eq!(
        source_text(&sources, helper.span),
        "private static fn helper() -> unit {}"
    );
    assert!(matches!(
        helper.visibility,
        MemberVisibility::Private { .. }
    ));
    assert_eq!(source_text(&sources, helper.static_span.unwrap()), "static");

    let ClassMember::Method(contextual_method) = &tools.members[4] else {
        panic!("expected method named static");
    };
    assert_eq!(contextual_method.name.text, "static");
    assert!(contextual_method.static_span.is_none());
    assert_eq!(function(&output.ast, 1).name.text, "static");

    let dump = dump_ast(&output.ast);
    assert!(dump.contains("Method Static"));
    assert!(dump.contains("Static @"));
}

#[test]
fn rejects_invalid_static_member_forms_and_recovers_to_later_members() {
    let (_, output) = parse_text(concat!(
        "class Broken {\n",
        "  static mut fn mutable() -> unit {}\n",
        "  static virtual fn virtual_after() -> unit {}\n",
        "  virtual static fn virtual_before() -> unit {}\n",
        "  static override fn override_after() -> unit {}\n",
        "  override static fn override_before() -> unit {}\n",
        "  static static fn repeated() -> unit {}\n",
        "  static value: i64;\n",
        "  private static hidden: i64;\n",
        "  static init() {}\n",
        "  static copy(ref source: Broken) {}\n",
        "  static assign(ref source: Broken) {}\n",
        "  static destroy {}\n",
        "  static fn recovered() -> unit {}\n",
        "  after: i64;\n",
        "}\n",
    ));

    assert_eq!(output.diagnostics.len(), 10);
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == INVALID_CLASS_MEMBER));
    let messages = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(messages.contains(&"static methods cannot use `mut`"));
    assert!(messages.contains(&"static methods cannot be `virtual` or `override`"));

    let broken = class(&output.ast, 0);
    assert_eq!(
        broken
            .members
            .iter()
            .filter(|member| matches!(member, ClassMember::StaticField(_)))
            .count(),
        2
    );
    assert!(broken.members.iter().any(
        |member| matches!(member, ClassMember::Method(method) if method.name.text == "recovered")
    ));
    assert!(broken
        .members
        .iter()
        .any(|member| matches!(member, ClassMember::Field(field) if field.name.text == "after")));
}
