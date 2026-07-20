use super::*;

#[test]
fn preserves_boolean_types_literals_and_bindings() {
    let output = resolve_text(concat!(
        "fn identity(value: bool) -> bool { return value; }\n",
        "fn main() -> i64 { var flag: bool = true; identity(flag); return 0; }\n",
    ));

    assert!(!output.has_errors());
    let identity = output.program.declarations.get(FunctionId::new(0)).unwrap();
    assert_eq!(
        identity.parameters[0].type_syntax.kind,
        ResolvedTypeKind::Bool
    );
    assert_eq!(identity.return_type.kind, ResolvedTypeKind::Bool);
    let main = output.program.definitions.get(FunctionId::new(1)).unwrap();
    assert_eq!(main.locals[0].type_syntax.kind, ResolvedTypeKind::Bool);
    assert!(matches!(
        local_initializer(&main.body.statements[0]),
        ResolvedExpression::Boolean(ResolvedBooleanExpr { value: true, .. })
    ));

    let dump = dump_resolved(&output.program);
    assert!(dump.contains("Type Bool"));
    assert!(dump.contains("Boolean true"));
}

#[test]
fn nested_blocks_shadow_and_then_restore_outer_bindings() {
    let output = resolve_text(concat!(
        "fn main(value: i64) -> i64 {\n",
        "  var result: i64 = value;\n",
        "  { var result: i64 = 2; return result; }\n",
        "  return result;\n",
        "}\n",
    ));
    assert!(!output.has_errors());
    let declaration = output.program.declarations.iter().next().unwrap();
    let definition = output.program.definitions.get(declaration.id).unwrap();
    assert_eq!(definition.locals.len(), 2);

    let ResolvedExpression::Binding(initial_value) =
        local_initializer(&definition.body.statements[0])
    else {
        panic!("outer initializer must resolve to the parameter");
    };
    assert_eq!(
        initial_value.binding,
        BindingId::Parameter(declaration.parameters[0].id)
    );

    let ResolvedStatement::Block(nested) = &definition.body.statements[1] else {
        panic!("expected nested block");
    };
    let ResolvedExpression::Binding(nested_value) = return_value(&nested.statements[1]) else {
        panic!("nested return must resolve to a local");
    };
    assert_eq!(
        nested_value.binding,
        BindingId::Local(definition.locals[1].id)
    );

    let ResolvedExpression::Binding(outer_value) = return_value(&definition.body.statements[2])
    else {
        panic!("outer return must resolve to a local");
    };
    assert_eq!(
        outer_value.binding,
        BindingId::Local(definition.locals[0].id)
    );
}

#[test]
fn local_is_not_visible_in_its_own_initializer_but_is_visible_afterward() {
    let output = resolve_text(concat!(
        "fn main() -> i64 {\n",
        "  var value: i64 = value;\n",
        "  return value;\n",
        "}\n",
    ));
    let declaration = output.program.declarations.iter().next().unwrap();
    let definition = output.program.definitions.get(declaration.id).unwrap();

    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(output.diagnostics.iter().next().unwrap().code, UNKNOWN_NAME);
    assert_eq!(definition.locals.len(), 1);
    assert_eq!(definition.body.statements.len(), 1);
    let ResolvedExpression::Binding(binding) = return_value(&definition.body.statements[0]) else {
        panic!("later use must resolve to the local");
    };
    assert_eq!(binding.binding, BindingId::Local(definition.locals[0].id));
}

#[test]
fn local_binding_shadows_a_function_as_a_call_target() {
    let output = resolve_text(concat!(
        "fn target() -> i64 { return 1; }\n",
        "fn main() -> i64 {\n",
        "  var target: i64 = 2;\n",
        "  return target();\n",
        "}\n",
    ));

    assert_eq!(output.diagnostics.len(), 1);
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, INVALID_CALL_TARGET);
    assert_eq!(diagnostic.labels.len(), 2);
}
