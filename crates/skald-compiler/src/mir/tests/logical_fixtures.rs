//! Reusable internal-HIR logical-expression fixtures.

use super::*;
use crate::{
    backend::{emit_assembly, Target},
    hir::{
        HirExpression, HirExpressionKind, HirFunctionDefinition, HirLogicalExpression,
        HirLogicalOperation, HirProgram, HirReturnValue, HirStatement, Type,
    },
    test_support::{run_native_assembly_output, type_check_source},
};

pub(super) fn function_id(program: &HirProgram, name: &str) -> FunctionId {
    program
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .map(|declaration| declaration.id)
        .unwrap_or_else(|| panic!("fixture function `{name}` must be declared"))
}

pub(super) fn function_id_from_mir(program: &MirProgram, name: &str) -> FunctionId {
    program
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .map(|declaration| declaration.id)
        .unwrap_or_else(|| panic!("fixture function `{name}` must be declared"))
}

pub(super) fn returned_scalar(definition: &HirFunctionDefinition) -> &HirExpression {
    let HirStatement::Return(statement) = definition.body.statements.last().unwrap() else {
        panic!("expected final return statement");
    };
    let HirReturnValue::Scalar(expression) = statement
        .value
        .as_ref()
        .expect("expected scalar return value")
    else {
        panic!("expected scalar return value");
    };
    expression
}

pub(super) fn returned_scalar_mut(definition: &mut HirFunctionDefinition) -> &mut HirExpression {
    let HirStatement::Return(statement) = definition.body.statements.last_mut().unwrap() else {
        panic!("expected final return statement");
    };
    let HirReturnValue::Scalar(expression) = statement
        .value
        .as_mut()
        .expect("expected scalar return value")
    else {
        panic!("expected scalar return value");
    };
    expression
}

pub(super) fn replace_return_with_logical(
    program: &mut HirProgram,
    destination: &str,
    operation: HirLogicalOperation,
    left: &str,
    right: &str,
) {
    let left = returned_scalar(
        program
            .definitions
            .get(function_id(program, left))
            .expect("left fixture function must have a body"),
    )
    .clone();
    let right = returned_scalar(
        program
            .definitions
            .get(function_id(program, right))
            .expect("right fixture function must have a body"),
    )
    .clone();
    replace_return_with_logical_expressions(program, destination, operation, left, right);
}

pub(super) fn replace_return_with_logical_expressions(
    program: &mut HirProgram,
    destination: &str,
    operation: HirLogicalOperation,
    left: HirExpression,
    right: HirExpression,
) {
    let destination = function_id(program, destination);
    let span = returned_scalar(
        program
            .definitions
            .get(destination)
            .expect("destination fixture function must have a body"),
    )
    .span;
    *returned_scalar_mut(
        program
            .definitions
            .get_mut_for_test(destination)
            .expect("destination fixture function must have a body"),
    ) = logical_expression(operation, left, right, span);
}

pub(super) fn boolean(value: bool, span: crate::source::Span) -> HirExpression {
    HirExpression {
        kind: HirExpressionKind::Boolean(value),
        ty: Type::Bool,
        span,
    }
}

pub(super) fn logical_expression(
    operation: HirLogicalOperation,
    left: HirExpression,
    right: HirExpression,
    span: crate::source::Span,
) -> HirExpression {
    HirExpression {
        kind: HirExpressionKind::Logical(Box::new(HirLogicalExpression::new(
            operation, left, right,
        ))),
        ty: Type::Bool,
        span,
    }
}

pub(super) fn lower_internal_logical(
    source: &str,
    destination: &str,
    operation: HirLogicalOperation,
    left: &str,
    right: &str,
) -> MirProgram {
    let checked = type_check_source(source);
    let mut hir = checked
        .hir
        .unwrap_or_else(|| panic!("fixture must type-check: {:?}", checked.diagnostics));
    replace_return_with_logical(&mut hir, destination, operation, left, right);
    lower_hir(&hir)
}

pub(super) fn native_output(mir: &MirProgram) -> std::process::Output {
    let mut assembly = emit_assembly(Target::X86_64SysV, mir).unwrap();
    assembly.push_str(println_digit_stub());
    run_native_assembly_output(&assembly)
}

fn println_digit_stub() -> &'static str {
    concat!(
        ".text\n",
        ".globl ska_rt_alloc\n",
        ".type ska_rt_alloc, @function\n",
        "ska_rt_alloc:\n",
        "    jmp malloc\n",
        ".size ska_rt_alloc, .-ska_rt_alloc\n",
        ".globl ska_rt_free\n",
        ".type ska_rt_free, @function\n",
        "ska_rt_free:\n",
        "    jmp free\n",
        ".size ska_rt_free, .-ska_rt_free\n",
        ".globl ska_rt_println_i64\n",
        ".type ska_rt_println_i64, @function\n",
        "ska_rt_println_i64:\n",
        "    sub rsp, 8\n",
        "    add dil, 48\n",
        "    mov byte ptr [rsp], dil\n",
        "    mov byte ptr [rsp + 1], 10\n",
        "    mov eax, 1\n",
        "    mov edi, 1\n",
        "    mov rsi, rsp\n",
        "    mov edx, 2\n",
        "    syscall\n",
        "    add rsp, 8\n",
        "    ret\n",
        ".size ska_rt_println_i64, .-ska_rt_println_i64\n",
    )
}
