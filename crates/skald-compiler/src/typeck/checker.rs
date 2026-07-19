//! Checked lowering from resolved M3 input to typed HIR.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{
        HirBinaryOperation, HirBlock, HirExpression, HirExpressionKind, HirFunction,
        HirFunctionTable, HirLocal, HirLocalDecl, HirParameter, HirProgram, HirReturn,
        HirStatement, HirUnaryOperation, Type,
    },
    resolve::{
        BindingId, ResolvedBinaryOperator, ResolvedBlock, ResolvedExpression, ResolvedFunction,
        ResolvedProgram, ResolvedStatement, ResolvedType, ResolvedTypeKind, ResolvedUnaryOperator,
    },
    source::Span,
};

pub const MISSING_ENTRY_POINT: &str = "TYP001";
pub const INVALID_ENTRY_POINT: &str = "TYP002";
pub const INTEGER_LITERAL_OUT_OF_RANGE: &str = "TYP003";
pub const WRONG_ARGUMENT_COUNT: &str = "TYP004";
pub const TYPE_MISMATCH: &str = "TYP005";
pub const MISSING_RETURN: &str = "TYP006";

#[derive(Debug)]
pub struct TypeCheckOutput {
    /// Present only when the entire resolved program type-checks successfully.
    pub hir: Option<HirProgram>,
    pub diagnostics: Diagnostics,
}

impl TypeCheckOutput {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_errors()
    }
}

pub fn type_check(program: &ResolvedProgram) -> TypeCheckOutput {
    let mut diagnostics = Diagnostics::new();
    let entry_function = check_entry_point(program, &mut diagnostics);
    let functions = program
        .functions
        .iter()
        .map(|function| check_function(program, function, &mut diagnostics))
        .collect();

    let hir = if diagnostics.has_errors() {
        None
    } else {
        Some(HirProgram {
            functions: HirFunctionTable::new(functions),
            entry_function: entry_function.expect("valid program must have an entry function"),
            span: program.span,
        })
    };

    TypeCheckOutput { hir, diagnostics }
}

fn check_entry_point(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) -> Option<crate::resolve::FunctionId> {
    let Some(entry_id) = program.entry_function else {
        let start = program.span.range().start();
        diagnostics.push(
            Diagnostic::error(MISSING_ENTRY_POINT, "missing entry function `main`")
                .with_primary_label(
                    Span::empty(program.span.source_id(), start),
                    "define `fn main() -> i64` in this file",
                ),
        );
        return None;
    };
    let entry = program
        .functions
        .get(entry_id)
        .expect("resolved entry ID must exist in the function table");
    let return_type = lower_type(&entry.return_type);

    if !entry.parameters.is_empty() || return_type != Type::I64 {
        diagnostics.push(
            Diagnostic::error(
                INVALID_ENTRY_POINT,
                "entry function must have signature `fn main() -> i64`",
            )
            .with_primary_label(entry.name_span, "invalid entry signature")
            .with_note(format!(
                "found {} parameter{} and return type `{}`",
                entry.parameters.len(),
                if entry.parameters.len() == 1 { "" } else { "s" },
                return_type.name()
            )),
        );
        return None;
    }

    Some(entry_id)
}

fn check_function(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    diagnostics: &mut Diagnostics,
) -> HirFunction {
    let parameters = function
        .parameters
        .iter()
        .map(|parameter| HirParameter {
            id: parameter.id,
            name: parameter.name.clone(),
            name_span: parameter.name_span,
            ty: lower_type(&parameter.type_syntax),
            span: parameter.span,
        })
        .collect();
    let locals = function
        .locals
        .iter()
        .map(|local| HirLocal {
            id: local.id,
            name: local.name.clone(),
            name_span: local.name_span,
            ty: lower_type(&local.type_syntax),
            span: local.span,
        })
        .collect();
    let return_type = lower_type(&function.return_type);
    let body = check_block(program, function, &function.body, return_type, diagnostics);

    if !block_guarantees_return(&function.body) {
        diagnostics.push(
            Diagnostic::error(
                MISSING_RETURN,
                format!("function `{}` does not return a value", function.name),
            )
            .with_primary_label(
                function.body.span,
                "a return value is required on every path",
            )
            .with_note(format!(
                "function `{}` declares return type `{}`",
                function.name,
                return_type.name()
            )),
        );
    }

    HirFunction {
        id: function.id,
        name: function.name.clone(),
        name_span: function.name_span,
        parameters,
        return_type,
        locals,
        body,
        span: function.span,
    }
}

fn check_block(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    block: &ResolvedBlock,
    return_type: Type,
    diagnostics: &mut Diagnostics,
) -> HirBlock {
    let statements = block
        .statements
        .iter()
        .filter_map(|statement| {
            check_statement(program, function, statement, return_type, diagnostics)
        })
        .collect();

    HirBlock {
        statements,
        span: block.span,
    }
}

fn check_statement(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    statement: &ResolvedStatement,
    return_type: Type,
    diagnostics: &mut Diagnostics,
) -> Option<HirStatement> {
    match statement {
        ResolvedStatement::Local(local) => {
            let metadata = function
                .local(local.local)
                .expect("resolved local declaration must reference local metadata");
            let expected = lower_type(&metadata.type_syntax);
            let initializer = check_expression(program, function, &local.initializer, diagnostics)?;
            if !require_type(
                initializer.ty,
                expected,
                initializer.span,
                "local initializer",
                diagnostics,
            ) {
                return None;
            }
            Some(HirStatement::Local(HirLocalDecl {
                local: local.local,
                initializer,
                span: local.span,
            }))
        }
        ResolvedStatement::Return(statement) => {
            let value = check_expression(program, function, &statement.value, diagnostics)?;
            if !require_type(
                value.ty,
                return_type,
                value.span,
                "return value",
                diagnostics,
            ) {
                return None;
            }
            Some(HirStatement::Return(HirReturn {
                value,
                span: statement.span,
            }))
        }
        ResolvedStatement::Block(block) => Some(HirStatement::Block(check_block(
            program,
            function,
            block,
            return_type,
            diagnostics,
        ))),
    }
}

fn check_expression(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpression,
    diagnostics: &mut Diagnostics,
) -> Option<HirExpression> {
    match expression {
        ResolvedExpression::Binding(binding) => {
            let ty = binding_type(function, binding.binding);
            Some(HirExpression {
                kind: HirExpressionKind::Binding(binding.binding),
                ty,
                span: binding.span,
            })
        }
        ResolvedExpression::Integer(integer) => {
            check_positive_integer(&integer.spelling, integer.span, diagnostics)
        }
        ResolvedExpression::Unary(unary) => {
            if unary.operator == ResolvedUnaryOperator::Negate {
                if let Some(integer) = integer_through_groups(&unary.operand) {
                    match classify_magnitude(&integer.spelling) {
                        Magnitude::MinimumBoundary => {
                            return Some(HirExpression {
                                kind: HirExpressionKind::Integer(i64::MIN),
                                ty: Type::I64,
                                span: unary.span,
                            });
                        }
                        Magnitude::TooLarge => {
                            report_integer_out_of_range(
                                diagnostics,
                                unary.span,
                                format!("-{}", integer.spelling),
                            );
                            return None;
                        }
                        Magnitude::PositiveI64 => {}
                    }
                }
            }

            let operand = check_expression(program, function, &unary.operand, diagnostics)?;
            if !require_type(
                operand.ty,
                Type::I64,
                operand.span,
                "unary negation operand",
                diagnostics,
            ) {
                return None;
            }
            Some(HirExpression {
                kind: HirExpressionKind::Unary {
                    operation: match unary.operator {
                        ResolvedUnaryOperator::Negate => HirUnaryOperation::NegateI64,
                    },
                    operand: Box::new(operand),
                },
                ty: Type::I64,
                span: unary.span,
            })
        }
        ResolvedExpression::Binary(binary) => {
            let left = check_expression(program, function, &binary.left, diagnostics);
            let right = check_expression(program, function, &binary.right, diagnostics);
            let (left, right) = match (left, right) {
                (Some(left), Some(right)) => (left, right),
                _ => return None,
            };
            let left_valid = require_type(
                left.ty,
                Type::I64,
                left.span,
                "left arithmetic operand",
                diagnostics,
            );
            let right_valid = require_type(
                right.ty,
                Type::I64,
                right.span,
                "right arithmetic operand",
                diagnostics,
            );
            if !left_valid || !right_valid {
                return None;
            }

            Some(HirExpression {
                kind: HirExpressionKind::Binary {
                    operation: match binary.operator {
                        ResolvedBinaryOperator::Add => HirBinaryOperation::AddI64,
                        ResolvedBinaryOperator::Subtract => HirBinaryOperation::SubtractI64,
                        ResolvedBinaryOperator::Multiply => HirBinaryOperation::MultiplyI64,
                    },
                    left: Box::new(left),
                    right: Box::new(right),
                },
                ty: Type::I64,
                span: binary.span,
            })
        }
        ResolvedExpression::DirectCall(call) => {
            let target = program
                .functions
                .get(call.function)
                .expect("resolved direct-call target must exist");
            let mut arguments = Vec::with_capacity(call.arguments.len());
            let mut valid = true;
            for argument in &call.arguments {
                match check_expression(program, function, argument, diagnostics) {
                    Some(argument) => arguments.push(argument),
                    None => valid = false,
                }
            }

            if arguments.len() == call.arguments.len()
                && call.arguments.len() == target.parameters.len()
            {
                for (argument, parameter) in arguments.iter().zip(&target.parameters) {
                    valid &= require_type(
                        argument.ty,
                        lower_type(&parameter.type_syntax),
                        argument.span,
                        "call argument",
                        diagnostics,
                    );
                }
            } else if call.arguments.len() != target.parameters.len() {
                diagnostics.push(
                    Diagnostic::error(
                        WRONG_ARGUMENT_COUNT,
                        format!(
                            "function `{}` expects {} argument{} but received {}",
                            target.name,
                            target.parameters.len(),
                            if target.parameters.len() == 1 {
                                ""
                            } else {
                                "s"
                            },
                            call.arguments.len()
                        ),
                    )
                    .with_primary_label(
                        call.callee_span,
                        "called with the wrong number of arguments",
                    )
                    .with_secondary_label(target.name_span, "function declared here"),
                );
                valid = false;
            }

            if !valid {
                return None;
            }
            Some(HirExpression {
                kind: HirExpressionKind::DirectCall {
                    function: call.function,
                    arguments,
                },
                ty: lower_type(&target.return_type),
                span: call.span,
            })
        }
        ResolvedExpression::Grouped(grouped) => {
            let inner = check_expression(program, function, &grouped.expression, diagnostics)?;
            let ty = inner.ty;
            Some(HirExpression {
                kind: HirExpressionKind::Grouped(Box::new(inner)),
                ty,
                span: grouped.span,
            })
        }
    }
}

fn binding_type(function: &ResolvedFunction, binding: BindingId) -> Type {
    assert_eq!(
        binding.function(),
        function.id,
        "resolved binding must belong to the current function"
    );
    match binding {
        BindingId::Parameter(id) => lower_type(
            &function
                .parameter(id)
                .expect("resolved parameter ID must exist")
                .type_syntax,
        ),
        BindingId::Local(id) => lower_type(
            &function
                .local(id)
                .expect("resolved local ID must exist")
                .type_syntax,
        ),
    }
}

fn check_positive_integer(
    spelling: &str,
    span: Span,
    diagnostics: &mut Diagnostics,
) -> Option<HirExpression> {
    match spelling.parse::<i64>() {
        Ok(value) => Some(HirExpression {
            kind: HirExpressionKind::Integer(value),
            ty: Type::I64,
            span,
        }),
        Err(_) => {
            report_integer_out_of_range(diagnostics, span, spelling.to_owned());
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Magnitude {
    PositiveI64,
    MinimumBoundary,
    TooLarge,
}

fn classify_magnitude(spelling: &str) -> Magnitude {
    let Ok(magnitude) = spelling.parse::<u128>() else {
        return Magnitude::TooLarge;
    };
    let minimum_magnitude = (i64::MAX as u128) + 1;
    if magnitude <= i64::MAX as u128 {
        Magnitude::PositiveI64
    } else if magnitude == minimum_magnitude {
        Magnitude::MinimumBoundary
    } else {
        Magnitude::TooLarge
    }
}

fn integer_through_groups(
    expression: &ResolvedExpression,
) -> Option<&crate::resolve::ResolvedIntegerExpr> {
    match expression {
        ResolvedExpression::Integer(integer) => Some(integer),
        ResolvedExpression::Grouped(grouped) => integer_through_groups(&grouped.expression),
        _ => None,
    }
}

fn report_integer_out_of_range(diagnostics: &mut Diagnostics, span: Span, spelling: String) {
    diagnostics.push(
        Diagnostic::error(
            INTEGER_LITERAL_OUT_OF_RANGE,
            format!("integer literal `{spelling}` is out of range for `i64`"),
        )
        .with_primary_label(span, "value is not representable as `i64`")
        .with_note(format!(
            "the inclusive `i64` range is {} through {}",
            i64::MIN,
            i64::MAX
        )),
    );
}

fn require_type(
    actual: Type,
    expected: Type,
    span: Span,
    context: &'static str,
    diagnostics: &mut Diagnostics,
) -> bool {
    if actual == expected {
        return true;
    }
    diagnostics.push(
        Diagnostic::error(
            TYPE_MISMATCH,
            format!(
                "{context} has type `{}` but `{}` is required",
                actual.name(),
                expected.name()
            ),
        )
        .with_primary_label(span, "type mismatch"),
    );
    false
}

fn lower_type(type_syntax: &ResolvedType) -> Type {
    match type_syntax.kind {
        ResolvedTypeKind::I64 => Type::I64,
    }
}

fn block_guarantees_return(block: &ResolvedBlock) -> bool {
    block.statements.iter().any(|statement| match statement {
        ResolvedStatement::Return(_) => true,
        ResolvedStatement::Block(block) => block_guarantees_return(block),
        ResolvedStatement::Local(_) => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hir::{dump_hir, HirBinaryOperation},
        lexer::lex,
        resolve::resolve,
        source::SourceDatabase,
        syntax::parse,
    };

    fn check_text(text: &str) -> TypeCheckOutput {
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
        type_check(&resolved.program)
    }

    fn returned_expression(function: &HirFunction) -> &HirExpression {
        let HirStatement::Return(statement) = function.body.statements.last().unwrap() else {
            panic!("expected final return statement");
        };
        &statement.value
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
        assert_eq!(hir.functions.len(), 2);

        for function in hir.functions.iter() {
            assert_eq!(function.return_type, Type::I64);
            for parameter in &function.parameters {
                assert_eq!(parameter.ty, Type::I64);
            }
            for local in &function.locals {
                assert_eq!(local.ty, Type::I64);
            }
            for statement in &function.body.statements {
                match statement {
                    HirStatement::Local(local) => {
                        assert_expression_is_fully_typed(&local.initializer)
                    }
                    HirStatement::Return(statement) => {
                        assert_expression_is_fully_typed(&statement.value)
                    }
                    HirStatement::Block(_) => {}
                }
            }
        }

        let main = hir.functions.get(hir.entry_function).unwrap();
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
            returned_expression(hir.functions.get(hir.entry_function).unwrap()).kind,
            HirExpressionKind::Integer(i64::MAX)
        ));
    }

    #[test]
    fn unary_minus_admits_the_i64_minimum_boundary() {
        let output = check_text("fn main() -> i64 { return -9223372036854775808; }");
        let hir = output.hir.unwrap();
        let expression = returned_expression(hir.functions.get(hir.entry_function).unwrap());

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
                "  Functions\n",
                "    Function f0 \"main\" @0..35\n",
                "      Parameters\n",
                "      ReturnType i64\n",
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
}
