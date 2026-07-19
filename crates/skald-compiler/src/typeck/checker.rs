//! Checked lowering from resolved M3 input to typed HIR.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{
        HirBinaryOperation, HirBlock, HirCallStatement, HirExpression, HirExpressionKind,
        HirFunctionDeclaration, HirFunctionDeclarationTable, HirFunctionDefinition,
        HirFunctionDefinitionTable, HirFunctionLinkage, HirLocal, HirLocalDecl, HirParameter,
        HirProgram, HirReturn, HirStatement, HirUnaryOperation, Type,
    },
    resolve::{
        BindingId, ResolvedBinaryOperator, ResolvedBlock, ResolvedExpression,
        ResolvedFunctionDeclaration, ResolvedFunctionDefinition, ResolvedFunctionLinkage,
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
pub const INVALID_RETURN: &str = "TYP007";
pub const INVALID_CALL_STATEMENT: &str = "TYP008";
pub const INVALID_EXTERNAL_DECLARATION: &str = "TYP009";

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
    check_external_declarations(program, &mut diagnostics);
    let entry_function = check_entry_point(program, &mut diagnostics);
    let declarations = program.declarations.iter().map(lower_declaration).collect();
    let definitions = program
        .declarations
        .iter()
        .map(|declaration| {
            program.definitions.get(declaration.id).map(|definition| {
                check_definition(program, declaration, definition, &mut diagnostics)
            })
        })
        .collect();

    let hir = if diagnostics.has_errors() {
        None
    } else {
        Some(HirProgram {
            declarations: HirFunctionDeclarationTable::new(declarations),
            definitions: HirFunctionDefinitionTable::new(definitions),
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
        .declarations
        .get(entry_id)
        .expect("resolved entry ID must exist in the declaration table");
    let return_type = lower_type(&entry.return_type);

    if !matches!(entry.linkage, ResolvedFunctionLinkage::Internal)
        || program.definitions.get(entry_id).is_none()
    {
        diagnostics.push(
            Diagnostic::error(
                INVALID_ENTRY_POINT,
                "entry function must have signature `fn main() -> i64`",
            )
            .with_primary_label(
                entry.name_span,
                "an external declaration cannot be the entry point",
            )
            .with_note("define `fn main() -> i64` with a Skald function body"),
        );
        return None;
    }

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

fn check_external_declarations(program: &ResolvedProgram, diagnostics: &mut Diagnostics) {
    for declaration in program.declarations.iter() {
        let ResolvedFunctionLinkage::External { symbol } = &declaration.linkage else {
            continue;
        };
        let has_valid_parameters = declaration
            .parameters
            .iter()
            .all(|parameter| matches!(lower_type(&parameter.type_syntax), Type::I64 | Type::Bool));
        let has_valid_return = matches!(
            lower_type(&declaration.return_type),
            Type::I64 | Type::Bool | Type::Unit
        );
        if !has_valid_parameters || !has_valid_return || symbol != &declaration.name {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_EXTERNAL_DECLARATION,
                    format!(
                        "external function `{}` has an unsupported signature",
                        declaration.name
                    ),
                )
                .with_primary_label(
                    declaration.span,
                    "expected by-value `i64` or `bool` parameters and an `i64`, `bool`, or `unit` result",
                )
                .with_note("the source function name must also be its exact linker symbol"),
            );
        }
    }
}

fn lower_declaration(function: &ResolvedFunctionDeclaration) -> HirFunctionDeclaration {
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

    HirFunctionDeclaration {
        id: function.id,
        name: function.name.clone(),
        name_span: function.name_span,
        parameters,
        return_type: lower_type(&function.return_type),
        linkage: match &function.linkage {
            ResolvedFunctionLinkage::Internal => HirFunctionLinkage::Internal,
            ResolvedFunctionLinkage::External { symbol } => HirFunctionLinkage::External {
                symbol: symbol.clone(),
            },
        },
        span: function.span,
    }
}

fn check_definition(
    program: &ResolvedProgram,
    declaration: &ResolvedFunctionDeclaration,
    definition: &ResolvedFunctionDefinition,
    diagnostics: &mut Diagnostics,
) -> HirFunctionDefinition {
    let locals = definition
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
    let return_type = lower_type(&declaration.return_type);
    let body = check_block(
        program,
        declaration,
        definition,
        &definition.body,
        return_type,
        diagnostics,
    );

    if return_type != Type::Unit && !block_guarantees_return(&definition.body) {
        diagnostics.push(
            Diagnostic::error(
                MISSING_RETURN,
                format!("function `{}` does not return a value", declaration.name),
            )
            .with_primary_label(
                definition.body.span,
                "a return value is required on every path",
            )
            .with_note(format!(
                "function `{}` declares return type `{}`",
                declaration.name,
                return_type.name()
            )),
        );
    }

    HirFunctionDefinition {
        function: definition.function,
        locals,
        body,
        span: definition.span,
    }
}

fn check_block(
    program: &ResolvedProgram,
    declaration: &ResolvedFunctionDeclaration,
    definition: &ResolvedFunctionDefinition,
    block: &ResolvedBlock,
    return_type: Type,
    diagnostics: &mut Diagnostics,
) -> HirBlock {
    let statements = block
        .statements
        .iter()
        .filter_map(|statement| {
            check_statement(
                program,
                declaration,
                definition,
                statement,
                return_type,
                diagnostics,
            )
        })
        .collect();

    HirBlock {
        statements,
        span: block.span,
    }
}

fn check_statement(
    program: &ResolvedProgram,
    declaration: &ResolvedFunctionDeclaration,
    definition: &ResolvedFunctionDefinition,
    statement: &ResolvedStatement,
    return_type: Type,
    diagnostics: &mut Diagnostics,
) -> Option<HirStatement> {
    match statement {
        ResolvedStatement::Local(local) => {
            let metadata = definition
                .local(local.local)
                .expect("resolved local declaration must reference local metadata");
            let expected = lower_type(&metadata.type_syntax);
            let initializer = check_expression(
                program,
                declaration,
                definition,
                &local.initializer,
                diagnostics,
            )?;
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
            match (return_type, &statement.value) {
                (Type::I64 | Type::Bool, Some(value)) => {
                    let value =
                        check_expression(program, declaration, definition, value, diagnostics)?;
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
                        value: Some(value),
                        span: statement.span,
                    }))
                }
                (Type::I64 | Type::Bool, None) => {
                    diagnostics.push(
                        Diagnostic::error(
                            INVALID_RETURN,
                            format!(
                                "{} `{}` function must return a value",
                                return_type.indefinite_article(),
                                return_type.name()
                            ),
                        )
                        .with_primary_label(statement.span, "expected `return expression;`"),
                    );
                    None
                }
                (Type::Unit, Some(value)) => {
                    // Check the expression as well so independent errors in it
                    // are not hidden by the invalid return form.
                    let _ = check_expression(program, declaration, definition, value, diagnostics);
                    diagnostics.push(
                        Diagnostic::error(
                            INVALID_RETURN,
                            "a `unit` function cannot return a value",
                        )
                        .with_primary_label(statement.span, "use `return;` instead"),
                    );
                    None
                }
                (Type::Unit, None) => Some(HirStatement::Return(HirReturn {
                    value: None,
                    span: statement.span,
                })),
            }
        }
        ResolvedStatement::Expression(statement) => {
            let expression = check_expression(
                program,
                declaration,
                definition,
                &statement.expression,
                diagnostics,
            )?;
            if !is_direct_call_through_groups(&statement.expression) {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_CALL_STATEMENT,
                        "only function calls can be used as expression statements",
                    )
                    .with_primary_label(statement.span, "this expression is not a call"),
                );
                return None;
            }
            if expression.ty != Type::Unit {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_CALL_STATEMENT,
                        "a call statement must call a function returning `unit`",
                    )
                    .with_primary_label(
                        statement.span,
                        format!("this call returns `{}`", expression.ty.name()),
                    )
                    .with_note("use the returned value instead of discarding it"),
                );
                return None;
            }
            Some(HirStatement::Call(HirCallStatement {
                call: expression,
                span: statement.span,
            }))
        }
        ResolvedStatement::Block(block) => Some(HirStatement::Block(check_block(
            program,
            declaration,
            definition,
            block,
            return_type,
            diagnostics,
        ))),
    }
}

fn check_expression(
    program: &ResolvedProgram,
    declaration: &ResolvedFunctionDeclaration,
    definition: &ResolvedFunctionDefinition,
    expression: &ResolvedExpression,
    diagnostics: &mut Diagnostics,
) -> Option<HirExpression> {
    match expression {
        ResolvedExpression::Binding(binding) => {
            let ty = binding_type(declaration, definition, binding.binding);
            Some(HirExpression {
                kind: HirExpressionKind::Binding(binding.binding),
                ty,
                span: binding.span,
            })
        }
        ResolvedExpression::Integer(integer) => {
            check_positive_integer(&integer.spelling, integer.span, diagnostics)
        }
        ResolvedExpression::Boolean(boolean) => Some(HirExpression {
            kind: HirExpressionKind::Boolean(boolean.value),
            ty: Type::Bool,
            span: boolean.span,
        }),
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

            let operand = check_expression(
                program,
                declaration,
                definition,
                &unary.operand,
                diagnostics,
            )?;
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
            let left =
                check_expression(program, declaration, definition, &binary.left, diagnostics);
            let right =
                check_expression(program, declaration, definition, &binary.right, diagnostics);
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
                .declarations
                .get(call.function)
                .expect("resolved direct-call target must exist");
            let mut arguments = Vec::with_capacity(call.arguments.len());
            let mut valid = true;
            for argument in &call.arguments {
                match check_expression(program, declaration, definition, argument, diagnostics) {
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
            let inner = check_expression(
                program,
                declaration,
                definition,
                &grouped.expression,
                diagnostics,
            )?;
            let ty = inner.ty;
            Some(HirExpression {
                kind: HirExpressionKind::Grouped(Box::new(inner)),
                ty,
                span: grouped.span,
            })
        }
    }
}

fn binding_type(
    declaration: &ResolvedFunctionDeclaration,
    definition: &ResolvedFunctionDefinition,
    binding: BindingId,
) -> Type {
    assert_eq!(
        binding.function(),
        declaration.id,
        "resolved binding must belong to the current function"
    );
    match binding {
        BindingId::Parameter(id) => lower_type(
            &declaration
                .parameter(id)
                .expect("resolved parameter ID must exist")
                .type_syntax,
        ),
        BindingId::Local(id) => lower_type(
            &definition
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
        ResolvedTypeKind::Bool => Type::Bool,
        ResolvedTypeKind::Unit => Type::Unit,
    }
}

fn is_direct_call_through_groups(expression: &ResolvedExpression) -> bool {
    match expression {
        ResolvedExpression::DirectCall(_) => true,
        ResolvedExpression::Grouped(grouped) => is_direct_call_through_groups(&grouped.expression),
        _ => false,
    }
}

fn block_guarantees_return(block: &ResolvedBlock) -> bool {
    block.statements.iter().any(|statement| match statement {
        ResolvedStatement::Return(_) => true,
        ResolvedStatement::Block(block) => block_guarantees_return(block),
        ResolvedStatement::Local(_) | ResolvedStatement::Expression(_) => false,
    })
}
