use super::*;
use crate::{
    hir::{
        dump_hir, BlockFlow, HirBinaryOperation, HirExpression, HirExpressionKind,
        HirFunctionDefinition, HirStatement, Type,
    },
    identity::FunctionId,
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
        HirExpressionKind::Binding(_)
        | HirExpressionKind::I64(_)
        | HirExpressionKind::U64(_)
        | HirExpressionKind::U8(_)
        | HirExpressionKind::F64Bits(_)
        | HirExpressionKind::Boolean(_) => {}
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
                HirStatement::Conditional(_) => {}
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
            .get(FunctionId::new(0))
            .unwrap()
            .return_type,
        Type::Unit
    );
    assert_eq!(
        hir.declarations
            .get(FunctionId::new(1))
            .unwrap()
            .return_type,
        Type::Unit
    );
    let explicit = hir.definitions.get(FunctionId::new(0)).unwrap();
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
fn checks_boolean_values_without_conflating_them_with_i64() {
    let output = check_text(concat!(
        "extern fn external_flag(value: bool) -> bool;\n",
        "fn identity(value: bool) -> bool { var result: bool = value; return result; }\n",
        "fn main() -> i64 { var flag: bool = external_flag(true); var echoed: bool = identity(flag); return 0; }\n",
    ));

    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    let external = hir.declarations.get(FunctionId::new(0)).unwrap();
    assert_eq!(external.parameters[0].ty, Type::Bool);
    assert_eq!(external.return_type, Type::Bool);
    let identity = hir.declarations.get(FunctionId::new(1)).unwrap();
    assert_eq!(identity.parameters[0].ty, Type::Bool);
    assert_eq!(identity.return_type, Type::Bool);
    let main = hir.definitions.get(hir.entry_function).unwrap();
    assert!(main.locals.iter().all(|local| local.ty == Type::Bool));

    let dump = dump_hir(&hir);
    assert!(dump.contains("Boolean true : bool"));
    assert!(dump.contains("DirectCall f0 : bool"));
}

#[test]
fn rejects_boolean_i64_mismatches_and_boolean_arithmetic() {
    for source in [
        "fn flag() -> bool { return 1; } fn main() -> i64 { return 0; }",
        "fn flag(value: bool) -> bool { return value; } fn main() -> i64 { flag(1); return 0; }",
        "fn main() -> i64 { var flag: bool = true; return flag + 1; }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));
    }
}

#[test]
fn boolean_functions_require_a_boolean_return_value() {
    let output = check_text("fn flag() -> bool {} fn main() -> i64 { return 0; }");

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == MISSING_RETURN
            && diagnostic
                .notes
                .iter()
                .any(|note| note.contains("return type `bool`"))
    }));
}

#[test]
fn checks_typed_conditionals_and_preserves_ordered_arms_in_hir() {
    let output = check_text(concat!(
        "fn choose(first: bool, second: bool) -> i64 {\n",
        "  if (first) { return 1; }\n",
        "  elif (second) { return 2; }\n",
        "  else { return 3; }\n",
        "}\n",
        "fn main() -> i64 { return choose(false, true); }\n",
    ));

    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    let choose = hir.definitions.get(FunctionId::new(0)).unwrap();
    let HirStatement::Conditional(conditional) = &choose.body.statements[0] else {
        panic!("expected typed conditional");
    };
    assert_eq!(conditional.arms.len(), 2);
    assert!(conditional
        .arms
        .iter()
        .all(|arm| arm.condition.ty == Type::Bool));
    assert!(conditional.else_block.is_some());
    let dump = dump_hir(&hir);
    assert_eq!(dump.matches("ElifArm").count(), 1);
    assert!(dump.contains("Binding f0:p0 : bool"));
    assert!(dump.contains("Binding f0:p1 : bool"));
}

#[test]
fn rejects_non_boolean_conditional_conditions() {
    let output = check_text("fn main() -> i64 { if (1) { return 1; } else { return 0; } }");

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == TYPE_MISMATCH
            && diagnostic
                .message
                .contains("conditional condition has type `i64` but `bool` is required")
    }));
}

#[test]
fn exhaustive_all_returning_conditionals_satisfy_definite_return() {
    for source in [
        "fn main() -> i64 { if (true) { return 1; } else { return 2; } }",
        "fn main() -> i64 { if (false) { return 1; } elif (true) { return 2; } else { return 3; } }",
        "fn flag(value: bool) -> bool { if (value) { return true; } else { return false; } } fn main() -> i64 { return 0; }",
    ] {
        let output = check_text(source);
        assert!(!output.has_errors(), "source should type-check: {source}");
    }
}

#[test]
fn non_exhaustive_or_fallthrough_conditionals_do_not_guarantee_return() {
    for source in [
        "fn main() -> i64 { if (true) { return 1; } }",
        "fn main() -> i64 { if (true) { return 1; } else {} }",
        "fn main() -> i64 { if (true) {} else { return 1; } }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == MISSING_RETURN));
    }
}

#[test]
fn a_return_after_a_non_exhaustive_conditional_remains_definite() {
    let output = check_text("fn main() -> i64 { if (false) { return 1; } return 2; }");

    assert!(!output.has_errors());
}

#[test]
fn typed_blocks_record_composed_flow_for_later_phases() {
    let output = check_text(concat!(
        "fn inspect(value: bool) -> unit {\n",
        "  if (value) { return; } else {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  { if (true) { return 1; } else { return 2; } }\n",
        "  return 3;\n",
        "}\n",
    ));
    assert!(!output.has_errors());
    let hir = output.hir.unwrap();

    let inspect = hir.definitions.get(FunctionId::new(0)).unwrap();
    assert_eq!(inspect.body.flow, BlockFlow::FallsThrough);
    let HirStatement::Conditional(conditional) = &inspect.body.statements[0] else {
        panic!("expected conditional");
    };
    assert_eq!(conditional.flow, BlockFlow::FallsThrough);
    assert_eq!(conditional.arms[0].body.flow, BlockFlow::Terminates);
    assert_eq!(
        conditional.else_block.as_ref().unwrap().flow,
        BlockFlow::FallsThrough
    );

    let main = hir.definitions.get(hir.entry_function).unwrap();
    assert_eq!(main.body.flow, BlockFlow::Terminates);
    let HirStatement::Block(nested) = &main.body.statements[0] else {
        panic!("expected nested block");
    };
    assert_eq!(nested.flow, BlockFlow::Terminates);
    let HirStatement::Conditional(conditional) = &nested.statements[0] else {
        panic!("expected nested conditional");
    };
    assert_eq!(conditional.flow, BlockFlow::Terminates);
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
    for id in [FunctionId::new(0), FunctionId::new(1)] {
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
        assert!(!output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == MISSING_RETURN));
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
    for source in [
        "fn main() -> unit {}",
        "fn main() -> bool { return false; }",
        "fn main() -> u64 { return 0u; }",
        "fn main() -> f64 { return 0.0; }",
    ] {
        let output = check_text(source);

        assert!(output.hir.is_none());
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == INVALID_ENTRY_POINT
                && diagnostic.message.contains("fn main() -> i64")
        }));
    }
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
        HirExpressionKind::I64(i64::MAX)
    ));
}

#[test]
fn checks_u64_literals_signatures_and_typed_arithmetic() {
    let output = check_text(concat!(
        "extern fn observe(value: u64) -> unit;\n",
        "fn calculate(left: u64, right: u64) -> u64 { return left + right * 2u - 1u; }\n",
        "fn maximum() -> u64 { return 18446744073709551615u; }\n",
        "fn main() -> i64 { var value: u64 = calculate(maximum(), 1u); observe(value); return 0; }\n",
    ));
    let hir = output.hir.expect("valid u64 program must produce HIR");
    let calculate = hir.definitions.get(FunctionId::new(1)).unwrap();
    let expression = returned_expression(calculate);

    assert_eq!(expression.ty, Type::U64);
    assert!(matches!(
        expression.kind,
        HirExpressionKind::Binary {
            operation: HirBinaryOperation::SubtractU64,
            ..
        }
    ));
    let maximum = hir.definitions.get(FunctionId::new(2)).unwrap();
    assert!(matches!(
        returned_expression(maximum).kind,
        HirExpressionKind::U64(u64::MAX)
    ));
}

#[test]
fn diagnoses_u64_literal_overflow() {
    let output = check_text(
        "fn too_large() -> u64 { return 18446744073709551616u; } fn main() -> i64 { return 0; }",
    );

    assert!(output.hir.is_none());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, U64_LITERAL_OUT_OF_RANGE);
    assert_eq!(
        diagnostic.message,
        "integer literal `18446744073709551616u` is out of range for `u64`"
    );
}

#[test]
fn rejects_implicit_i64_u64_conversions_and_unsigned_negation() {
    for (source, expected) in [
        (
            "fn value() -> u64 { return 1; } fn main() -> i64 { return 0; }",
            "return value has type `i64` but `u64` is required",
        ),
        (
            "fn value() -> i64 { return 1u; } fn main() -> i64 { return 0; }",
            "return value has type `u64` but `i64` is required",
        ),
        (
            "fn value() -> u64 { return 1u + 2; } fn main() -> i64 { return 0; }",
            "right arithmetic operand has type `i64` but `u64` is required",
        ),
        (
            "fn value() -> i64 { return 1 + 2u; } fn main() -> i64 { return 0; }",
            "right arithmetic operand has type `u64` but `i64` is required",
        ),
        (
            "fn main() -> i64 { var value: i64 = 1u; return 0; }",
            "local initializer has type `u64` but `i64` is required",
        ),
        (
            "fn take(value: u64) -> unit {} fn main() -> i64 { take(1); return 0; }",
            "call argument has type `i64` but `u64` is required",
        ),
        (
            "fn take(value: i64) -> unit {} fn main() -> i64 { take(1u); return 0; }",
            "call argument has type `u64` but `i64` is required",
        ),
        (
            "fn value() -> u64 { return -1u; } fn main() -> i64 { return 0; }",
            "unary negation operand has type `u64` but `i64` is required",
        ),
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == expected));
    }
}

#[test]
fn checks_u8_bounds_signatures_and_typed_arithmetic() {
    let output = check_text(concat!(
        "extern fn observe(value: u8) -> unit;\n",
        "fn calculate(left: u8, right: u8) -> u8 { return left + right * 2u8 - 1u8; }\n",
        "fn bounds() -> u8 { var zero: u8 = 0u8; return 255u8; }\n",
        "fn main() -> i64 { observe(calculate(bounds(), 1u8)); return 0; }\n",
    ));
    let hir = output.hir.expect("valid u8 program must produce HIR");
    let calculate = hir.definitions.get(FunctionId::new(1)).unwrap();
    let expression = returned_expression(calculate);

    assert_eq!(expression.ty, Type::U8);
    assert!(matches!(
        expression.kind,
        HirExpressionKind::Binary {
            operation: HirBinaryOperation::SubtractU8,
            ..
        }
    ));
    let bounds = hir.definitions.get(FunctionId::new(2)).unwrap();
    assert!(matches!(
        returned_expression(bounds).kind,
        HirExpressionKind::U8(u8::MAX)
    ));
}

#[test]
fn diagnoses_u8_literal_overflow() {
    let output =
        check_text("fn too_large() -> u8 { return 256u8; } fn main() -> i64 { return 0; }");

    assert!(output.hir.is_none());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, U8_LITERAL_OUT_OF_RANGE);
    assert_eq!(
        diagnostic.message,
        "integer literal `256u8` is out of range for `u8`"
    );
}

#[test]
fn rejects_implicit_u8_conversions_and_unsigned_negation() {
    for (source, expected) in [
        (
            "fn value() -> u8 { return 1; } fn main() -> i64 { return 0; }",
            "return value has type `i64` but `u8` is required",
        ),
        (
            "fn value() -> u64 { return 1u8; } fn main() -> i64 { return 0; }",
            "return value has type `u8` but `u64` is required",
        ),
        (
            "fn value() -> u8 { return 1u8 + 2u; } fn main() -> i64 { return 0; }",
            "right arithmetic operand has type `u64` but `u8` is required",
        ),
        (
            "fn main() -> i64 { var value: i64 = 1u8; return 0; }",
            "local initializer has type `u8` but `i64` is required",
        ),
        (
            "fn take(value: u8) -> unit {} fn main() -> i64 { take(1u); return 0; }",
            "call argument has type `u64` but `u8` is required",
        ),
        (
            "fn value() -> u8 { return -1u8; } fn main() -> i64 { return 0; }",
            "unary negation operand has type `u8` but `i64` is required",
        ),
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == expected));
    }
}

#[test]
fn checks_f64_raw_bits_signatures_and_typed_arithmetic() {
    let output = check_text(concat!(
        "extern fn observe(value: f64) -> unit;\n",
        "fn calculate(left: f64, right: f64) -> f64 { return -(left + right * 2.0 - 1.0); }\n",
        "fn main() -> i64 { var value: f64 = calculate(1.5, 2e0); observe(value); return 0; }\n",
    ));
    let hir = output.hir.expect("valid f64 program must produce HIR");
    let calculate = hir.definitions.get(FunctionId::new(1)).unwrap();
    let expression = returned_expression(calculate);

    assert_eq!(expression.ty, Type::F64);
    assert!(matches!(
        expression.kind,
        HirExpressionKind::Unary {
            operation: crate::hir::HirUnaryOperation::NegateF64,
            ..
        }
    ));
    let dump = dump_hir(&hir);
    assert!(dump.contains("F64 0x3ff8000000000000 : f64"));
    assert!(dump.contains("MultiplyF64"));
    assert!(dump.contains("SubtractF64"));
}

#[test]
fn converts_f64_boundaries_once_to_exact_raw_bits() {
    for (spelling, expected_bits) in [
        ("0.0", 0_u64),
        (
            "1.00000000000000011102230246251565404236316680908203125",
            1.0_f64.to_bits(),
        ),
        ("4.9406564584124654e-324", 1_u64),
        ("1e-400", 0_u64),
        ("1.7976931348623157e308", f64::MAX.to_bits()),
    ] {
        let output = check_text(&format!(
            "fn value() -> f64 {{ return {spelling}; }} fn main() -> i64 {{ return 0; }}"
        ));
        let hir = output.hir.expect("finite f64 literal must type-check");
        let value = hir.definitions.get(FunctionId::new(0)).unwrap();
        assert!(matches!(
            returned_expression(value).kind,
            HirExpressionKind::F64Bits(bits) if bits == expected_bits
        ));
    }
}

#[test]
fn diagnoses_f64_literal_overflow() {
    let output = check_text(
        "fn value() -> f64 { return 1.7976931348623159e308; } fn main() -> i64 { return 0; }",
    );

    assert!(output.hir.is_none());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, F64_LITERAL_OUT_OF_RANGE);
    assert!(diagnostic.message.contains("out of range for `f64`"));
}

#[test]
fn rejects_implicit_f64_conversions_and_truthiness() {
    for (source, expected) in [
        (
            "fn value() -> f64 { return 1; } fn main() -> i64 { return 0; }",
            "return value has type `i64` but `f64` is required",
        ),
        (
            "fn value() -> i64 { return 1.0; } fn main() -> i64 { return 0; }",
            "return value has type `f64` but `i64` is required",
        ),
        (
            "fn value() -> f64 { return 1.0 + 2; } fn main() -> i64 { return 0; }",
            "right arithmetic operand has type `i64` but `f64` is required",
        ),
        (
            "fn take(value: f64) -> unit {} fn main() -> i64 { take(1); return 0; }",
            "call argument has type `i64` but `f64` is required",
        ),
        (
            "fn main() -> i64 { if (1.0) { return 1; } return 0; }",
            "conditional condition has type `f64` but `bool` is required",
        ),
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none(), "{source}");
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message == expected),
            "{source}"
        );
    }
}

#[test]
fn unary_minus_admits_the_i64_minimum_boundary() {
    let output = check_text("fn main() -> i64 { return -9223372036854775808; }");
    let hir = output.hir.unwrap();
    let expression = returned_expression(hir.definitions.get(hir.entry_function).unwrap());

    assert_eq!(expression.ty, Type::I64);
    assert!(matches!(expression.kind, HirExpressionKind::I64(i64::MIN)));
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
