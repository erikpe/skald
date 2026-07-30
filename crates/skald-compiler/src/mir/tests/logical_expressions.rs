use super::*;
use crate::{
    backend::{emit_assembly, Target},
    hir::{
        dump_hir, HirExpression, HirExpressionKind, HirFunctionDefinition, HirLogicalExpression,
        HirLogicalOperation, HirProgram, HirReturnValue, HirStatement, Type,
    },
    test_support::run_native_assembly,
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
fn internal_logical_hir_executes_both_truth_tables_natively() {
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
fn malformed_logical_hir_and_mir_are_rejected() {
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

    let mut wrong_short = lowered_logical(HirLogicalOperation::And, true, true);
    let function = wrong_short
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let short = function.body.logical_expressions[0].short;
    let block = &mut function.body.blocks[short.index()];
    let fixed = block
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) => Some(assignment),
            _ => None,
        })
        .unwrap();
    fixed.rvalue.kind = MirRvalueKind::ConstantBool(true);
    let errors = verify_mir(&wrong_short).unwrap_err().to_string();
    assert!(errors.contains("logical short path stores the wrong selected result"));

    let mut missing_store = lowered_logical(HirLogicalOperation::Or, false, true);
    let function = missing_store
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let right = function.body.logical_expressions[0].right_exit;
    function.body.blocks[right.index()].instructions.pop();
    let errors = verify_mir(&missing_store).unwrap_err().to_string();
    assert!(errors.contains("logical right result block must end by storing its result"));

    let mut wrong_branch = lowered_logical(HirLogicalOperation::And, true, false);
    let function = wrong_branch
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let logical = function.body.logical_expressions[0].clone();
    let Some(MirTerminator::Branch {
        true_target,
        false_target,
        ..
    }) = &mut function.body.blocks[logical.split.index()].terminator
    else {
        unreachable!()
    };
    std::mem::swap(true_target, false_target);
    let errors = verify_mir(&wrong_branch).unwrap_err().to_string();
    assert!(errors.contains("logical split has the wrong operand or branch targets"));

    let mut duplicate_right = lowered_logical(HirLogicalOperation::Or, false, true);
    let function = duplicate_right
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let logical = function.body.logical_expressions[0].clone();
    let duplicated = function.body.blocks[logical.right_exit.index()]
        .instructions
        .last()
        .unwrap()
        .clone();
    function.body.blocks[logical.right_exit.index()]
        .instructions
        .push(duplicated);
    let errors = verify_mir(&duplicate_right).unwrap_err().to_string();
    assert!(errors.contains("logical result carrier must be written exactly once"));

    let mut duplicated_evaluation = lowered_logical(HirLogicalOperation::And, true, false);
    let function = duplicated_evaluation
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let logical = function.body.logical_expressions[0].clone();
    let right_definition = function.body.blocks[logical.right_exit.index()]
        .instructions
        .iter()
        .find(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(assignment)
                    if assignment.result == logical.right_result
            )
        })
        .unwrap()
        .clone();
    function.body.blocks[logical.split.index()]
        .instructions
        .push(right_definition);
    let errors = verify_mir(&duplicated_evaluation).unwrap_err().to_string();
    assert!(errors.contains("is defined more than once"));

    let mut use_before_join = lowered_logical(HirLogicalOperation::Or, false, true);
    let function = use_before_join
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let logical = function.body.logical_expressions[0].clone();
    let join = &mut function.body.blocks[logical.join.index()];
    let selected = join
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(assignment)
                    if assignment.result == logical.selected_result
            )
        })
        .unwrap();
    let selected = join.instructions.remove(selected);
    function.body.blocks[logical.split.index()]
        .instructions
        .push(selected);
    let errors = verify_mir(&use_before_join).unwrap_err().to_string();
    assert!(errors.contains("logical selected result must load its carrier in the result join"));
}

#[test]
fn source_logical_syntax_remains_unavailable() {
    let mut sources = crate::source::SourceDatabase::new();
    let source = sources.add("test.ska", "true && false");
    let output = crate::lexer::lex(sources.get(source).unwrap());
    assert_eq!(output.diagnostics.len(), 2);
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == crate::lexer::UNEXPECTED_CHARACTER));
}
