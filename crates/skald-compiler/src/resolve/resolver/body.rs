//! Callable-body resolution facade and shared expression/name resolution.

use super::*;
use crate::{
    diagnostics::Diagnostic,
    identity::{BindingId, CallableId, ClassId, InitializerId},
};

mod call;
mod place;
mod statement;

pub(super) struct ResolvedCallableBody {
    pub(super) locals: Vec<ResolvedLocal>,
    pub(super) body: ResolvedBlock,
}

#[derive(Clone, Copy)]
pub(super) struct BodyResolutionEnvironment<'program> {
    top_levels: &'program HashMap<String, TopLevelSymbol>,
    classes: &'program ResolvedClassDeclarationTable,
    interfaces: &'program ResolvedInterfaceDeclarationTable,
    class_symbols: &'program [ClassSymbols],
    hierarchy: &'program ResolvedClassHierarchy,
}

impl<'program> BodyResolutionEnvironment<'program> {
    pub(super) fn new(
        top_levels: &'program HashMap<String, TopLevelSymbol>,
        classes: &'program ResolvedClassDeclarationTable,
        interfaces: &'program ResolvedInterfaceDeclarationTable,
        class_symbols: &'program [ClassSymbols],
        hierarchy: &'program ResolvedClassHierarchy,
    ) -> Self {
        Self {
            top_levels,
            classes,
            interfaces,
            class_symbols,
            hierarchy,
        }
    }
}

pub(super) fn resolve_callable_body(
    callable: CallableId,
    receiver_class: Option<ClassId>,
    parameters: &[ResolvedParameter],
    body: &syntax::Block,
    base_initialization: BaseInitializationPolicy,
    environment: BodyResolutionEnvironment<'_>,
    diagnostics: &mut Diagnostics,
) -> ResolvedCallableBody {
    CallableResolver::new(
        callable,
        receiver_class,
        parameters,
        base_initialization,
        environment,
        diagnostics,
    )
    .resolve(body)
}

#[derive(Clone, Copy)]
pub(super) enum BaseInitializationPolicy {
    Forbidden,
    Required {
        base: ClassId,
        initializer: Option<InitializerId>,
    },
}

#[derive(Clone, Copy)]
struct BindingSymbol {
    id: BindingId,
    ty: ResolvedTypeKind,
    name_span: Span,
}

struct CallableResolver<'program, 'diagnostics> {
    callable: CallableId,
    receiver_class: Option<ClassId>,
    environment: BodyResolutionEnvironment<'program>,
    diagnostics: &'diagnostics mut Diagnostics,
    base_initialization: BaseInitializationPolicy,
    scopes: Vec<HashMap<String, BindingSymbol>>,
    locals: Vec<ResolvedLocal>,
}

impl<'program, 'diagnostics> CallableResolver<'program, 'diagnostics> {
    fn new(
        callable: CallableId,
        receiver_class: Option<ClassId>,
        parameters: &[ResolvedParameter],
        base_initialization: BaseInitializationPolicy,
        environment: BodyResolutionEnvironment<'program>,
        diagnostics: &'diagnostics mut Diagnostics,
    ) -> Self {
        let parameters = parameters
            .iter()
            .map(|parameter| {
                (
                    parameter.name.clone(),
                    BindingSymbol {
                        id: BindingId::Parameter(parameter.id),
                        ty: parameter.type_syntax.kind,
                        name_span: parameter.name_span,
                    },
                )
            })
            .collect();
        Self {
            callable,
            receiver_class,
            environment,
            diagnostics,
            base_initialization,
            scopes: vec![parameters],
            locals: Vec::new(),
        }
    }

    fn resolve(mut self, body: &syntax::Block) -> ResolvedCallableBody {
        if matches!(
            self.base_initialization,
            BaseInitializationPolicy::Required { .. }
        ) && !matches!(
            body.statements.first(),
            Some(syntax::Statement::BaseInitialization(_))
        ) {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_BASE_INITIALIZATION,
                    "a derived initializer must begin with `super(...)`",
                )
                .with_primary_label(
                    body.statements
                        .first()
                        .map(syntax::Statement::span)
                        .unwrap_or(body.span),
                    "initialize the direct base before derived fields",
                ),
            );
        }
        let body = self.resolve_block(body, false);
        ResolvedCallableBody {
            locals: self.locals,
            body,
        }
    }

    fn resolve_view_target(&mut self, name: &syntax::Name) -> Option<ResolvedType> {
        self.resolve_type(&syntax::TypeSyntax {
            kind: syntax::TypeKind::Named(name.clone()),
            span: name.span,
        })
    }

    fn resolve_type(&mut self, type_syntax: &syntax::TypeSyntax) -> Option<ResolvedType> {
        super::resolve_type(type_syntax, self.environment.top_levels, self.diagnostics)
    }

    fn resolve_expression(
        &mut self,
        expression: &syntax::Expression,
    ) -> Option<ResolvedExpression> {
        match expression {
            syntax::Expression::Identifier(identifier) => self.resolve_identifier(identifier),
            syntax::Expression::NumericLiteral(literal) => Some(
                ResolvedExpression::NumericLiteral(ResolvedNumericLiteralExpr {
                    kind: literal.kind,
                    spelling: literal.spelling.clone(),
                    span: literal.span,
                }),
            ),
            syntax::Expression::Boolean(boolean) => {
                Some(ResolvedExpression::Boolean(ResolvedBooleanExpr {
                    value: boolean.value,
                    span: boolean.span,
                }))
            }
            syntax::Expression::Unary(unary) => {
                let operand = self.resolve_expression(&unary.operand)?;
                Some(ResolvedExpression::Unary(ResolvedUnaryExpr {
                    operator: match unary.operator {
                        syntax::UnaryOperator::Negate => ResolvedUnaryOperator::Negate,
                    },
                    operator_span: unary.operator_span,
                    operand: Box::new(operand),
                    span: unary.span,
                }))
            }
            syntax::Expression::Binary(binary) => {
                let left = self.resolve_expression(&binary.left);
                let right = self.resolve_expression(&binary.right);
                match (left, right) {
                    (Some(left), Some(right)) => {
                        Some(ResolvedExpression::Binary(ResolvedBinaryExpr {
                            left: Box::new(left),
                            operator: match binary.operator {
                                syntax::BinaryOperator::Add => ResolvedBinaryOperator::Add,
                                syntax::BinaryOperator::Subtract => {
                                    ResolvedBinaryOperator::Subtract
                                }
                                syntax::BinaryOperator::Multiply => {
                                    ResolvedBinaryOperator::Multiply
                                }
                            },
                            operator_span: binary.operator_span,
                            right: Box::new(right),
                            span: binary.span,
                        }))
                    }
                    _ => None,
                }
            }
            syntax::Expression::TypeTest(test) => {
                let source = self.resolve_expression(&test.source);
                let target = self.resolve_view_target(&test.target);
                match (source, target) {
                    (Some(source), Some(target)) => {
                        Some(ResolvedExpression::TypeTest(ResolvedTypeTestExpr {
                            source: Box::new(source),
                            target,
                            target_span: test.target.span,
                            span: test.span,
                        }))
                    }
                    _ => None,
                }
            }
            syntax::Expression::ObjectCast(cast) => {
                let source = self.resolve_expression(&cast.source);
                let target = self.resolve_view_target(&cast.target);
                match (source, target) {
                    (Some(source), Some(target)) => {
                        Some(ResolvedExpression::ObjectCast(ResolvedObjectCastExpr {
                            source: Box::new(source),
                            target,
                            target_mode: match cast.target_mode {
                                syntax::ObjectCastTargetMode::Plain => {
                                    ResolvedObjectCastTargetMode::Plain
                                }
                                syntax::ObjectCastTargetMode::Shared { shared_span } => {
                                    ResolvedObjectCastTargetMode::Shared { shared_span }
                                }
                            },
                            target_span: cast.target.span,
                            span: cast.span,
                        }))
                    }
                    _ => None,
                }
            }
            syntax::Expression::Call(call) => self.resolve_call(call),
            syntax::Expression::Grouped(grouped) => {
                let expression = self.resolve_expression(&grouped.expression)?;
                Some(ResolvedExpression::Grouped(ResolvedGroupedExpr {
                    expression: Box::new(expression),
                    span: grouped.span,
                }))
            }
            syntax::Expression::SelfValue(self_value) => self.resolve_self(self_value.span),
            syntax::Expression::MemberAccess(member) => self.resolve_field_access(member),
        }
    }

    fn resolve_identifier(
        &mut self,
        identifier: &syntax::IdentifierExpr,
    ) -> Option<ResolvedExpression> {
        if let Some(symbol) = self.lookup_binding(&identifier.name.text) {
            return Some(ResolvedExpression::Binding(ResolvedBindingExpr {
                binding: symbol.id,
                span: identifier.span,
            }));
        }
        match self.environment.top_levels.get(&identifier.name.text) {
            Some(TopLevelSymbol {
                kind: TopLevelSymbolKind::Function(_),
                ..
            }) => self.diagnostics.push(
                Diagnostic::error(
                    TOP_LEVEL_USED_AS_VALUE,
                    format!(
                        "function `{}` cannot be used as a value",
                        identifier.name.text
                    ),
                )
                .with_primary_label(identifier.span, "call the function with `(...)`"),
            ),
            Some(TopLevelSymbol {
                kind: TopLevelSymbolKind::Class(_),
                ..
            }) => self.diagnostics.push(
                Diagnostic::error(
                    TOP_LEVEL_USED_AS_VALUE,
                    format!("class `{}` cannot be used as a value", identifier.name.text),
                )
                .with_primary_label(identifier.span, "construct it with `(...)`"),
            ),
            Some(TopLevelSymbol {
                kind: TopLevelSymbolKind::Interface(_),
                ..
            }) => self.diagnostics.push(
                Diagnostic::error(
                    TOP_LEVEL_USED_AS_VALUE,
                    format!(
                        "interface `{}` cannot be used as a value",
                        identifier.name.text
                    ),
                )
                .with_primary_label(identifier.span, "interfaces are declaration-only"),
            ),
            None => self.report_unknown(&identifier.name.text, identifier.span, "unknown name"),
        }
        None
    }

    fn resolve_self(&mut self, span: Span) -> Option<ResolvedExpression> {
        self.receiver_class.or_else(|| {
            self.diagnostics.push(
                Diagnostic::error(SELF_OUTSIDE_MEMBER, "`self` is not available here")
                    .with_primary_label(span, "only an initializer or instance method has `self`"),
            );
            None
        })?;
        Some(ResolvedExpression::Binding(ResolvedBindingExpr {
            binding: BindingId::Receiver(self.callable),
            span,
        }))
    }

    fn lookup_binding(&self, name: &str) -> Option<BindingSymbol> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn report_unknown(&mut self, name: &str, span: Span, kind: &'static str) {
        self.diagnostics.push(
            Diagnostic::error(UNKNOWN_NAME, format!("{kind} `{name}`"))
                .with_primary_label(span, "not declared in this scope"),
        );
    }
}
