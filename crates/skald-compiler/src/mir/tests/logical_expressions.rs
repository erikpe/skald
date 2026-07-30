use super::*;
use crate::{
    backend::{emit_assembly, Target},
    hir::{
        dump_hir, HirExpression, HirExpressionKind, HirFunctionDefinition, HirLogicalExpression,
        HirLogicalOperation, HirProgram, HirReturnValue, HirStatement, Type,
    },
    test_support::{run_native_assembly, run_native_assembly_output},
};

fn returned_expression_mut(definition: &mut HirFunctionDefinition) -> &mut HirExpression {
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

fn boolean(value: bool, span: crate::source::Span) -> HirExpression {
    HirExpression {
        kind: HirExpressionKind::Boolean(value),
        ty: Type::Bool,
        span,
    }
}

fn logical(
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

fn logical_program(expression: impl FnOnce(crate::source::Span) -> HirExpression) -> HirProgram {
    let mut hir = type_check_source(concat!(
        "fn evaluate() -> bool { return false; }\n",
        "fn main() -> i64 { if (evaluate()) { return 1; } return 0; }\n",
    ))
    .hir
    .unwrap();
    let target = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap(),
    );
    *target = expression(target.span);
    hir
}

fn lowered_logical(operation: HirLogicalOperation, left: bool, right: bool) -> MirProgram {
    let hir =
        logical_program(|span| logical(operation, boolean(left, span), boolean(right, span), span));
    lower_hir(&hir)
}

#[test]
fn structured_logical_hir_executes_both_truth_tables_natively() {
    for operation in [HirLogicalOperation::And, HirLogicalOperation::Or] {
        for left in [false, true] {
            for right in [false, true] {
                let mir = lowered_logical(operation, left, right);
                verify_mir(&mir).unwrap();
                let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();
                let status = run_native_assembly(&assembly);
                let expected = match operation {
                    HirLogicalOperation::And => left && right,
                    HirLogicalOperation::Or => left || right,
                };
                assert_eq!(status.code(), Some(i32::from(expected)));
            }
        }
    }
}

#[test]
fn nested_and_mixed_logical_hir_lowers_deterministically() {
    let cases = [
        logical_program(|span| {
            logical(
                HirLogicalOperation::And,
                logical(
                    HirLogicalOperation::And,
                    boolean(true, span),
                    boolean(false, span),
                    span,
                ),
                boolean(true, span),
                span,
            )
        }),
        logical_program(|span| {
            let grouped_right = logical(
                HirLogicalOperation::Or,
                boolean(false, span),
                boolean(true, span),
                span,
            );
            logical(
                HirLogicalOperation::And,
                boolean(true, span),
                HirExpression {
                    kind: HirExpressionKind::Grouped(Box::new(grouped_right)),
                    ty: Type::Bool,
                    span,
                },
                span,
            )
        }),
        logical_program(|span| {
            logical(
                HirLogicalOperation::Or,
                logical(
                    HirLogicalOperation::And,
                    boolean(true, span),
                    boolean(false, span),
                    span,
                ),
                boolean(true, span),
                span,
            )
        }),
    ];

    for hir in cases {
        let mir = lower_hir(&hir);
        verify_mir(&mir).unwrap();
        assert_eq!(dump_hir(&hir), dump_hir(&hir));
        assert_eq!(dump_mir(&mir), dump_mir(&mir));
        let function = mir.definitions.get(FunctionId::new(0)).unwrap();
        assert_eq!(function.body.logical_expressions.len(), 2);
        assert_eq!(function.body.path_conditions.len(), 2);
    }
}

#[test]
fn logical_hir_composes_under_eager_consumers_and_spills_earlier_values() {
    let hir = logical_program(|span| {
        let left = logical(
            HirLogicalOperation::Or,
            boolean(false, span),
            boolean(true, span),
            span,
        );
        let right = logical(
            HirLogicalOperation::And,
            boolean(true, span),
            boolean(false, span),
            span,
        );
        let comparison = HirExpression {
            kind: HirExpressionKind::PrimitiveComparison {
                operation: crate::hir::HirPrimitiveComparison {
                    predicate: crate::hir::HirComparisonPredicate::Equal,
                    operand: crate::hir::HirComparisonOperand::Bool,
                },
                left: Box::new(left),
                right: Box::new(right),
            },
            ty: Type::Bool,
            span,
        };
        HirExpression {
            kind: HirExpressionKind::Unary {
                operation: crate::hir::HirUnaryOperation::LogicalNotBool,
                operand: Box::new(comparison),
            },
            ty: Type::Bool,
            span,
        }
    });

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let function = mir.definitions.get(FunctionId::new(0)).unwrap();
    assert!(function
        .storage
        .iter()
        .any(|storage| storage.kind == MirStorageKind::ScalarSpill));
    assert!(dump_mir(&mir).contains("eq.bool"));
    assert!(dump_mir(&mir).contains("not.bool"));
}

#[test]
fn logical_calls_are_emitted_once_and_only_the_right_call_is_selected() {
    let mut hir = type_check_source(concat!(
        "fn left() -> bool { return true; }\n",
        "fn right() -> bool { return false; }\n",
        "fn evaluate() -> bool { return false; }\n",
        "fn main() -> i64 { if (evaluate()) { return 1; } return 0; }\n",
    ))
    .hir
    .unwrap();
    let expression = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(2))
            .unwrap(),
    );
    let span = expression.span;
    let call = |function| HirExpression {
        kind: HirExpressionKind::DirectCall {
            function,
            arguments: vec![],
        },
        ty: Type::Bool,
        span,
    };
    *expression = logical(
        HirLogicalOperation::And,
        call(FunctionId::new(0)),
        call(FunctionId::new(1)),
        span,
    );

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let function = mir.definitions.get(FunctionId::new(2)).unwrap();
    let logical = &function.body.logical_expressions[0];
    let calls: Vec<_> = function
        .body
        .blocks
        .iter()
        .flat_map(|block| {
            block.instructions.iter().filter_map(move |instruction| {
                let MirInstruction::Call(call) = instruction else {
                    return None;
                };
                Some((block.id, call.target))
            })
        })
        .collect();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0],
        (logical.split, MirCallTarget::Direct(FunctionId::new(0)))
    );
    assert_eq!(
        calls[1],
        (
            logical.right_entry,
            MirCallTarget::Direct(FunctionId::new(1))
        )
    );
}

#[test]
fn logical_result_composes_as_a_call_argument() {
    let mut hir = type_check_source(concat!(
        "fn consume(value: bool) -> bool { return value; }\n",
        "fn evaluate() -> bool { return consume(false); }\n",
        "fn main() -> i64 { if (evaluate()) { return 1; } return 0; }\n",
    ))
    .hir
    .unwrap();
    let expression = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(1))
            .unwrap(),
    );
    let HirExpressionKind::DirectCall { arguments, .. } = &mut expression.kind else {
        panic!("fixture must contain a direct call");
    };
    let crate::hir::HirCallArgument::Value(argument) = &mut arguments[0] else {
        panic!("fixture must contain one value argument");
    };
    let span = argument.span;
    *argument = logical(
        HirLogicalOperation::And,
        boolean(true, span),
        boolean(false, span),
        span,
    );

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let function = mir.definitions.get(FunctionId::new(1)).unwrap();
    let logical = &function.body.logical_expressions[0];
    let call = function.body.blocks[logical.join.index()]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .expect("call must consume the selected result after the logical join");
    assert_eq!(call.target, MirCallTarget::Direct(FunctionId::new(0)));
}

#[test]
fn malformed_logical_hir_is_rejected() {
    let result = std::panic::catch_unwind(|| {
        let mut hir = logical_program(|span| HirExpression {
            kind: HirExpressionKind::Logical(Box::new(HirLogicalExpression {
                operation: HirLogicalOperation::And,
                left: Box::new(HirExpression {
                    kind: HirExpressionKind::I64(1),
                    ty: Type::I64,
                    span,
                }),
                right: Box::new(boolean(true, span)),
            })),
            ty: Type::Bool,
            span,
        });
        let expression = returned_expression_mut(
            hir.definitions
                .get_mut_for_test(FunctionId::new(0))
                .unwrap(),
        );
        expression.ty = Type::I64;
        lower_hir(&hir)
    });
    assert!(result.is_err());
}

#[test]
fn source_logical_syntax_reaches_structured_hir_and_verified_mir() {
    let output = crate::test_support::type_check_source(concat!(
        "fn evaluate() -> bool { return true && false; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty());
    let hir = output.hir.unwrap();
    assert!(dump_hir(&hir).contains("Logical And : bool"));
    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let dump = dump_mir(&mir);
    assert!(dump.contains("LogicalExpressions"));
    assert!(dump.contains("\n        and condition "));
}

#[test]
fn source_logical_expressions_preserve_external_boolean_abi_and_skipping() {
    let mir = lower_text(concat!(
        "extern fn external_flag(value: bool) -> bool;\n",
        "extern fn external_call_count() -> i64;\n",
        "fn main() -> i64 {\n",
        "  var first: bool = external_flag(true) || external_flag(false);\n",
        "  var second: bool = external_flag(true) && external_flag(false);\n",
        "  if (first && !second && external_call_count() == 3) { return 0; }\n",
        "  return 1;\n",
        "}\n",
    ));
    verify_mir(&mir).unwrap();
    let mut assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    assembly.push_str(concat!(
        ".data\n",
        "external_counter:\n",
        "    .quad 0\n",
        ".text\n",
        ".globl external_flag\n",
        ".type external_flag, @function\n",
        "external_flag:\n",
        "    add qword ptr [rip + external_counter], 1\n",
        "    mov eax, edi\n",
        "    ret\n",
        ".size external_flag, .-external_flag\n",
        ".globl external_call_count\n",
        ".type external_call_count, @function\n",
        "external_call_count:\n",
        "    mov rax, qword ptr [rip + external_counter]\n",
        "    ret\n",
        ".size external_call_count, .-external_call_count\n",
    ));
    assert_eq!(run_native_assembly_output(&assembly).status.code(), Some(0));
}
