use super::*;
use crate::{
    hir::{
        dump_hir, HirBinaryOperation, HirExpression, HirExpressionKind, HirFunctionDefinition,
        HirStatement, Type,
    },
    lexer::lex,
    resolve::resolve,
    source::SourceDatabase,
    syntax::parse,
};

fn check_text(text: &str) -> TypeCheckOutput {
    let resolved = resolve_text(text);
    type_check(&resolved)
}

fn resolve_text(text: &str) -> crate::resolve::ResolvedProgram {
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
    let resolved = resolve(&parsed.ast);
    assert!(
        resolved.diagnostics.is_empty(),
        "test source must resolve cleanly"
    );
    resolved.program
}

fn returned_expression(function: &HirFunctionDefinition) -> &HirExpression {
    let HirStatement::Return(statement) = function.body.statements.last().unwrap() else {
        panic!("expected final return statement");
    };
    statement.value.as_ref().expect("expected a return value")
}

fn assert_expression_is_fully_typed(expression: &HirExpression) {
    assert_eq!(expression.ty, Type::I64);
    match &expression.kind {
        HirExpressionKind::Unary { operand, .. } | HirExpressionKind::Grouped(operand) => {
            assert_expression_is_fully_typed(operand)
        }
        HirExpressionKind::Binary { left, right, .. } => {
            assert_expression_is_fully_typed(left);
            assert_expression_is_fully_typed(right);
        }
        HirExpressionKind::DirectCall { arguments, .. } => {
            for argument in arguments {
                assert_expression_is_fully_typed(argument);
            }
        }
        HirExpressionKind::Binding(_) | HirExpressionKind::Integer(_) => {}
    }
}

#[test]
fn checks_the_demonstration_program_into_fully_typed_hir() {
    let output = check_text(concat!(
        "fn twice(value: i64) -> i64 { return value * 2; }\n",
        "fn main() -> i64 {\n",
        "  var result: i64 = twice(20);\n",
        "  return result + 2;\n",
        "}\n",
    ));
    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    assert_eq!(hir.entry_function.index(), 1);
    assert_eq!(hir.declarations.len(), 2);
    assert_eq!(hir.definitions.len(), 2);

    for declaration in hir.declarations.iter() {
        assert_eq!(declaration.return_type, Type::I64);
        for parameter in &declaration.parameters {
            assert_eq!(parameter.ty, Type::I64);
        }
        let definition = hir.definitions.get(declaration.id).unwrap();
        for local in &definition.locals {
            assert_eq!(local.ty, Type::I64);
        }
        for statement in &definition.body.statements {
            match statement {
                HirStatement::Local(local) => assert_expression_is_fully_typed(&local.initializer),
                HirStatement::Return(statement) => {
                    if let Some(value) = &statement.value {
                        assert_expression_is_fully_typed(value);
                    }
                }
                HirStatement::Call(statement) => assert_expression_is_fully_typed(&statement.call),
                HirStatement::Block(_) => {}
            }
        }
    }

    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::Local(local) = &main.body.statements[0] else {
        panic!("expected local declaration");
    };
    let HirExpressionKind::DirectCall {
        function,
        arguments,
    } = &local.initializer.kind
    else {
        panic!("expected typed direct call");
    };
    assert_eq!(function.index(), 0);
    assert_eq!(arguments.len(), 1);

    let HirExpressionKind::Binary { operation, .. } = &returned_expression(main).kind else {
        panic!("expected typed addition");
    };
    assert_eq!(*operation, HirBinaryOperation::AddI64);
}

#[test]
fn checks_unit_functions_returns_and_call_statements() {
    let output = check_text(concat!(
        "fn explicit() -> unit { return; }\n",
        "fn implicit() -> unit {}\n",
        "fn main() -> i64 { (explicit()); implicit(); return 0; }\n",
    ));

    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    assert_eq!(
        hir.declarations
            .get(crate::resolve::FunctionId::new(0))
            .unwrap()
            .return_type,
        Type::Unit
    );
    assert_eq!(
        hir.declarations
            .get(crate::resolve::FunctionId::new(1))
            .unwrap()
            .return_type,
        Type::Unit
    );
    let explicit = hir
        .definitions
        .get(crate::resolve::FunctionId::new(0))
        .unwrap();
    let HirStatement::Return(statement) = &explicit.body.statements[0] else {
        panic!("expected explicit unit return");
    };
    assert!(statement.value.is_none());
    let main = hir.definitions.get(hir.entry_function).unwrap();
    assert!(main.body.statements[..2].iter().all(|statement| {
        matches!(
            statement,
            HirStatement::Call(call) if call.call.ty == Type::Unit
        )
    }));
    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains("ReturnType unit"));
    assert!(dump.contains("CallStatement"));
    assert!(dump.contains("DirectCall f0 : unit"));
}

#[test]
fn checks_external_calls_from_bodyless_signatures() {
    let output = check_text(concat!(
        "extern fn read_value(seed: i64) -> i64;\n",
        "extern fn emit(value: i64) -> unit;\n",
        "fn main() -> i64 { var value: i64 = read_value(7); emit(value); return value; }\n",
    ));

    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    for id in [
        crate::resolve::FunctionId::new(0),
        crate::resolve::FunctionId::new(1),
    ] {
        assert!(matches!(
            hir.declarations.get(id).unwrap().linkage,
            crate::hir::HirFunctionLinkage::External { .. }
        ));
        assert!(hir.definitions.get(id).is_none());
    }
    let dump = dump_hir(&hir);
    assert!(dump.contains("Declaration f0 \"read_value\" external \"read_value\""));
    assert!(dump.contains("Declaration f1 \"emit\" external \"emit\""));
    assert!(!dump.contains("Definition f0"));
    assert!(!dump.contains("Definition f1"));
}

#[test]
fn rejects_an_external_main_even_with_the_entry_signature() {
    let output = check_text("extern fn main() -> i64;");

    assert!(output.hir.is_none());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, INVALID_ENTRY_POINT);
    assert!(diagnostic.message.contains("fn main() -> i64"));
    assert!(diagnostic
        .labels
        .iter()
        .any(|label| label.message.contains("cannot be the entry point")));
}

#[test]
fn rejects_external_signatures_outside_the_restricted_abi_profile() {
    let mut resolved = resolve_text(concat!(
        "extern fn emit(value: i64) -> unit;\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    resolved.declarations.entries_mut_for_test()[0].parameters[0]
        .type_syntax
        .kind = crate::resolve::ResolvedTypeKind::Unit;

    let output = type_check(&resolved);

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_EXTERNAL_DECLARATION
            && diagnostic.message.contains("unsupported signature")
    }));
}

#[test]
fn diagnoses_invalid_unit_and_value_return_forms() {
    for (source, message) in [
        (
            "fn bad() -> unit { return 1; } fn main() -> i64 { return 0; }",
            "cannot return a value",
        ),
        ("fn main() -> i64 { return; }", "must return a value"),
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == INVALID_RETURN && diagnostic.message.contains(message)
        }));
    }
}

#[test]
fn rejects_unit_calls_in_value_contexts() {
    let output = check_text(concat!(
        "fn notify() -> unit {}\n",
        "fn main() -> i64 { return notify(); }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == TYPE_MISMATCH
            && diagnostic
                .message
                .contains("type `unit` but `i64` is required")
    }));
}

#[test]
fn rejects_discarded_i64_calls_and_non_call_expression_statements() {
    for (source, message) in [
        (
            "fn value() -> i64 { return 1; } fn main() -> i64 { value(); return 0; }",
            "returning `unit`",
        ),
        (
            "fn main() -> i64 { 1 + 2; return 0; }",
            "only function calls",
        ),
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == INVALID_CALL_STATEMENT && diagnostic.message.contains(message)
        }));
    }
}

#[test]
fn main_must_remain_i64_returning() {
    let output = check_text("fn main() -> unit {}");

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_ENTRY_POINT && diagnostic.message.contains("fn main() -> i64")
    }));
}

#[test]
fn missing_entry_point_prevents_hir_construction() {
    let output = check_text("fn helper() -> i64 { return 0; }");

    assert!(output.hir.is_none());
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        MISSING_ENTRY_POINT
    );
}

#[test]
fn entry_point_must_have_the_exact_first_slice_signature() {
    let output = check_text("fn main(value: i64) -> i64 { return value; }");

    assert!(output.hir.is_none());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, INVALID_ENTRY_POINT);
    assert!(diagnostic.message.contains("fn main() -> i64"));
}

#[test]
fn every_i64_function_must_return_a_value() {
    let output = check_text("fn main() -> i64 { var value: i64 = 0; }");

    assert!(output.hir.is_none());
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        MISSING_RETURN
    );
}

#[test]
fn nested_unconditional_block_can_supply_the_return() {
    let output = check_text("fn main() -> i64 { { return 7; } }");

    assert!(!output.has_errors());
    assert!(output.hir.is_some());
}

#[test]
fn direct_call_arity_is_checked_against_the_resolved_target() {
    let output = check_text(concat!(
        "fn one(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { return one(); }\n",
    ));

    assert!(output.hir.is_none());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, WRONG_ARGUMENT_COUNT);
    assert_eq!(diagnostic.labels.len(), 2);
    assert!(diagnostic
        .message
        .contains("expects 1 argument but received 0"));
}

#[test]
fn positive_i64_maximum_is_accepted() {
    let output = check_text("fn main() -> i64 { return 9223372036854775807; }");
    let hir = output.hir.unwrap();

    assert!(matches!(
        returned_expression(hir.definitions.get(hir.entry_function).unwrap()).kind,
        HirExpressionKind::Integer(i64::MAX)
    ));
}

#[test]
fn unary_minus_admits_the_i64_minimum_boundary() {
    let output = check_text("fn main() -> i64 { return -9223372036854775808; }");
    let hir = output.hir.unwrap();
    let expression = returned_expression(hir.definitions.get(hir.entry_function).unwrap());

    assert_eq!(expression.ty, Type::I64);
    assert!(matches!(
        expression.kind,
        HirExpressionKind::Integer(i64::MIN)
    ));
}

#[test]
fn grouping_does_not_break_the_i64_minimum_boundary() {
    let output = check_text("fn main() -> i64 { return -(9223372036854775808); }");

    assert!(output.hir.is_some());
    assert!(output.diagnostics.is_empty());
}

#[test]
fn positive_and_negative_out_of_range_literals_are_diagnosed() {
    for source in [
        "fn main() -> i64 { return 9223372036854775808; }",
        "fn main() -> i64 { return -9223372036854775809; }",
        "fn main() -> i64 { return 999999999999999999999999999999999999999; }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(
            output.diagnostics.iter().next().unwrap().code,
            INTEGER_LITERAL_OUT_OF_RANGE
        );
    }
}

#[test]
fn hir_dump_is_deterministic_and_records_types_and_operations() {
    let output = check_text("fn main() -> i64 { return 1 + -2; }");
    let hir = output.hir.unwrap();

    assert_eq!(
        dump_hir(&hir),
        concat!(
            "HirProgram @0..35\n",
            "  Entry f0\n",
            "  Declarations\n",
            "    Declaration f0 \"main\" internal @0..35\n",
            "      Parameters\n",
            "      ReturnType i64\n",
            "  Definitions\n",
            "    Definition f0 @0..35\n",
            "      Locals\n",
            "      Block @17..35\n",
            "        Return @19..33\n",
            "          Binary AddI64 : i64 @26..32\n",
            "            Integer 1 : i64 @26..27\n",
            "            Unary NegateI64 : i64 @30..32\n",
            "              Integer 2 : i64 @31..32\n",
        )
    );
}
