use super::*;

#[test]
fn nan_and_infinity_spellings_are_rejected_as_single_unknown_names() {
    for spelling in ["NaN", "inf"] {
        let output = resolve_text(&format!(
            "fn value() -> f64 {{ return {spelling}; }} fn main() -> i64 {{ return 0; }}"
        ));

        assert_eq!(output.diagnostics.len(), 1, "{spelling}");
        assert!(output
            .diagnostics
            .iter()
            .next()
            .unwrap()
            .message
            .contains(spelling));
    }
}

#[test]
fn diagnoses_duplicate_parameters_and_outer_block_locals() {
    let output = resolve_text(concat!(
        "fn main(value: i64, value: i64) -> i64 {\n",
        "  var value: i64 = 1;\n",
        "  return value;\n",
        "}\n",
    ));
    let declaration = output.program.declarations.iter().next().unwrap();
    let definition = output.program.definitions.get(declaration.id).unwrap();

    assert_eq!(output.diagnostics.len(), 2);
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == DUPLICATE_BINDING));
    assert_eq!(declaration.parameters.len(), 1);
    assert!(definition.locals.is_empty());
    let ResolvedExpression::Binding(binding) = return_value(&definition.body.statements[0]) else {
        panic!("return must resolve to the first parameter");
    };
    assert_eq!(
        binding.binding,
        BindingId::Parameter(declaration.parameters[0].id)
    );
}

#[test]
fn reports_multiple_unknown_names_without_stopping() {
    let output = resolve_text("fn main() -> i64 { var value: i64 = first; return second; }");

    assert_eq!(output.diagnostics.len(), 2);
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == UNKNOWN_NAME));
}

#[test]
fn rejects_non_identifier_and_unknown_call_targets() {
    let output = resolve_text(concat!(
        "fn target() -> i64 { return 1; }\n",
        "fn main() -> i64 {\n",
        "  var one: i64 = (target)();\n",
        "  return missing();\n",
        "}\n",
    ));

    let codes: Vec<_> = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(codes, vec![INVALID_CALL_TARGET, UNKNOWN_NAME]);
}

#[test]
fn function_name_without_a_call_is_not_a_value() {
    let output = resolve_text(concat!(
        "fn target() -> i64 { return 1; }\n",
        "fn main() -> i64 { return target; }\n",
    ));

    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        FUNCTION_USED_AS_VALUE
    );
}
