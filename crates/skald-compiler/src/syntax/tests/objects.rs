use super::*;

fn class(ast: &CompilationUnit, index: usize) -> &ClassDecl {
    let TopLevelDeclaration::Class(class) = &ast.declarations[index] else {
        panic!("expected a class declaration");
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
fn parses_the_restricted_class_and_member_surface_without_lookup() {
    let (sources, output) = parse_text(concat!(
        "class Counter {\n",
        "    value: i64;\n",
        "    init(value: i64) { self.value = value; }\n",
        "    fn get() -> i64 { return self.value; }\n",
        "    mut fn add(amount: i64) -> unit { self.value = self.value + amount; }\n",
        "    init: bool;\n",
        "    fn init() -> unit {}\n",
        "}\n",
        "fn main() -> i64 { var counter: Counter = Counter(40); counter.add(2); return counter.get(); }\n",
    ));

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.ast.declarations.len(), 2);
    let counter = class(&output.ast, 0);
    assert_eq!(counter.name.text, "Counter");
    assert_eq!(counter.members.len(), 6);

    let ClassMember::Initializer(initializer) = &counter.members[1] else {
        panic!("expected initializer");
    };
    assert_eq!(source_text(&sources, initializer.introducer_span), "init");
    let Statement::FieldAssignment(assignment) = &initializer.body.statements[0] else {
        panic!("expected initializer field assignment");
    };
    assert_eq!(
        source_text(&sources, assignment.place.receiver.span()),
        "self"
    );
    assert_eq!(source_text(&sources, assignment.place.dot_span), ".");
    assert_eq!(source_text(&sources, assignment.place.member.span), "value");
    assert_eq!(source_text(&sources, assignment.equal_span), "=");

    let ClassMember::Method(method) = &counter.members[3] else {
        panic!("expected mutable method");
    };
    assert_eq!(source_text(&sources, method.mut_span.unwrap()), "mut");

    let main = function(&output.ast, 1);
    let Statement::Local(local) = &main.body.statements[0] else {
        panic!("expected object local");
    };
    let TypeKind::Named(name) = &local.type_syntax.kind else {
        panic!("expected a named local type");
    };
    assert_eq!(name.text, "Counter");
    assert_eq!(source_text(&sources, name.span), "Counter");
}

#[test]
fn member_access_and_calls_form_one_left_associative_postfix_chain() {
    let (_, output) = parse_text("fn main() -> i64 { return (counter).get().value; }");
    assert!(output.diagnostics.is_empty());

    let Expression::MemberAccess(value) = return_value(function(&output.ast, 0)) else {
        panic!("expected outer member access");
    };
    assert_eq!(value.member.text, "value");
    let Expression::Call(call) = value.receiver.as_ref() else {
        panic!("expected a call before the outer member access");
    };
    let Expression::MemberAccess(get) = call.callee.as_ref() else {
        panic!("expected member access as the call target");
    };
    assert_eq!(get.member.text, "get");
    assert!(matches!(get.receiver.as_ref(), Expression::Grouped(_)));
}

#[test]
fn grouping_preserves_a_valid_field_assignment_receiver() {
    let (_, output) = parse_text(concat!(
        "class Value { value: i64; init(value: i64) { (self).value = value; } }",
        "fn main() -> i64 { return 0; }",
    ));
    assert!(output.diagnostics.is_empty());

    let ClassMember::Initializer(initializer) = &class(&output.ast, 0).members[1] else {
        panic!("expected initializer");
    };
    let Statement::FieldAssignment(assignment) = &initializer.body.statements[0] else {
        panic!("expected field assignment");
    };
    assert!(matches!(
        assignment.place.receiver.as_ref(),
        Expression::Grouped(_)
    ));
}

#[test]
fn lifecycle_spellings_remain_ordinary_names_outside_special_member_syntax() {
    let (_, output) = parse_text(concat!(
        "class Names {\n",
        "    init: i64;\n",
        "    assign: i64;\n",
        "    destroy: i64;\n",
        "    init() {}\n",
        "    fn init(assign: i64) -> i64 { var destroy: i64 = assign; return destroy; }\n",
        "    fn assign() -> unit {}\n",
        "    fn destroy() -> unit {}\n",
        "}\n",
        "fn init(assign: i64) -> i64 { var destroy: i64 = assign; return destroy; }\n",
        "fn destroy() -> unit {}\n",
    ));

    assert!(output.diagnostics.is_empty());
    assert_eq!(class(&output.ast, 0).members.len(), 7);
    assert_eq!(function(&output.ast, 1).name.text, "init");
    assert_eq!(function(&output.ast, 2).name.text, "destroy");
}

#[test]
fn parses_a_dedicated_destructor_with_a_complete_source_span() {
    let (sources, output) = parse_text(concat!(
        "class Resource {\n",
        "    value: i64;\n",
        "    init() { self.value = 0; }\n",
        "    destroy { self.value = 1; return; }\n",
        "    fn destroy() -> unit {}\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty());
    let resource = class(&output.ast, 0);
    let ClassMember::Destructor(destructor) = &resource.members[2] else {
        panic!("expected a destructor declaration");
    };
    assert_eq!(source_text(&sources, destructor.introducer_span), "destroy");
    assert_eq!(
        source_text(&sources, destructor.span),
        "destroy { self.value = 1; return; }"
    );
    assert_eq!(destructor.body.statements.len(), 2);
    assert!(matches!(resource.members[3], ClassMember::Method(_)));
}

#[test]
fn parses_copy_assignment_as_a_dedicated_contextual_member() {
    let (sources, output) =
        parse_text("class Value { init() {} assign(ref other: Value) { return; } }");
    assert!(output.diagnostics.is_empty());

    let value = class(&output.ast, 0);
    let ClassMember::CopyAssignment(assignment) = &value.members[1] else {
        panic!("expected a copy-assignment declaration");
    };
    assert_eq!(source_text(&sources, assignment.introducer_span), "assign");
    assert_eq!(assignment.parameters.len(), 1);
    assert_eq!(assignment.parameters[0].name.text, "other");
    assert_eq!(assignment.body.statements.len(), 1);

    assert_eq!(
        dump_ast(&output.ast),
        concat!(
            "CompilationUnit @0..62\n",
            "  Class @0..62\n",
            "    Name \"Value\" @6..11\n",
            "    Members\n",
            "      Initializer @14..23\n",
            "        Introducer @14..18\n",
            "        Parameters\n",
            "        Block @21..23\n",
            "      CopyAssignment @24..60\n",
            "        Introducer @24..30\n",
            "        Parameters\n",
            "          Parameter @31..47\n",
            "            Binding ReadOnlyAlias\n",
            "              Ref @31..34\n",
            "            Name \"other\" @35..40\n",
            "            Type Named \"Value\" @42..47\n",
            "        Block @49..60\n",
            "          Return @51..58\n",
        )
    );
}

#[test]
fn malformed_destructors_recover_to_later_class_members() {
    let (_, output) = parse_text(concat!(
        "class Broken {\n",
        "    init() {}\n",
        "    destroy() {}\n",
        "    after_parameters: i64;\n",
        "    destroy -> unit {}\n",
        "    after_result: i64;\n",
        "    mut destroy {}\n",
        "    after_modifier: i64;\n",
        "    destroy;\n",
        "    after_semicolon: i64;\n",
        "    destroy\n",
        "    fn recovered() -> unit {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.has_errors());
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == INVALID_CLASS_MEMBER));
    let broken = class(&output.ast, 0);
    for expected in [
        "after_parameters",
        "after_result",
        "after_modifier",
        "after_semicolon",
    ] {
        assert!(broken.members.iter().any(
            |member| matches!(member, ClassMember::Field(field) if field.name.text == expected)
        ));
    }
    assert!(broken.members.iter().any(
        |member| matches!(member, ClassMember::Method(method) if method.name.text == "recovered")
    ));
    assert_eq!(function(&output.ast, 1).name.text, "main");
}

#[test]
fn malformed_copy_assignments_recover_to_later_class_members() {
    let (_, output) = parse_text(concat!(
        "class Broken {\n",
        "    init() {}\n",
        "    assign(ref other: Broken) -> unit {}\n",
        "    after_result: i64;\n",
        "    mut assign(ref other: Broken) {}\n",
        "    after_modifier: i64;\n",
        "    assign(ref other: Broken);\n",
        "    after_semicolon: i64;\n",
        "    assign(ref other: Broken)\n",
        "    fn recovered() -> unit {}\n",
        "}\n",
    ));

    assert!(output.has_errors());
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == INVALID_CLASS_MEMBER
            || diagnostic.code == EXPECTED_TOKEN));
    let broken = class(&output.ast, 0);
    for expected in ["after_result", "after_modifier", "after_semicolon"] {
        assert!(broken.members.iter().any(
            |member| matches!(member, ClassMember::Field(field) if field.name.text == expected)
        ));
    }
    assert!(broken.members.iter().any(
        |member| matches!(member, ClassMember::Method(method) if method.name.text == "recovered")
    ));
}

#[test]
fn destructor_ast_dump_is_exact_and_source_shaped() {
    let (_, output) = parse_text("class Empty { init() {} destroy { return; } }");
    assert!(output.diagnostics.is_empty());

    assert_eq!(
        dump_ast(&output.ast),
        concat!(
            "CompilationUnit @0..45\n",
            "  Class @0..45\n",
            "    Name \"Empty\" @6..11\n",
            "    Members\n",
            "      Initializer @14..23\n",
            "        Introducer @14..18\n",
            "        Parameters\n",
            "        Block @21..23\n",
            "      Destructor @24..43\n",
            "        Introducer @24..31\n",
            "        Block @32..43\n",
            "          Return @34..41\n",
        )
    );
}

#[test]
fn malformed_and_excluded_members_recover_to_later_members_and_declarations() {
    let (_, output) = parse_text(concat!(
        "class Broken {\n",
        "    assign(ref other: Broken) {}\n",
        "    first: i64;\n",
        "    destroy(value: i64) {}\n",
        "    mut value: i64;\n",
        "    fn good() -> i64 { return self.first; }\n",
        "}\n",
        "class Recovered { init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.has_errors());
    assert_eq!(output.ast.declarations.len(), 3);
    let broken = class(&output.ast, 0);
    assert!(broken
        .members
        .iter()
        .any(|member| matches!(member, ClassMember::CopyAssignment(_))));
    assert!(broken
        .members
        .iter()
        .any(|member| matches!(member, ClassMember::Field(field) if field.name.text == "first")));
    assert!(broken
        .members
        .iter()
        .any(|member| matches!(member, ClassMember::Method(method) if method.name.text == "good")));
    assert_eq!(class(&output.ast, 1).name.text, "Recovered");
    assert_eq!(function(&output.ast, 2).name.text, "main");
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == INVALID_CLASS_MEMBER
            || diagnostic.code == EXPECTED_TOKEN));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("destruction members")));
}

#[test]
fn named_types_are_accepted_for_fields_and_value_parameters_but_not_results() {
    let (_, output) = parse_text(concat!(
        "class Invalid {\n",
        "    child: Other;\n",
        "    init(child: Other) {}\n",
        "    fn child() -> Other {}\n",
        "    valid: i64;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.has_errors());
    assert_eq!(output.ast.declarations.len(), 2);
    let invalid = class(&output.ast, 0);
    let child = invalid
        .members
        .iter()
        .find_map(|member| match member {
            ClassMember::Field(field) if field.name.text == "child" => Some(field),
            _ => None,
        })
        .expect("named field type should remain in the AST");
    assert!(matches!(
        &child.type_syntax.kind,
        TypeKind::Named(name) if name.text == "Other"
    ));
    let initializer = invalid
        .members
        .iter()
        .find_map(|member| match member {
            ClassMember::Initializer(initializer) => Some(initializer),
            _ => None,
        })
        .expect("initializer should remain in the AST");
    assert!(matches!(
        &initializer.parameters[0].type_syntax.kind,
        TypeKind::Named(name) if name.text == "Other"
    ));
    assert!(invalid
        .members
        .iter()
        .any(|member| matches!(member, ClassMember::Field(field) if field.name.text == "valid")));
    assert_eq!(function(&output.ast, 1).name.text, "main");
}

#[test]
fn unit_remains_invalid_as_a_field_type() {
    let (_, output) = parse_text(concat!(
        "class Invalid { empty: unit; init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(output.diagnostics.len(), 1);
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, EXPECTED_TOKEN);
    assert!(diagnostic.message.contains("expected a field type"));
    assert!(diagnostic.labels[0]
        .message
        .contains("or a named class type"));
}

#[test]
fn unsupported_assignment_shapes_are_diagnosed_without_losing_later_statements() {
    let (_, output) = parse_text(concat!(
        "fn main() -> i64 {\n",
        "    value = 1;\n",
        "    make().field = 2;\n",
        "    return 0;\n",
        "}\n",
    ));

    assert!(output.has_errors());
    assert_eq!(output.diagnostics.len(), 1);
    let main = function(&output.ast, 0);
    assert_eq!(main.body.statements.len(), 2);
    assert!(matches!(
        main.body.statements[0],
        Statement::ObjectAssignment(_)
    ));
    assert!(matches!(main.body.statements[1], Statement::Return(_)));
}

#[test]
fn parses_whole_object_assignment_without_performing_type_lookup() {
    let (sources, output) = parse_text("fn main() -> i64 { destination = (source); return 0; }");

    assert!(output.diagnostics.is_empty());
    let main = function(&output.ast, 0);
    let Statement::ObjectAssignment(assignment) = &main.body.statements[0] else {
        panic!("expected object assignment syntax");
    };
    assert_eq!(
        source_text(&sources, assignment.place.span()),
        "destination"
    );
    assert_eq!(source_text(&sources, assignment.equal_span), "=");
    assert_eq!(source_text(&sources, assignment.value.span()), "(source)");
}

#[test]
fn object_ast_dump_is_exact_and_source_shaped() {
    let (_, output) =
        parse_text("class Box { value: i64; init(value: i64) { self.value = value; } }");
    assert!(output.diagnostics.is_empty());

    assert_eq!(
        dump_ast(&output.ast),
        concat!(
            "CompilationUnit @0..66\n",
            "  Class @0..66\n",
            "    Name \"Box\" @6..9\n",
            "    Members\n",
            "      Field @12..23\n",
            "        Name \"value\" @12..17\n",
            "        Type I64 @19..22\n",
            "      Initializer @24..64\n",
            "        Introducer @24..28\n",
            "        Parameters\n",
            "          Parameter @29..39\n",
            "            Binding Value\n",
            "            Name \"value\" @29..34\n",
            "            Type I64 @36..39\n",
            "        Block @41..64\n",
            "          FieldAssignment @43..62\n",
            "            Place\n",
            "              MemberAccess @43..53\n",
            "                Receiver\n",
            "                  Self @43..47\n",
            "                Dot @47..48\n",
            "                Member \"value\" @48..53\n",
            "            Equal @54..55\n",
            "            Value\n",
            "              Identifier \"value\" @56..61\n",
        )
    );
}
