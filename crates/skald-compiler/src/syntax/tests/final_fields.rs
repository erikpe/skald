use super::*;

fn class(ast: &CompilationUnit) -> &ClassDecl {
    let TopLevelDeclaration::Class(class) = &ast.declarations[0] else {
        panic!("expected class declaration");
    };
    class
}

#[test]
fn parses_canonical_final_fields_without_reserving_contextual_names() {
    let (sources, output) = parse_text(concat!(
        "class Values {\n",
        "  final value: i64;\n",
        "  private final hidden: u64;\n",
        "  final static version: u64 = 1u;\n",
        "  private final static secret: i64 = 2;\n",
        "  final: i64;\n",
        "  private final: i64;\n",
        "  final static: i64;\n",
        "  static final: i64;\n",
        "  init() {}\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let values = class(&output.ast);
    for member in &values.members[..2] {
        assert!(matches!(member, ClassMember::Field(field) if field.final_span.is_some()));
    }
    for member in &values.members[2..4] {
        assert!(matches!(member, ClassMember::StaticField(field) if field.final_span.is_some()));
    }
    for member in &values.members[4..6] {
        assert!(matches!(member, ClassMember::Field(field) if field.final_span.is_none()));
    }
    assert!(matches!(
        &values.members[6],
        ClassMember::Field(field) if field.name.text == "static" && field.final_span.is_some()
    ));
    assert!(matches!(
        &values.members[7],
        ClassMember::StaticField(field) if field.name.text == "final" && field.final_span.is_none()
    ));
    let ClassMember::Field(value) = &values.members[0] else {
        panic!("expected final field");
    };
    assert_eq!(
        sources
            .get(value.final_span.unwrap().source_id())
            .unwrap()
            .slice(value.final_span.unwrap().range()),
        Some("final")
    );

    let dump = dump_ast(&output.ast);
    assert_eq!(dump.matches("Final @").count(), 5, "{dump}");
    assert_eq!(dump, dump_ast(&output.ast));
}

#[test]
fn diagnoses_invalid_final_modifier_forms_and_recovers() {
    let (_, output) = parse_text(concat!(
        "class Broken {\n",
        "  final private reordered: i64;\n",
        "  final private static reordered_private_static: i64 = 1;\n",
        "  final final repeated: i64;\n",
        "  final static static repeated_static: i64 = 1;\n",
        "  static final reordered_static: i64 = 1;\n",
        "  final cell incompatible: i64;\n",
        "  private cell final incompatible_again: i64;\n",
        "  final initialized: i64 = 1;\n",
        "  final static missing: i64;\n",
        "  final fn recovered() -> unit {}\n",
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
        "`private` must precede `final`",
        "a field cannot repeat `final`",
        "a field cannot repeat `static`",
        "`final` must precede `static`",
        "a field cannot be both `final` and `cell`",
        "a field cannot be both `cell` and `final`",
        "instance fields cannot have declaration initializers",
        "final static fields require an initializer",
        "`final` modifies only an instance or static field",
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
