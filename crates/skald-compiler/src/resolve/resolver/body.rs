//! Callable-body resolution facade and shared expression/name resolution.

use std::collections::HashMap;

use super::*;
use crate::{
    diagnostics::Diagnostic,
    identity::{
        BindingId, CallableId, ClassId, FieldId, LiteralDataId, LoopId, MethodId, StaticFieldId,
    },
    source::{Span, TextRange},
};

mod allocation;
mod call;
mod dereference;
mod place;
mod statement;

/// A selected ordinary class member after privacy and hierarchy lookup.
#[derive(Clone, Copy)]
enum SelectedClassMember {
    Field(FieldId),
    StaticField(StaticFieldId),
    Method(MethodId),
}

impl SelectedClassMember {
    const fn declaring_class(self) -> ClassId {
        match self {
            Self::Field(field) => field.class(),
            Self::StaticField(field) => field.class(),
            Self::Method(method) => method.class(),
        }
    }
}

pub(super) struct ResolvedCallableBody {
    pub(super) locals: Vec<ResolvedLocal>,
    pub(super) body: ResolvedBlock,
}

#[derive(Clone, Copy)]
pub(super) struct BodyResolutionEnvironment<'program> {
    lookup: ModuleLookup<'program>,
    functions: &'program ResolvedFunctionDeclarationTable,
    classes: &'program ResolvedClassDeclarationTable,
    interfaces: &'program ResolvedInterfaceDeclarationTable,
    hierarchy: &'program ResolvedClassHierarchy,
    has_module_context: bool,
    string_literals: StringLiteralResolutionEnvironment<'program>,
}

#[derive(Clone, Copy)]
pub(super) struct StringLiteralResolutionEnvironment<'program> {
    language_item: Option<&'program ResolvedStringLanguageItem>,
    ids: &'program HashMap<Span, LiteralDataId>,
}

impl<'program> StringLiteralResolutionEnvironment<'program> {
    pub(super) const fn new(
        language_item: Option<&'program ResolvedStringLanguageItem>,
        ids: &'program HashMap<Span, LiteralDataId>,
    ) -> Self {
        Self { language_item, ids }
    }
}

impl<'program> BodyResolutionEnvironment<'program> {
    pub(super) fn new(
        lookup: ModuleLookup<'program>,
        functions: &'program ResolvedFunctionDeclarationTable,
        classes: &'program ResolvedClassDeclarationTable,
        interfaces: &'program ResolvedInterfaceDeclarationTable,
        hierarchy: &'program ResolvedClassHierarchy,
        has_module_context: bool,
        string_literals: StringLiteralResolutionEnvironment<'program>,
    ) -> Self {
        Self {
            lookup,
            functions,
            classes,
            interfaces,
            hierarchy,
            has_module_context,
            string_literals,
        }
    }
}

pub(super) fn resolve_callable_body(
    context: CallableResolutionContext,
    parameters: &[ResolvedParameter],
    body: &syntax::Block,
    environment: BodyResolutionEnvironment<'_>,
    array_types: &mut ArrayTypeInterner,
    diagnostics: &mut Diagnostics,
) -> ResolvedCallableBody {
    CallableResolver::new(context, parameters, environment, array_types, diagnostics).resolve(body)
}

pub(super) fn resolve_static_initializer_expression(
    context: CallableResolutionContext,
    expression: &syntax::Expression,
    environment: BodyResolutionEnvironment<'_>,
    array_types: &mut ArrayTypeInterner,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedExpression> {
    CallableResolver::new(context, &[], environment, array_types, diagnostics)
        .resolve_declaration_expression(expression)
}

#[derive(Clone, Copy)]
pub(super) struct CallableResolutionContext {
    callable: CallableId,
    class_owner: Option<ClassId>,
    receiver_class: Option<ClassId>,
    base_initialization: BaseInitializationPolicy,
}

impl CallableResolutionContext {
    pub(super) const fn callable(self) -> CallableId {
        self.callable
    }

    pub(super) const fn function(callable: CallableId) -> Self {
        Self {
            callable,
            class_owner: None,
            receiver_class: None,
            base_initialization: BaseInitializationPolicy::Forbidden,
        }
    }

    pub(super) const fn member(
        callable: CallableId,
        class_owner: ClassId,
        receiver_class: Option<ClassId>,
        base_initialization: BaseInitializationPolicy,
    ) -> Self {
        Self {
            callable,
            class_owner: Some(class_owner),
            receiver_class,
            base_initialization,
        }
    }

    pub(super) const fn receiver_member(
        callable: CallableId,
        class_owner: ClassId,
        base_initialization: BaseInitializationPolicy,
    ) -> Self {
        Self::member(
            callable,
            class_owner,
            Some(class_owner),
            base_initialization,
        )
    }

    pub(super) const fn static_initializer(callable: CallableId, class_owner: ClassId) -> Self {
        Self::member(
            callable,
            class_owner,
            None,
            BaseInitializationPolicy::Forbidden,
        )
    }
}

#[derive(Clone, Copy)]
pub(super) enum BaseInitializationPolicy {
    Forbidden,
    Required { base: ClassId },
}

#[derive(Clone, Copy)]
struct BindingSymbol {
    id: BindingId,
    ty: ResolvedTypeKind,
    name_span: Span,
}

struct CallableResolver<'program, 'state> {
    callable: CallableId,
    class_owner: Option<ClassId>,
    receiver_class: Option<ClassId>,
    environment: BodyResolutionEnvironment<'program>,
    array_types: &'state mut ArrayTypeInterner,
    diagnostics: &'state mut Diagnostics,
    base_initialization: BaseInitializationPolicy,
    scopes: Vec<HashMap<String, BindingSymbol>>,
    locals: Vec<ResolvedLocal>,
    next_loop_index: usize,
    active_loops: Vec<LoopId>,
}

impl<'program, 'state> CallableResolver<'program, 'state> {
    fn new(
        context: CallableResolutionContext,
        parameters: &[ResolvedParameter],
        environment: BodyResolutionEnvironment<'program>,
        array_types: &'state mut ArrayTypeInterner,
        diagnostics: &'state mut Diagnostics,
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
            callable: context.callable,
            class_owner: context.class_owner,
            receiver_class: context.receiver_class,
            environment,
            array_types,
            diagnostics,
            base_initialization: context.base_initialization,
            scopes: vec![parameters],
            locals: Vec::new(),
            next_loop_index: 0,
            active_loops: Vec::new(),
        }
    }

    fn resolve(mut self, body: &syntax::Block) -> ResolvedCallableBody {
        debug_assert_eq!(self.class_owner, self.callable.class());
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

    fn resolve_declaration_expression(
        mut self,
        expression: &syntax::Expression,
    ) -> Option<ResolvedExpression> {
        debug_assert_eq!(self.class_owner, self.callable.class());
        debug_assert!(self.receiver_class.is_none());
        debug_assert!(matches!(
            self.base_initialization,
            BaseInitializationPolicy::Forbidden
        ));
        self.resolve_expression(expression)
    }

    fn resolve_view_target(&mut self, name: &syntax::Name) -> Option<ResolvedType> {
        self.resolve_type(&syntax::TypeSyntax {
            kind: syntax::TypeKind::Named(name.clone()),
            span: name.span,
        })
    }

    fn resolve_type(&mut self, type_syntax: &syntax::TypeSyntax) -> Option<ResolvedType> {
        super::resolve_type(
            type_syntax,
            self.environment.lookup,
            self.array_types,
            self.diagnostics,
        )
    }

    fn resolve_expression(
        &mut self,
        expression: &syntax::Expression,
    ) -> Option<ResolvedExpression> {
        match expression {
            syntax::Expression::Absent(absent) => {
                Some(ResolvedExpression::Absent(ResolvedAbsentExpr {
                    span: absent.span,
                }))
            }
            syntax::Expression::Identifier(identifier) => self.resolve_identifier(identifier),
            syntax::Expression::NumericLiteral(literal) => Some(
                ResolvedExpression::NumericLiteral(ResolvedNumericLiteralExpr {
                    kind: literal.kind,
                    spelling: literal.spelling.clone(),
                    span: literal.span,
                }),
            ),
            syntax::Expression::ByteLiteral(literal) => {
                Some(ResolvedExpression::ByteLiteral(ResolvedByteLiteralExpr {
                    value: literal.value,
                    span: literal.span,
                }))
            }
            syntax::Expression::StringLiteral(literal) => {
                if !self.environment.has_module_context
                    && self
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.code == MISSING_STRING_LANGUAGE_ITEM)
                {
                    return None;
                }
                if !self.environment.has_module_context {
                    self.diagnostics.push(
                        Diagnostic::error(
                            MISSING_STRING_LANGUAGE_ITEM,
                            "string literal requires the `std::str::Str` language item",
                        )
                        .with_primary_label(literal.span, "required by this string literal")
                        .with_note("the source-text convenience API has no module providers"),
                    );
                    return None;
                }
                let item = self.environment.string_literals.language_item?;
                let data = *self
                    .environment
                    .string_literals
                    .ids
                    .get(&literal.span)
                    .expect("loaded string literal must have a canonical data identity");
                Some(ResolvedExpression::StringLiteral(
                    ResolvedStringLiteralExpr {
                        data,
                        class: item.class,
                        span: literal.span,
                    },
                ))
            }
            syntax::Expression::Boolean(boolean) => {
                Some(ResolvedExpression::Boolean(ResolvedBooleanExpr {
                    value: boolean.value,
                    span: boolean.span,
                }))
            }
            syntax::Expression::Unary(unary) => {
                if unary.operator == syntax::UnaryOperator::Dereference {
                    return self
                        .resolve_dereference(
                            &unary.operand,
                            ResolvedDereferenceOperator::Star,
                            unary.operator_span,
                            unary.span,
                        )
                        .map(ResolvedExpression::Dereference);
                }
                let operand = self.resolve_expression(&unary.operand)?;
                Some(ResolvedExpression::Unary(ResolvedUnaryExpr {
                    operator: match unary.operator {
                        syntax::UnaryOperator::Negate => ResolvedUnaryOperator::Negate,
                        syntax::UnaryOperator::LogicalNot => ResolvedUnaryOperator::LogicalNot,
                        syntax::UnaryOperator::BitwiseComplement => {
                            ResolvedUnaryOperator::BitwiseComplement
                        }
                        syntax::UnaryOperator::Dereference => {
                            unreachable!("dereference returned above")
                        }
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
                                syntax::BinaryOperator::Divide => ResolvedBinaryOperator::Divide,
                                syntax::BinaryOperator::Remainder => {
                                    ResolvedBinaryOperator::Remainder
                                }
                                syntax::BinaryOperator::ShiftLeft => {
                                    ResolvedBinaryOperator::ShiftLeft
                                }
                                syntax::BinaryOperator::ShiftRight => {
                                    ResolvedBinaryOperator::ShiftRight
                                }
                                syntax::BinaryOperator::BitwiseAnd => {
                                    ResolvedBinaryOperator::BitwiseAnd
                                }
                                syntax::BinaryOperator::BitwiseOr => {
                                    ResolvedBinaryOperator::BitwiseOr
                                }
                                syntax::BinaryOperator::BitwiseXor => {
                                    ResolvedBinaryOperator::BitwiseXor
                                }
                                syntax::BinaryOperator::Equal => ResolvedBinaryOperator::Equal,
                                syntax::BinaryOperator::NotEqual => {
                                    ResolvedBinaryOperator::NotEqual
                                }
                                syntax::BinaryOperator::LessThan => {
                                    ResolvedBinaryOperator::LessThan
                                }
                                syntax::BinaryOperator::LessEqual => {
                                    ResolvedBinaryOperator::LessEqual
                                }
                                syntax::BinaryOperator::GreaterThan => {
                                    ResolvedBinaryOperator::GreaterThan
                                }
                                syntax::BinaryOperator::GreaterEqual => {
                                    ResolvedBinaryOperator::GreaterEqual
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
            syntax::Expression::Logical(logical) => {
                let left = self.resolve_expression(&logical.left);
                let right = self.resolve_expression(&logical.right);
                match (left, right) {
                    (Some(left), Some(right)) => {
                        Some(ResolvedExpression::Logical(ResolvedLogicalExpr {
                            left: Box::new(left),
                            operator: match logical.operator {
                                syntax::LogicalOperator::And => ResolvedLogicalOperator::And,
                                syntax::LogicalOperator::Or => ResolvedLogicalOperator::Or,
                            },
                            operator_span: logical.operator_span,
                            right: Box::new(right),
                            span: logical.span,
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
            syntax::Expression::PresenceTest(test) => {
                let source = self.resolve_expression(&test.source)?;
                Some(ResolvedExpression::PresenceTest(ResolvedPresenceTestExpr {
                    source: Box::new(source),
                    kind: match test.kind {
                        syntax::PresenceTestKind::Some => ResolvedPresenceTestKind::Some,
                        syntax::PresenceTestKind::None => ResolvedPresenceTestKind::None,
                    },
                    is_span: test.is_span,
                    target_span: test.target_span,
                    span: test.span,
                }))
            }
            syntax::Expression::Unwrap(unwrap) => {
                let source = self.resolve_expression(&unwrap.source)?;
                Some(ResolvedExpression::Unwrap(ResolvedUnwrapExpr {
                    source: Box::new(source),
                    bang_span: unwrap.bang_span,
                    span: unwrap.span,
                }))
            }
            syntax::Expression::PrimitiveCast(cast) => {
                let source = self.resolve_expression(&cast.source)?;
                Some(ResolvedExpression::PrimitiveCast(
                    ResolvedPrimitiveCastExpr {
                        target: match cast.target {
                            syntax::PrimitiveType::I64 => ResolvedPrimitiveType::I64,
                            syntax::PrimitiveType::U64 => ResolvedPrimitiveType::U64,
                            syntax::PrimitiveType::U8 => ResolvedPrimitiveType::U8,
                            syntax::PrimitiveType::F64 => ResolvedPrimitiveType::F64,
                            syntax::PrimitiveType::Bool => ResolvedPrimitiveType::Bool,
                        },
                        target_span: cast.target_span,
                        source: Box::new(source),
                        span: cast.span,
                    },
                ))
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
            syntax::Expression::Allocation(allocation) => self.resolve_allocation(allocation),
            syntax::Expression::ArrayConstruction(construction) => {
                let array_type = self.resolve_type(&construction.array_type)?;
                let arguments = match &construction.arguments {
                    syntax::ArrayConstructionArguments::Empty {
                        left_paren_span,
                        right_paren_span,
                    } => ResolvedArrayConstructionArguments::Empty {
                        left_paren_span: *left_paren_span,
                        right_paren_span: *right_paren_span,
                    },
                    syntax::ArrayConstructionArguments::Length {
                        left_paren_span,
                        length,
                        right_paren_span,
                    } => ResolvedArrayConstructionArguments::Length {
                        left_paren_span: *left_paren_span,
                        length: Box::new(self.resolve_expression(length)?),
                        right_paren_span: *right_paren_span,
                    },
                    syntax::ArrayConstructionArguments::Copy {
                        left_paren_span,
                        copy_span,
                        source,
                        right_paren_span,
                    } => ResolvedArrayConstructionArguments::Copy {
                        left_paren_span: *left_paren_span,
                        copy_span: *copy_span,
                        source: Box::new(self.resolve_expression(source)?),
                        right_paren_span: *right_paren_span,
                    },
                    syntax::ArrayConstructionArguments::Elements(list) => {
                        let mut elements = Vec::with_capacity(list.elements.len());
                        let mut valid = true;
                        for element in &list.elements {
                            match self.resolve_expression(element) {
                                Some(element) => elements.push(element),
                                None => valid = false,
                            }
                        }
                        if !valid {
                            return None;
                        }
                        ResolvedArrayConstructionArguments::Elements(ResolvedArrayElementList {
                            left_brace_span: list.left_brace_span,
                            elements,
                            comma_spans: list.comma_spans.clone(),
                            right_brace_span: list.right_brace_span,
                        })
                    }
                };
                Some(ResolvedExpression::ArrayConstruction(Box::new(
                    ResolvedArrayConstructionExpr {
                        new_span: construction.new_span,
                        array_type,
                        arguments,
                        span: construction.span,
                    },
                )))
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
            syntax::Expression::ArrayProjection(projection) => {
                let receiver = Box::new(self.resolve_expression(&projection.receiver)?);
                let operator = match projection.operator {
                    syntax::ArrayProjectionOperator::Ordinary { left_bracket_span } => {
                        ResolvedArrayProjectionOperator::Ordinary { left_bracket_span }
                    }
                    syntax::ArrayProjectionOperator::Shared {
                        arrow_span,
                        left_bracket_span,
                    } => ResolvedArrayProjectionOperator::Shared {
                        arrow_span,
                        left_bracket_span,
                    },
                };
                let bounds = match &projection.bounds {
                    syntax::ArrayProjectionBounds::Index(index) => {
                        ResolvedArrayProjectionBounds::Index(Box::new(
                            self.resolve_expression(index)?,
                        ))
                    }
                    syntax::ArrayProjectionBounds::Slice {
                        start,
                        colon_span,
                        end,
                    } => ResolvedArrayProjectionBounds::Slice {
                        start: match start {
                            Some(bound) => Some(Box::new(self.resolve_expression(bound)?)),
                            None => None,
                        },
                        colon_span: *colon_span,
                        end: match end {
                            Some(bound) => Some(Box::new(self.resolve_expression(bound)?)),
                            None => None,
                        },
                    },
                };
                Some(ResolvedExpression::ArrayProjection(Box::new(
                    ResolvedArrayProjectionExpr {
                        receiver,
                        operator,
                        bounds,
                        right_bracket_span: projection.right_bracket_span,
                        span: projection.span,
                    },
                )))
            }
        }
    }

    fn resolve_identifier(
        &mut self,
        identifier: &syntax::IdentifierExpr,
    ) -> Option<ResolvedExpression> {
        if !identifier.name.is_qualified() {
            if let Some(symbol) = self.lookup_binding(&identifier.name.text) {
                return Some(ResolvedExpression::Binding(ResolvedBindingExpr {
                    binding: symbol.id,
                    span: identifier.span,
                }));
            }
        }
        match self
            .environment
            .lookup
            .select(&identifier.name, self.diagnostics)
        {
            TopLevelLookup::Found(TopLevelSymbol {
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
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::Class(_),
                ..
            }) => self.diagnostics.push(
                Diagnostic::error(
                    TOP_LEVEL_USED_AS_VALUE,
                    format!("class `{}` cannot be used as a value", identifier.name.text),
                )
                .with_primary_label(identifier.span, "construct it with `(...)`"),
            ),
            TopLevelLookup::Found(TopLevelSymbol {
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
            TopLevelLookup::Missing => {
                self.report_unknown(&identifier.name.text, identifier.span, "unknown name")
            }
            TopLevelLookup::Diagnosed => {}
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

    fn cover(&self, start: Span, end: Span) -> Span {
        assert_eq!(
            start.source_id(),
            end.source_id(),
            "resolved expression children must belong to one source"
        );
        Span::new(
            start.source_id(),
            TextRange::new(start.range().start(), end.range().end())
                .expect("resolved expression children must retain source order"),
        )
    }

    fn report_unknown(&mut self, name: &str, span: Span, kind: &'static str) {
        self.diagnostics.push(
            Diagnostic::error(UNKNOWN_NAME, format!("{kind} `{name}`"))
                .with_primary_label(span, "not declared in this scope"),
        );
    }
}
