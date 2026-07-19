use super::*;
use crate::{
    lexer::lex,
    source::SourceDatabase,
    syntax::{self, parse, Statement},
};

use crate::resolve::dump_resolved;

fn resolve_text(text: &str) -> ResolveOutput {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("test.ska", text);
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty(), "test source must lex cleanly");
    let parsed = parse(source, &lexed.tokens);
    assert!(
        parsed.diagnostics.is_empty(),
        "test source must parse cleanly"
    );
    resolve(&parsed.ast)
}

fn local_initializer(statement: &ResolvedStatement) -> &ResolvedExpression {
    let ResolvedStatement::Local(local) = statement else {
        panic!("expected local declaration");
    };
    &local.initializer
}

fn return_value(statement: &ResolvedStatement) -> &ResolvedExpression {
    let ResolvedStatement::Return(statement) = statement else {
        panic!("expected return statement");
    };
    &statement.value
}

#[test]
fn collects_functions_before_resolving_forward_calls() {
    let output = resolve_text(concat!(
        "fn main() -> i64 { return twice(21); }\n",
        "fn twice(value: i64) -> i64 { return value * 2; }\n",
    ));

    assert!(!output.has_errors());
    assert_eq!(output.program.functions.len(), 2);
    assert_eq!(output.program.entry_function.unwrap().index(), 0);

    let main = output.program.functions.iter().next().unwrap();
    let ResolvedExpression::DirectCall(call) = return_value(&main.body.statements[0]) else {
        panic!("expected a resolved direct call");
    };
    assert_eq!(call.function.index(), 1);
    assert_eq!(call.arguments.len(), 1);
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
    let function = output.program.functions.iter().next().unwrap();

    assert_eq!(function.id.index(), 0);
    assert_eq!(function.parameters[0].id.index(), 0);
    assert_eq!(function.parameters[1].id.index(), 1);
    assert_eq!(function.locals[0].id.index(), 0);
    assert_eq!(function.locals[1].id.index(), 1);
    assert_eq!(function.locals[1].id.function(), function.id);
    assert_eq!(
        function.parameter(function.parameters[1].id).unwrap().name,
        "right"
    );
    assert_eq!(function.local(function.locals[0].id).unwrap().name, "first");
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
    let function = output.program.functions.iter().next().unwrap();
    assert_eq!(function.locals.len(), 2);

    let ResolvedExpression::Binding(initial_value) =
        local_initializer(&function.body.statements[0])
    else {
        panic!("outer initializer must resolve to the parameter");
    };
    assert_eq!(
        initial_value.binding,
        BindingId::Parameter(function.parameters[0].id)
    );

    let ResolvedStatement::Block(nested) = &function.body.statements[1] else {
        panic!("expected nested block");
    };
    let ResolvedExpression::Binding(nested_value) = return_value(&nested.statements[1]) else {
        panic!("nested return must resolve to a local");
    };
    assert_eq!(
        nested_value.binding,
        BindingId::Local(function.locals[1].id)
    );

    let ResolvedExpression::Binding(outer_value) = return_value(&function.body.statements[2])
    else {
        panic!("outer return must resolve to a local");
    };
    assert_eq!(outer_value.binding, BindingId::Local(function.locals[0].id));
}

#[test]
fn diagnoses_duplicate_functions_and_keeps_the_first() {
    let output = resolve_text(concat!(
        "fn same() -> i64 { return 1; }\n",
        "fn same() -> i64 { return 2; }\n",
        "fn other() -> i64 { return same(); }\n",
    ));

    assert!(output.has_errors());
    assert_eq!(output.program.functions.len(), 2);
    assert_eq!(
        output.program.functions.iter().nth(1).unwrap().id.index(),
        1
    );
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, DUPLICATE_FUNCTION);
    assert_eq!(diagnostic.labels.len(), 2);
}

#[test]
fn diagnoses_duplicate_parameters_and_outer_block_locals() {
    let output = resolve_text(concat!(
        "fn main(value: i64, value: i64) -> i64 {\n",
        "  var value: i64 = 1;\n",
        "  return value;\n",
        "}\n",
    ));
    let function = output.program.functions.iter().next().unwrap();

    assert_eq!(output.diagnostics.len(), 2);
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == DUPLICATE_BINDING));
    assert_eq!(function.parameters.len(), 1);
    assert!(function.locals.is_empty());
    let ResolvedExpression::Binding(binding) = return_value(&function.body.statements[0]) else {
        panic!("return must resolve to the first parameter");
    };
    assert_eq!(
        binding.binding,
        BindingId::Parameter(function.parameters[0].id)
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
    let function = output.program.functions.iter().next().unwrap();

    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(output.diagnostics.iter().next().unwrap().code, UNKNOWN_NAME);
    assert_eq!(function.locals.len(), 1);
    assert_eq!(function.body.statements.len(), 1);
    let ResolvedExpression::Binding(binding) = return_value(&function.body.statements[0]) else {
        panic!("later use must resolve to the local");
    };
    assert_eq!(binding.binding, BindingId::Local(function.locals[0].id));
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

#[test]
fn resolved_dump_is_deterministic_and_exposes_only_ids_at_uses() {
    let output = resolve_text("fn main(value: i64) -> i64 { return value; }");

    assert_eq!(
        dump_resolved(&output.program),
        concat!(
            "ResolvedProgram @0..44\n",
            "  Entry f0\n",
            "  Functions\n",
            "    Function f0 \"main\" @0..44\n",
            "      Parameters\n",
            "        Parameter f0:p0 \"value\" @8..18\n",
            "          Type I64 @15..18\n",
            "      ReturnType\n",
            "        Type I64 @23..26\n",
            "      Locals\n",
            "      Block @27..44\n",
            "        Return @29..42\n",
            "          Binding f0:p0 @36..41\n",
        )
    );
}

#[test]
fn parsed_source_ast_still_contains_names_before_resolution() {
    // This compile-time shape check documents the phase boundary: M3 reads
    // source names, while resolved uses are represented only by BindingId
    // or FunctionId.
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("test.ska", "fn main() -> i64 { return name; }");
    let source = sources.get(source_id).unwrap();
    let tokens = lex(source).tokens;
    let ast = parse(source, &tokens).ast;
    let Statement::Return(statement) = &ast.functions[0].body.statements[0] else {
        panic!("expected return");
    };
    assert!(matches!(statement.value, syntax::Expression::Identifier(_)));
}
