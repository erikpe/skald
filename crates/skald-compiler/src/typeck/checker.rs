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
