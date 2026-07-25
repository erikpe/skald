use super::*;

fn source_text(sources: &SourceDatabase, span: crate::source::Span) -> &str {
    sources
        .get(span.source_id())
        .unwrap()
        .slice(span.range())
        .unwrap()
}

fn shared_target(type_syntax: &TypeSyntax) -> (&crate::source::Span, &Name) {
    let TypeKind::Shared {
        shared_span,
        target,
    } = &type_syntax.kind
    else {
        panic!("expected shared type syntax");
    };
    (shared_span, target)
}

#[test]
fn parses_shared_storage_and_result_types_with_complete_spans() {
    let (sources, output) = parse_text(concat!(
        "class Holder {\n",
        "  value: shared Widget;\n",
        "  init(value: shared Widget) {}\n",
        "}\n",
        "fn make(value: shared Drawable) -> shared Obj {\n",
        "  var local: shared Widget = value;\n",
        "  return local;\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let TopLevelDeclaration::Class(holder) = &output.ast.declarations[0] else {
        panic!("expected class");
    };
    let ClassMember::Field(field) = &holder.members[0] else {
        panic!("expected field");
    };
    let (shared_span, target) = shared_target(&field.type_syntax);
    assert_eq!(source_text(&sources, *shared_span), "shared");
    assert_eq!(source_text(&sources, target.span), "Widget");
    assert_eq!(
        source_text(&sources, field.type_syntax.span),
        "shared Widget"
    );

    let make = function(&output.ast, 1);
    assert_eq!(
        source_text(&sources, make.parameters[0].type_syntax.span),
        "shared Drawable"
    );
    assert_eq!(source_text(&sources, make.return_type.span), "shared Obj");
    let Statement::Local(local) = &make.body.statements[0] else {
        panic!("expected local");
    };
    assert_eq!(
        source_text(&sources, local.type_syntax.span),
        "shared Widget"
    );

    let dump = dump_ast(&output.ast);
    assert_eq!(dump, dump_ast(&output.ast));
    assert!(dump.contains("Type Shared @"));
    assert!(dump.contains("Target \"Drawable\""));
    assert!(dump.contains("Target \"Obj\""));
}

#[test]
fn parses_ordinary_and_copy_allocation_without_reserving_new_or_copy() {
    let (sources, output) = parse_text(concat!(
        "fn new() -> i64 { return 1; }\n",
        "fn main() -> i64 {\n",
        "  var ordinary: shared Widget = new Widget(1, new());\n",
        "  var copied: shared Widget = new Widget(copy ordinary);\n",
        "  var named: shared Widget = new Widget(copy);\n",
        "  new Widget().field;\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let main = function(&output.ast, 1);
    let Statement::Local(ordinary) = &main.body.statements[0] else {
        panic!("expected local");
    };
    let Expression::Allocation(ordinary) = &ordinary.initializer else {
        panic!("expected allocation");
    };
    assert_eq!(source_text(&sources, ordinary.new_span), "new");
    assert_eq!(source_text(&sources, ordinary.target.span), "Widget");
    let CallArguments::Ordinary(arguments) = &ordinary.arguments else {
        panic!("expected ordinary allocation");
    };
    assert_eq!(arguments.len(), 2);
    assert!(matches!(arguments[1], Expression::Call(_)));

    let Statement::Local(copied) = &main.body.statements[1] else {
        panic!("expected local");
    };
    let Expression::Allocation(copied) = &copied.initializer else {
        panic!("expected allocation");
    };
    assert!(matches!(copied.arguments, CallArguments::Copy { .. }));

    let Statement::Local(named) = &main.body.statements[2] else {
        panic!("expected local");
    };
    let Expression::Allocation(named) = &named.initializer else {
        panic!("expected allocation");
    };
    assert!(matches!(named.arguments, CallArguments::Ordinary(_)));

    let Statement::Expression(access) = &main.body.statements[3] else {
        panic!("expected expression statement");
    };
    let Expression::MemberAccess(access) = &access.expression else {
        panic!("allocation must participate in postfix member selection");
    };
    assert!(matches!(*access.receiver, Expression::Allocation(_)));
}

#[test]
fn malformed_shared_and_allocation_forms_recover_to_later_statements() {
    let (_, output) = parse_text(concat!(
        "fn broken() -> i64 {\n",
        "  var first: shared Widget = new Widget(copy source, other);\n",
        "  var second: shared Widget = new Widget(1,);\n",
        "  return 7;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.has_errors());
    assert_eq!(output.ast.declarations.len(), 2);
    let broken = function(&output.ast, 0);
    assert!(matches!(
        broken.body.statements.last(),
        Some(Statement::Return(_))
    ));
}

#[test]
fn shared_types_do_not_change_alias_parameter_grammar() {
    let (_, output) = parse_text(concat!(
        "fn broken(ref value: shared Widget) -> i64 { return 1; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.has_errors());
    assert_eq!(output.ast.declarations.len(), 1);
    assert_eq!(function(&output.ast, 0).name.text, "main");
}

#[test]
fn explicit_dereference_preserves_precedence_member_spelling_and_spans() {
    let (sources, output) = parse_text(concat!(
        "fn main() -> i64 {\n",
        "  var product: i64 = value * *owner;\n",
        "  var selected: i64 = *owner.field;\n",
        "  owner->child.value;\n",
        "  make()->read();\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let main = function(&output.ast, 0);

    let Statement::Local(product) = &main.body.statements[0] else {
        panic!("expected product local");
    };
    let Expression::Binary(product) = &product.initializer else {
        panic!("expected binary multiplication");
    };
    assert_eq!(product.operator, BinaryOperator::Multiply);
    let Expression::Unary(dereference) = product.right.as_ref() else {
        panic!("right operand must be a dereference");
    };
    assert_eq!(dereference.operator, UnaryOperator::Dereference);
    assert_eq!(source_text(&sources, dereference.operator_span), "*");

    let Statement::Local(selected) = &main.body.statements[1] else {
        panic!("expected selected local");
    };
    let Expression::Unary(dereference) = &selected.initializer else {
        panic!("prefix dereference must bind outside postfix member access");
    };
    assert!(matches!(
        dereference.operand.as_ref(),
        Expression::MemberAccess(_)
    ));

    let Statement::Expression(path) = &main.body.statements[2] else {
        panic!("expected member path statement");
    };
    let Expression::MemberAccess(outer) = &path.expression else {
        panic!("expected outer member access");
    };
    assert!(matches!(outer.operator, MemberAccessOperator::Dot { .. }));
    let Expression::MemberAccess(inner) = outer.receiver.as_ref() else {
        panic!("expected inner member access");
    };
    assert!(matches!(inner.operator, MemberAccessOperator::Arrow { .. }));
    assert_eq!(source_text(&sources, inner.operator.span()), "->");

    let dump = dump_ast(&output.ast);
    assert!(dump.contains("Unary Dereference"));
    assert!(dump.contains("Arrow @"));
}

#[test]
fn malformed_dereference_member_recovers_to_later_statements() {
    let (_, output) = parse_text(concat!(
        "fn main() -> i64 {\n",
        "  owner->;\n",
        "  return 7;\n",
        "}\n",
    ));
    assert!(output.has_errors());
    let main = function(&output.ast, 0);
    assert!(matches!(
        main.body.statements.last(),
        Some(Statement::Return(_))
    ));
}
