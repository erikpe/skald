use super::*;

#[test]
fn collects_functions_before_resolving_forward_calls() {
    let output = resolve_text(concat!(
        "fn main() -> i64 { return twice(21); }\n",
        "fn twice(value: i64) -> i64 { return value * 2; }\n",
    ));

    assert!(!output.has_errors());
    assert_eq!(output.program.declarations.len(), 2);
    assert_eq!(output.program.definitions.len(), 2);
    assert_eq!(output.program.entry_function.unwrap().index(), 0);

    let main = output
        .program
        .definitions
        .get(output.program.entry_function.unwrap())
        .unwrap();
    let ResolvedExpression::DirectCall(call) = return_value(&main.body.statements[0]) else {
        panic!("expected a resolved direct call");
    };
    assert_eq!(call.function.index(), 1);
    assert_eq!(call.arguments.len(), 1);
}

#[test]
fn external_declarations_share_the_callable_namespace_and_have_no_body() {
    let output = resolve_text(concat!(
        "extern fn emit(value: i64) -> unit;\n",
        "fn main() -> i64 { emit(7); return 0; }\n",
    ));

    assert!(!output.has_errors());
    let external = output.program.declarations.get(FunctionId::new(0)).unwrap();
    assert!(matches!(
        &external.linkage,
        ResolvedFunctionLinkage::External { symbol } if symbol == "emit"
    ));
    assert!(output.program.definitions.get(external.id).is_none());
    let main = output
        .program
        .definitions
        .get(output.program.entry_function.unwrap())
        .unwrap();
    let ResolvedStatement::Expression(statement) = &main.body.statements[0] else {
        panic!("expected call statement");
    };
    let ResolvedExpression::DirectCall(call) = &statement.expression else {
        panic!("expected resolved direct call");
    };
    assert_eq!(call.function, external.id);

    let dump = dump_resolved(&output.program);
    assert!(dump.contains("Declaration f0 \"emit\" external \"emit\""));
    assert!(!dump.contains("Definition f0"));
}

#[test]
fn diagnoses_duplicate_names_across_all_external_and_defined_combinations() {
    for source in [
        "extern fn same() -> unit; extern fn same() -> unit; fn main() -> i64 { return 0; }",
        "extern fn same() -> unit; fn same() -> unit {} fn main() -> i64 { return 0; }",
        "fn same() -> unit {} extern fn same() -> unit; fn main() -> i64 { return 0; }",
    ] {
        let output = resolve_text(source);
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(
            output.diagnostics.iter().next().unwrap().code,
            DUPLICATE_FUNCTION
        );
        assert_eq!(output.program.declarations.len(), 2);
    }
}

#[test]
fn diagnoses_duplicate_external_parameter_names() {
    let output = resolve_text(concat!(
        "extern fn emit(value: i64, value: i64) -> unit;\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        DUPLICATE_BINDING
    );
}

#[test]
fn assigns_dense_owner_qualified_ids_in_source_order() {
    let output = resolve_text(concat!(
        "fn add(left: i64, right: i64) -> i64 {\n",
        "  var first: i64 = left;\n",
        "  { var second: i64 = right; return second; }\n",
        "  return first;\n",
        "}\n",
    ));
    let declaration = output.program.declarations.iter().next().unwrap();
    let definition = output.program.definitions.get(declaration.id).unwrap();

    assert_eq!(declaration.id.index(), 0);
    assert_eq!(declaration.parameters[0].id.index(), 0);
    assert_eq!(declaration.parameters[1].id.index(), 1);
    assert_eq!(definition.locals[0].id.index(), 0);
    assert_eq!(definition.locals[1].id.index(), 1);
    assert_eq!(definition.locals[1].id.function(), declaration.id);
    assert_eq!(
        declaration
            .parameter(declaration.parameters[1].id)
            .unwrap()
            .name,
        "right"
    );
    assert_eq!(
        definition.local(definition.locals[0].id).unwrap().name,
        "first"
    );
}

#[test]
fn diagnoses_duplicate_functions_and_keeps_the_first() {
    let output = resolve_text(concat!(
        "fn same() -> i64 { return 1; }\n",
        "fn same() -> i64 { return 2; }\n",
        "fn other() -> i64 { return same(); }\n",
    ));

    assert!(output.has_errors());
    assert_eq!(output.program.declarations.len(), 2);
    assert_eq!(
        output
            .program
            .declarations
            .iter()
            .nth(1)
            .unwrap()
            .id
            .index(),
        1
    );
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, DUPLICATE_FUNCTION);
    assert_eq!(diagnostic.labels.len(), 2);
}
