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
fn parses_alias_modes_uniformly_for_every_callable_form() {
    let (sources, output) = parse_text(concat!(
        "class Sample {\n",
        "    init(ref source: Sample) {}\n",
        "    fn view(ref other: Sample) -> unit {}\n",
        "    mut fn edit(mut ref other: Sample) -> unit {}\n",
        "}\n",
        "fn use(ref item: Sample, mut ref target: Sample, count: i64) -> unit {}\n",
        "extern fn imported(ref item: Sample) -> unit;\n",
    ));

    assert!(output.diagnostics.is_empty());
    let sample = class(&output.ast, 0);

    let ClassMember::Initializer(initializer) = &sample.members[0] else {
        panic!("expected initializer");
    };
    assert_read_only_alias(&sources, &initializer.parameters[0], "source", "Sample");

    let ClassMember::Method(view) = &sample.members[1] else {
        panic!("expected read-only method");
    };
    assert_read_only_alias(&sources, &view.parameters[0], "other", "Sample");

    let ClassMember::Method(edit) = &sample.members[2] else {
        panic!("expected mutable method");
    };
    assert_mutable_alias(&sources, &edit.parameters[0], "other", "Sample");

    let use_function = function(&output.ast, 1);
    assert_read_only_alias(&sources, &use_function.parameters[0], "item", "Sample");
    assert_mutable_alias(&sources, &use_function.parameters[1], "target", "Sample");
    assert_eq!(
        use_function.parameters[2].binding_mode,
        ParameterBindingMode::Value
    );

    let TopLevelDeclaration::ExternalFunction(imported) = &output.ast.declarations[2] else {
        panic!("expected external declaration");
    };
    assert_read_only_alias(&sources, &imported.parameters[0], "item", "Sample");
}

#[test]
fn alias_parameter_dump_is_exact_and_preserves_modifier_spans() {
    let (_, output) =
        parse_text("fn inspect(ref dog: Dog, mut ref other: Dog, count: i64) -> unit {}");

    assert!(output.diagnostics.is_empty());
    assert_eq!(
        dump_ast(&output.ast),
        concat!(
            "CompilationUnit @0..67\n",
            "  Function @0..67\n",
            "    Name \"inspect\" @3..10\n",
            "    Parameters\n",
            "      Parameter @11..23\n",
            "        Binding ReadOnlyAlias\n",
            "          Ref @11..14\n",
            "        Name \"dog\" @15..18\n",
            "        Type Named \"Dog\" @20..23\n",
            "      Parameter @25..43\n",
            "        Binding MutableAlias\n",
            "          Mut @25..28\n",
            "          Ref @29..32\n",
            "        Name \"other\" @33..38\n",
            "        Type Named \"Dog\" @40..43\n",
            "      Parameter @45..55\n",
            "        Binding Value\n",
            "        Name \"count\" @45..50\n",
            "        Type I64 @52..55\n",
            "    ReturnType\n",
            "      Type Unit @60..64\n",
            "    Block @65..67\n",
        )
    );
}

#[test]
fn malformed_alias_modifiers_report_focused_errors_and_recover() {
    let cases = [
        (
            "ref mut value: Thing",
            "`mut` must precede `ref` in a mutable alias parameter",
        ),
        ("ref ref value: Thing", "repeated alias parameter modifier"),
        (
            "mut mut ref value: Thing",
            "expected `ref` after `mut` in a parameter",
        ),
        (
            "mut value: Thing",
            "expected `ref` after `mut` in a parameter",
        ),
        ("ref : Thing", "expected a parameter name"),
        (
            "ref value: )",
            "expected an object view or inline optional alias parameter type",
        ),
    ];

    for (parameters, expected_message) in cases {
        let (_, output) = parse_text(&format!(
            "fn broken({parameters}) -> unit {{}} fn main() -> i64 {{ return 0; }}"
        ));

        assert!(output.has_errors(), "expected `{parameters}` to be invalid");
        assert_eq!(output.ast.declarations.len(), 1);
        assert_eq!(function(&output.ast, 0).name.text, "main");
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected_message)));
    }
}

#[test]
fn parameter_recovery_recognizes_aliases_after_a_missing_comma() {
    let (_, output) = parse_text(concat!(
        "fn broken(value: i64 ref item: Thing, later: i64) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.has_errors());
    let broken = function(&output.ast, 0);
    assert_eq!(broken.parameters.len(), 3);
    assert!(matches!(
        broken.parameters[1].binding_mode,
        ParameterBindingMode::ReadOnlyAlias { .. }
    ));
    assert_eq!(function(&output.ast, 1).name.text, "main");
    assert!(output.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("expected `,` between parameters")));
}

#[test]
fn grouped_call_arguments_remain_source_shaped_beside_alias_signatures() {
    let (_, output) = parse_text(concat!(
        "fn inspect(ref item: Thing) -> unit {}\n",
        "fn caller() -> unit { inspect((item)); }\n",
    ));

    assert!(output.diagnostics.is_empty());
    let Statement::Expression(statement) = &function(&output.ast, 1).body.statements[0] else {
        panic!("expected call statement");
    };
    let Expression::Call(call) = &statement.expression else {
        panic!("expected call expression");
    };
    let CallArguments::Ordinary(arguments) = &call.arguments else {
        panic!("expected ordinary call arguments");
    };
    assert!(matches!(arguments[0], Expression::Grouped(_)));
}

#[test]
fn alias_modifiers_outside_parameters_recover_to_later_syntax() {
    let (_, output) = parse_text(concat!(
        "ref top_level;\n",
        "class Recovered {\n",
        "    ref field: Recovered;\n",
        "    mut ref other: Recovered;\n",
        "    init() {}\n",
        "    fn good() -> unit {}\n",
        "}\n",
        "fn main() -> i64 { ref local: Recovered; return 0; }\n",
    ));

    assert!(output.has_errors());
    assert_eq!(output.ast.declarations.len(), 2);
    assert_eq!(class(&output.ast, 0).members.len(), 2);
    let main = function(&output.ast, 1);
    assert_eq!(main.body.statements.len(), 1);
    assert!(matches!(main.body.statements[0], Statement::Return(_)));
    assert!(output.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("alias binding modifiers are valid only on parameters")));
    assert!(output.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("alias bindings are not valid class fields")));
    assert!(output.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("local alias bindings are not supported")));
}

fn assert_read_only_alias(
    sources: &SourceDatabase,
    parameter: &Parameter,
    expected_name: &str,
    expected_type: &str,
) {
    let ParameterBindingMode::ReadOnlyAlias { ref_span } = parameter.binding_mode else {
        panic!("expected read-only alias parameter");
    };
    assert_eq!(source_text(sources, ref_span), "ref");
    assert_parameter_shape(sources, parameter, expected_name, expected_type);
    assert!(source_text(sources, parameter.span).starts_with("ref "));
}

fn assert_mutable_alias(
    sources: &SourceDatabase,
    parameter: &Parameter,
    expected_name: &str,
    expected_type: &str,
) {
    let ParameterBindingMode::MutableAlias { mut_span, ref_span } = parameter.binding_mode else {
        panic!("expected mutable alias parameter");
    };
    assert_eq!(source_text(sources, mut_span), "mut");
    assert_eq!(source_text(sources, ref_span), "ref");
    assert_parameter_shape(sources, parameter, expected_name, expected_type);
    assert!(source_text(sources, parameter.span).starts_with("mut ref "));
}

fn assert_parameter_shape(
    sources: &SourceDatabase,
    parameter: &Parameter,
    expected_name: &str,
    expected_type: &str,
) {
    assert_eq!(parameter.name.text, expected_name);
    assert_eq!(source_text(sources, parameter.name.span), expected_name);
    let TypeKind::Named(name) = &parameter.type_syntax.kind else {
        panic!("expected named alias parameter type");
    };
    assert_eq!(name.text, expected_type);
    assert_eq!(source_text(sources, name.span), expected_type);
}
