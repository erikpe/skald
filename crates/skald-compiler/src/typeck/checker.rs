//! Checked lowering from resolved M3 input to typed HIR.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{
        BlockFlow, HirBinaryOperation, HirBlock, HirCallStatement, HirConditional,
        HirConditionalArm, HirExpression, HirExpressionKind, HirFunctionDeclaration,
        HirFunctionDeclarationTable, HirFunctionDefinition, HirFunctionDefinitionTable,
        HirFunctionLinkage, HirLocal, HirLocalDecl, HirParameter, HirProgram, HirReturn,
        HirStatement, HirUnaryOperation, Type,
    },
    identity::{BindingId, FunctionId},
    literal::NumericLiteralKind,
    resolve::{
        ResolvedBinaryOperator, ResolvedBlock, ResolvedExpression, ResolvedFunctionDeclaration,
        ResolvedFunctionDefinition, ResolvedFunctionLinkage, ResolvedProgram, ResolvedStatement,
        ResolvedType, ResolvedTypeKind, ResolvedUnaryOperator,
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
pub const U64_LITERAL_OUT_OF_RANGE: &str = "TYP010";
pub const U8_LITERAL_OUT_OF_RANGE: &str = "TYP011";
pub const F64_LITERAL_OUT_OF_RANGE: &str = "TYP012";

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
) -> Option<FunctionId> {
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
        let has_valid_parameters = declaration.parameters.iter().all(|parameter| {
            matches!(
                lower_type(&parameter.type_syntax),
                Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool
            )
        });
        let has_valid_return = matches!(
            lower_type(&declaration.return_type),
            Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool | Type::Unit
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
                    "expected by-value `i64`, `u64`, `u8`, `f64`, or `bool` parameters and an `i64`, `u64`, `u8`, `f64`, `bool`, or `unit` result",
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

    if return_type != Type::Unit && body.flow == BlockFlow::FallsThrough {
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
    let mut statements = Vec::with_capacity(block.statements.len());
    let mut flow = BlockFlow::FallsThrough;
    for statement in &block.statements {
        let checked = check_statement(
            program,
            declaration,
            definition,
            statement,
            return_type,
            diagnostics,
        );
        flow = flow.then(checked.flow);
        if let Some(statement) = checked.hir {
            statements.push(statement);
        }
    }

    HirBlock {
        statements,
        flow,
        span: block.span,
    }
}

struct CheckedStatement {
    hir: Option<HirStatement>,
    flow: BlockFlow,
}

impl CheckedStatement {
    const fn falls_through(hir: Option<HirStatement>) -> Self {
        Self {
            hir,
            flow: BlockFlow::FallsThrough,
        }
    }

    const fn terminates(hir: Option<HirStatement>) -> Self {
        Self {
            hir,
            flow: BlockFlow::Terminates,
        }
    }
}

fn check_statement(
    program: &ResolvedProgram,
    declaration: &ResolvedFunctionDeclaration,
    definition: &ResolvedFunctionDefinition,
    statement: &ResolvedStatement,
    return_type: Type,
    diagnostics: &mut Diagnostics,
) -> CheckedStatement {
    match statement {
        ResolvedStatement::Local(local) => {
            let metadata = definition
                .local(local.local)
                .expect("resolved local declaration must reference local metadata");
            let expected = lower_type(&metadata.type_syntax);
            let Some(initializer) = check_expression(
                program,
                declaration,
                definition,
                &local.initializer,
                diagnostics,
            ) else {
                return CheckedStatement::falls_through(None);
            };
            let hir = require_type(
                initializer.ty,
                expected,
                initializer.span,
                "local initializer",
                diagnostics,
            )
            .then_some(HirStatement::Local(HirLocalDecl {
                local: local.local,
                initializer,
                span: local.span,
            }));
            CheckedStatement::falls_through(hir)
        }
        ResolvedStatement::Return(statement) => {
            let hir = match (return_type, &statement.value) {
                (Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool, Some(value)) => {
                    let Some(value) =
                        check_expression(program, declaration, definition, value, diagnostics)
                    else {
                        return CheckedStatement::terminates(None);
                    };
                    require_type(
                        value.ty,
                        return_type,
                        value.span,
                        "return value",
                        diagnostics,
                    )
                    .then_some(HirStatement::Return(HirReturn {
                        value: Some(value),
                        span: statement.span,
                    }))
                }
                (Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool, None) => {
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
            };
            CheckedStatement::terminates(hir)
        }
        ResolvedStatement::Expression(statement) => {
            let Some(expression) = check_expression(
                program,
                declaration,
                definition,
                &statement.expression,
                diagnostics,
            ) else {
                return CheckedStatement::falls_through(None);
            };
            if !is_direct_call_through_groups(&statement.expression) {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_CALL_STATEMENT,
                        "only function calls can be used as expression statements",
                    )
                    .with_primary_label(statement.span, "this expression is not a call"),
                );
                return CheckedStatement::falls_through(None);
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
                return CheckedStatement::falls_through(None);
            }
            CheckedStatement::falls_through(Some(HirStatement::Call(HirCallStatement {
                call: expression,
                span: statement.span,
            })))
        }
        ResolvedStatement::Conditional(statement) => {
            let mut arms = Vec::with_capacity(statement.arms.len());
            let mut valid = true;
            let mut all_arms_terminate = true;
            for arm in &statement.arms {
                let condition = check_expression(
                    program,
                    declaration,
                    definition,
                    &arm.condition,
                    diagnostics,
                );
                let body = check_block(
                    program,
                    declaration,
                    definition,
                    &arm.body,
                    return_type,
                    diagnostics,
                );
                all_arms_terminate &= body.flow == BlockFlow::Terminates;
                match condition {
                    Some(condition)
                        if require_type(
                            condition.ty,
                            Type::Bool,
                            condition.span,
                            "conditional condition",
                            diagnostics,
                        ) =>
                    {
                        arms.push(HirConditionalArm {
                            condition,
                            body,
                            span: arm.span,
                        })
                    }
                    _ => valid = false,
                }
            }
            let else_block = statement.else_block.as_ref().map(|block| {
                check_block(
                    program,
                    declaration,
                    definition,
                    block,
                    return_type,
                    diagnostics,
                )
            });
            let flow = if all_arms_terminate
                && else_block
                    .as_ref()
                    .is_some_and(|block| block.flow == BlockFlow::Terminates)
            {
                BlockFlow::Terminates
            } else {
                BlockFlow::FallsThrough
            };

            let hir = valid.then_some(HirStatement::Conditional(HirConditional {
                arms,
                else_block,
                flow,
                span: statement.span,
            }));
            CheckedStatement { hir, flow }
        }
        ResolvedStatement::Block(block) => {
            let block = check_block(
                program,
                declaration,
                definition,
                block,
                return_type,
                diagnostics,
            );
            let flow = block.flow;
            CheckedStatement {
                hir: Some(HirStatement::Block(block)),
                flow,
            }
        }
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
        ResolvedExpression::NumericLiteral(literal) => check_numeric_literal(literal, diagnostics),
        ResolvedExpression::Boolean(boolean) => Some(HirExpression {
            kind: HirExpressionKind::Boolean(boolean.value),
            ty: Type::Bool,
            span: boolean.span,
        }),
        ResolvedExpression::Unary(unary) => {
            if unary.operator == ResolvedUnaryOperator::Negate {
                if let Some(literal) = i64_literal_through_groups(&unary.operand) {
                    match classify_i64_magnitude(&literal.spelling) {
                        Magnitude::MinimumBoundary => {
                            return Some(HirExpression {
                                kind: HirExpressionKind::I64(i64::MIN),
                                ty: Type::I64,
                                span: unary.span,
                            });
                        }
                        Magnitude::TooLarge => {
                            report_integer_out_of_range(
                                diagnostics,
                                unary.span,
                                format!("-{}", literal.spelling),
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
            let operation = match operand.ty {
                Type::I64 => HirUnaryOperation::NegateI64,
                Type::F64 => HirUnaryOperation::NegateF64,
                _ => {
                    require_type(
                        operand.ty,
                        Type::I64,
                        operand.span,
                        "unary negation operand",
                        diagnostics,
                    );
                    return None;
                }
            };
            let ty = operand.ty;
            Some(HirExpression {
                kind: HirExpressionKind::Unary {
                    operation,
                    operand: Box::new(operand),
                },
                ty,
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
            let operation = if left.ty == right.ty {
                select_binary_operation(binary.operator, left.ty)
            } else {
                None
            };
            let Some(operation) = operation else {
                let expected = if matches!(left.ty, Type::I64 | Type::U64 | Type::U8 | Type::F64) {
                    left.ty
                } else {
                    Type::I64
                };
                let left_valid = require_type(
                    left.ty,
                    expected,
                    left.span,
                    "left arithmetic operand",
                    diagnostics,
                );
                let right_valid = require_type(
                    right.ty,
                    expected,
                    right.span,
                    "right arithmetic operand",
                    diagnostics,
                );
                debug_assert!(!left_valid || !right_valid);
                return None;
            };
            let ty = left.ty;

            Some(HirExpression {
                kind: HirExpressionKind::Binary {
                    operation,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                ty,
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

fn select_binary_operation(
    operator: ResolvedBinaryOperator,
    operand_type: Type,
) -> Option<HirBinaryOperation> {
    match (operator, operand_type) {
        (ResolvedBinaryOperator::Add, Type::I64) => Some(HirBinaryOperation::AddI64),
        (ResolvedBinaryOperator::Subtract, Type::I64) => Some(HirBinaryOperation::SubtractI64),
        (ResolvedBinaryOperator::Multiply, Type::I64) => Some(HirBinaryOperation::MultiplyI64),
        (ResolvedBinaryOperator::Add, Type::U64) => Some(HirBinaryOperation::AddU64),
        (ResolvedBinaryOperator::Subtract, Type::U64) => Some(HirBinaryOperation::SubtractU64),
        (ResolvedBinaryOperator::Multiply, Type::U64) => Some(HirBinaryOperation::MultiplyU64),
        (ResolvedBinaryOperator::Add, Type::U8) => Some(HirBinaryOperation::AddU8),
        (ResolvedBinaryOperator::Subtract, Type::U8) => Some(HirBinaryOperation::SubtractU8),
        (ResolvedBinaryOperator::Multiply, Type::U8) => Some(HirBinaryOperation::MultiplyU8),
        (ResolvedBinaryOperator::Add, Type::F64) => Some(HirBinaryOperation::AddF64),
        (ResolvedBinaryOperator::Subtract, Type::F64) => Some(HirBinaryOperation::SubtractF64),
        (ResolvedBinaryOperator::Multiply, Type::F64) => Some(HirBinaryOperation::MultiplyF64),
        (_, Type::Bool | Type::Unit) => None,
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

fn check_numeric_literal(
    literal: &crate::resolve::ResolvedNumericLiteralExpr,
    diagnostics: &mut Diagnostics,
) -> Option<HirExpression> {
    match literal.kind {
        NumericLiteralKind::I64 => check_positive_i64_literal(literal, diagnostics),
        NumericLiteralKind::U64 => check_u64_literal(literal, diagnostics),
        NumericLiteralKind::U8 => check_u8_literal(literal, diagnostics),
        NumericLiteralKind::F64 => check_f64_literal(literal, diagnostics),
    }
}

fn check_positive_i64_literal(
    literal: &crate::resolve::ResolvedNumericLiteralExpr,
    diagnostics: &mut Diagnostics,
) -> Option<HirExpression> {
    match literal.spelling.parse::<i64>() {
        Ok(value) => Some(HirExpression {
            kind: HirExpressionKind::I64(value),
            ty: Type::I64,
            span: literal.span,
        }),
        Err(_) => {
            report_integer_out_of_range(diagnostics, literal.span, literal.spelling.clone());
            None
        }
    }
}

fn check_u64_literal(
    literal: &crate::resolve::ResolvedNumericLiteralExpr,
    diagnostics: &mut Diagnostics,
) -> Option<HirExpression> {
    let digits = literal
        .spelling
        .strip_suffix('u')
        .expect("validated u64 literal must have a `u` suffix");
    match digits.parse::<u64>() {
        Ok(value) => Some(HirExpression {
            kind: HirExpressionKind::U64(value),
            ty: Type::U64,
            span: literal.span,
        }),
        Err(_) => {
            diagnostics.push(
                Diagnostic::error(
                    U64_LITERAL_OUT_OF_RANGE,
                    format!(
                        "integer literal `{}` is out of range for `u64`",
                        literal.spelling
                    ),
                )
                .with_primary_label(literal.span, "value is not representable as `u64`")
                .with_note(format!(
                    "the inclusive `u64` range is 0 through {}",
                    u64::MAX
                )),
            );
            None
        }
    }
}

fn check_u8_literal(
    literal: &crate::resolve::ResolvedNumericLiteralExpr,
    diagnostics: &mut Diagnostics,
) -> Option<HirExpression> {
    let digits = literal
        .spelling
        .strip_suffix("u8")
        .expect("validated u8 literal must have a `u8` suffix");
    match digits.parse::<u8>() {
        Ok(value) => Some(HirExpression {
            kind: HirExpressionKind::U8(value),
            ty: Type::U8,
            span: literal.span,
        }),
        Err(_) => {
            diagnostics.push(
                Diagnostic::error(
                    U8_LITERAL_OUT_OF_RANGE,
                    format!(
                        "integer literal `{}` is out of range for `u8`",
                        literal.spelling
                    ),
                )
                .with_primary_label(literal.span, "value is not representable as `u8`")
                .with_note(format!("the inclusive `u8` range is 0 through {}", u8::MAX)),
            );
            None
        }
    }
}

fn check_f64_literal(
    literal: &crate::resolve::ResolvedNumericLiteralExpr,
    diagnostics: &mut Diagnostics,
) -> Option<HirExpression> {
    let value = literal
        .spelling
        .parse::<f64>()
        .expect("validated decimal f64 literal must parse");
    if value.is_finite() {
        Some(HirExpression {
            kind: HirExpressionKind::F64Bits(value.to_bits()),
            ty: Type::F64,
            span: literal.span,
        })
    } else {
        diagnostics.push(
            Diagnostic::error(
                F64_LITERAL_OUT_OF_RANGE,
                format!(
                    "floating literal `{}` is out of range for `f64`",
                    literal.spelling
                ),
            )
            .with_primary_label(literal.span, "value rounds to infinity")
            .with_note("finite `f64` literals must round to a finite IEEE-754 binary64 value"),
        );
        None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Magnitude {
    PositiveI64,
    MinimumBoundary,
    TooLarge,
}

fn classify_i64_magnitude(spelling: &str) -> Magnitude {
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

fn i64_literal_through_groups(
    expression: &ResolvedExpression,
) -> Option<&crate::resolve::ResolvedNumericLiteralExpr> {
    match expression {
        ResolvedExpression::NumericLiteral(literal) if literal.kind == NumericLiteralKind::I64 => {
            Some(literal)
        }
        ResolvedExpression::Grouped(grouped) => i64_literal_through_groups(&grouped.expression),
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
        ResolvedTypeKind::U64 => Type::U64,
        ResolvedTypeKind::U8 => Type::U8,
        ResolvedTypeKind::F64 => Type::F64,
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
