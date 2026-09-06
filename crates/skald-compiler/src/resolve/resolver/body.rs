//! Callable-body resolution facade and shared expression/name resolution.

use std::{cell::RefCell, collections::HashMap};

use super::*;
use crate::{
    diagnostics::Diagnostic,
    identity::{
        BindingId, CallableId, ClassId, FieldId, FunctionTypeId, InterfaceId,
        InterfaceRequirementId, LiteralDataId, LocalId, LoopId, MethodId, ModuleId, StaticFieldId,
    },
    source::{Span, TextRange},
};

mod allocation;
mod call;
mod dereference;
mod indirect_call;
mod iteration;
mod operator;
mod place;
mod range;
mod statement;
mod structural_bracket;

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
    language_items: BodyLanguageItemEnvironment<'program>,
    specialization: Option<BodySpecializationEnvironment<'program>>,
    range_requests: Option<&'program SemanticRangeRequestCollector>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SemanticRangeRequest {
    pub(super) module: ModuleId,
    pub(super) endpoint: ResolvedTypeKind,
    pub(super) span: Span,
}

#[derive(Default)]
pub(super) struct SemanticRangeRequestCollector {
    requests: RefCell<Vec<SemanticRangeRequest>>,
}

impl SemanticRangeRequestCollector {
    pub(super) fn record(&self, request: SemanticRangeRequest) {
        let mut requests = self.requests.borrow_mut();
        if !requests.contains(&request) {
            requests.push(request);
        }
    }

    pub(super) fn into_requests(self) -> Vec<SemanticRangeRequest> {
        self.requests.into_inner()
    }
}

#[derive(Clone, Copy)]
pub(super) struct BodyDeclarationEnvironment<'program> {
    pub(super) functions: &'program ResolvedFunctionDeclarationTable,
    pub(super) classes: &'program ResolvedClassDeclarationTable,
    pub(super) interfaces: &'program ResolvedInterfaceDeclarationTable,
    pub(super) hierarchy: &'program ResolvedClassHierarchy,
}

impl<'program> BodyDeclarationEnvironment<'program> {
    pub(super) const fn new(
        functions: &'program ResolvedFunctionDeclarationTable,
        classes: &'program ResolvedClassDeclarationTable,
        interfaces: &'program ResolvedInterfaceDeclarationTable,
        hierarchy: &'program ResolvedClassHierarchy,
    ) -> Self {
        Self {
            functions,
            classes,
            interfaces,
            hierarchy,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct BodyLanguageItemEnvironment<'program> {
    string_literals: StringLiteralResolutionEnvironment<'program>,
    iteration: Option<IterationResolutionEnvironment<'program>>,
    operators: Option<OperatorResolutionEnvironment<'program>>,
    range: Option<RangeResolutionEnvironment<'program>>,
}

impl<'program> BodyLanguageItemEnvironment<'program> {
    pub(super) const fn new(
        string_literals: StringLiteralResolutionEnvironment<'program>,
        iteration: Option<IterationResolutionEnvironment<'program>>,
        operators: Option<OperatorResolutionEnvironment<'program>>,
        range: Option<RangeResolutionEnvironment<'program>>,
    ) -> Self {
        Self {
            string_literals,
            iteration,
            operators,
            range,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct RangeResolutionEnvironment<'program> {
    language_item: &'program ResolvedRangeLanguageItem,
    applications: &'program GenericInterfaceSpecializationTable,
}

impl<'program> RangeResolutionEnvironment<'program> {
    pub(super) const fn new(
        language_item: &'program ResolvedRangeLanguageItem,
        applications: &'program GenericInterfaceSpecializationTable,
    ) -> Self {
        Self {
            language_item,
            applications,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct OperatorResolutionEnvironment<'program> {
    language_item: &'program ResolvedOperatorLanguageItem,
    applications: &'program GenericInterfaceSpecializationTable,
}

impl<'program> OperatorResolutionEnvironment<'program> {
    pub(super) const fn new(
        language_item: &'program ResolvedOperatorLanguageItem,
        applications: &'program GenericInterfaceSpecializationTable,
    ) -> Self {
        Self {
            language_item,
            applications,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct IterationResolutionEnvironment<'program> {
    language_item: &'program ResolvedIterableLanguageItem,
    applications: &'program GenericInterfaceSpecializationTable,
}

impl<'program> IterationResolutionEnvironment<'program> {
    pub(super) const fn new(
        language_item: &'program ResolvedIterableLanguageItem,
        applications: &'program GenericInterfaceSpecializationTable,
    ) -> Self {
        Self {
            language_item,
            applications,
        }
    }
}

/// Closed template information consulted only while resolving a generated
/// class body. Ordinary bodies continue through the same resolver with no
/// specialization environment.
#[derive(Clone, Copy)]
pub(super) struct BodySpecializationEnvironment<'program> {
    semantics: &'program ResolvedClassTemplateSemantics,
    specialization: &'program GenericSpecialization,
}

#[derive(Clone, Copy)]
enum SpecializedBoundMember {
    Closed(ClosedGenericBoundMember),
    /// Template resolution selected this bound member, so ordinary member
    /// lookup must not reinterpret a failed closed witness and emit a cascade.
    Unavailable,
}

#[derive(Clone, Copy)]
struct SpecializedRangeSelection {
    class: Option<ClassId>,
    endpoint_provenance: [ResolvedRangeEndpointProvenance; 2],
}

impl<'program> BodySpecializationEnvironment<'program> {
    pub(super) const fn new(
        semantics: &'program ResolvedClassTemplateSemantics,
        specialization: &'program GenericSpecialization,
    ) -> Self {
        Self {
            semantics,
            specialization,
        }
    }

    fn closed_type(self, span: Span) -> Option<ResolvedTypeKind> {
        self.semantics
            .type_uses
            .iter()
            .zip(&self.specialization.closed_type_uses)
            .find_map(|(type_use, closed)| (type_use.type_term.span == span).then_some(*closed))
            .flatten()
    }

    fn bound_member(self, span: Span) -> Option<SpecializedBoundMember> {
        self.semantics
            .selections
            .iter()
            .zip(&self.specialization.closed_bound_members)
            .find_map(|(selection, closed)| {
                let ResolvedTemplateSelection::BoundMember {
                    span: selection_span,
                    ..
                } = selection
                else {
                    return None;
                };
                if *selection_span != span {
                    return None;
                }
                Some(closed.map_or(
                    SpecializedBoundMember::Unavailable,
                    SpecializedBoundMember::Closed,
                ))
            })
    }

    fn range_selection(self, span: Span) -> Option<SpecializedRangeSelection> {
        self.semantics
            .selections
            .iter()
            .zip(&self.specialization.closed_range_selections)
            .find_map(|(selection, closed)| {
                let ResolvedTemplateSelection::Range {
                    endpoint_provenance,
                    span: selection_span,
                    ..
                } = selection
                else {
                    return None;
                };
                if *selection_span != span {
                    return None;
                }
                Some(SpecializedRangeSelection {
                    class: *closed,
                    endpoint_provenance: *endpoint_provenance,
                })
            })
    }

    fn operator_selection(self, span: Span) -> Option<ClosedGenericOperatorSelection> {
        self.semantics
            .selections
            .iter()
            .zip(&self.specialization.closed_operator_selections)
            .find_map(|(selection, closed)| {
                let ResolvedTemplateSelection::Operator(selection) = selection else {
                    return None;
                };
                (selection.span == span).then_some(*closed).flatten()
            })
    }

    fn iteration_selection(self, span: Span) -> Option<ResolvedIterableSelection> {
        self.semantics
            .selections
            .iter()
            .zip(&self.specialization.closed_iteration_selections)
            .find_map(|(selection, closed)| {
                let ResolvedTemplateSelection::Iteration {
                    span: selection_span,
                    ..
                } = selection
                else {
                    return None;
                };
                if *selection_span != span {
                    return None;
                }
                closed.map(|closed| ResolvedIterableSelection {
                    interface: closed.interface,
                    iter_state: closed.iter_state,
                    iter_next: closed.iter_next,
                    item: closed.item,
                    state: closed.state,
                    origin_span: closed.origin_span,
                })
            })
    }
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
        declarations: BodyDeclarationEnvironment<'program>,
        has_module_context: bool,
        language_items: BodyLanguageItemEnvironment<'program>,
    ) -> Self {
        Self {
            lookup,
            functions: declarations.functions,
            classes: declarations.classes,
            interfaces: declarations.interfaces,
            hierarchy: declarations.hierarchy,
            has_module_context,
            language_items,
            specialization: None,
            range_requests: None,
        }
    }

    pub(super) const fn with_specialization(
        mut self,
        specialization: BodySpecializationEnvironment<'program>,
    ) -> Self {
        self.specialization = Some(specialization);
        self
    }

    pub(super) const fn with_range_request_collector(
        mut self,
        collector: &'program SemanticRangeRequestCollector,
    ) -> Self {
        self.range_requests = Some(collector);
        self
    }
}

pub(super) fn resolve_callable_body(
    context: CallableResolutionContext,
    parameters: &[ResolvedParameter],
    body: &syntax::Block,
    environment: BodyResolutionEnvironment<'_>,
    type_interner: &mut ResolvedTypeInterner,
    address_taken_callables: &mut ResolvedAddressTakenCallableTable,
    diagnostics: &mut Diagnostics,
) -> ResolvedCallableBody {
    CallableResolver::new(
        context,
        parameters,
        environment,
        type_interner,
        address_taken_callables,
        diagnostics,
    )
    .resolve(body)
}

pub(super) fn resolve_static_initializer_expression(
    context: CallableResolutionContext,
    expression: &syntax::Expression,
    environment: BodyResolutionEnvironment<'_>,
    type_interner: &mut ResolvedTypeInterner,
    address_taken_callables: &mut ResolvedAddressTakenCallableTable,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedExpression> {
    CallableResolver::new(
        context,
        &[],
        environment,
        type_interner,
        address_taken_callables,
        diagnostics,
    )
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
    type_interner: &'state mut ResolvedTypeInterner,
    address_taken_callables: &'state mut ResolvedAddressTakenCallableTable,
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
        type_interner: &'state mut ResolvedTypeInterner,
        address_taken_callables: &'state mut ResolvedAddressTakenCallableTable,
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
            type_interner,
            address_taken_callables,
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

    fn resolve_view_target(&mut self, named: &syntax::NamedTypeSyntax) -> Option<ResolvedType> {
        self.resolve_type(&syntax::TypeSyntax {
            kind: syntax::TypeKind::Named(named.clone()),
            span: named.span,
        })
    }

    fn specialized_class(&mut self, named: &syntax::NamedTypeSyntax) -> Option<ClassId> {
        if let Some(specialization) = self.environment.specialization {
            let kind = specialization.closed_type(named.span)?;
            let ResolvedTypeKind::Class(class) = kind else {
                return None;
            };
            return Some(class);
        }

        let class = self.environment.lookup.specialized_class(named.span)?;
        super::report_generic_application(named, self.environment.lookup, self.diagnostics);
        Some(class)
    }

    fn specialized_interface(&mut self, named: &syntax::NamedTypeSyntax) -> Option<InterfaceId> {
        if let Some(specialization) = self.environment.specialization {
            let kind = specialization.closed_type(named.span)?;
            let ResolvedTypeKind::Interface(interface) = kind else {
                return None;
            };
            return Some(interface);
        }

        let interface = self.environment.lookup.specialized_interface(named.span)?;
        super::report_generic_application(named, self.environment.lookup, self.diagnostics);
        Some(interface)
    }

    pub(super) fn report_unsupported_generic_application(
        &mut self,
        named: &syntax::NamedTypeSyntax,
    ) {
        super::report_generic_application(named, self.environment.lookup, self.diagnostics);
    }

    fn resolve_type(&mut self, type_syntax: &syntax::TypeSyntax) -> Option<ResolvedType> {
        if let Some(kind) = self
            .environment
            .specialization
            .and_then(|specialization| specialization.closed_type(type_syntax.span))
        {
            return Some(ResolvedType {
                kind,
                span: type_syntax.span,
            });
        }
        super::resolve_type(
            type_syntax,
            self.environment.lookup,
            self.type_interner,
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
            syntax::Expression::Present(present) => {
                let value = self.resolve_expression(&present.value)?;
                Some(ResolvedExpression::Present(ResolvedPresentExpr {
                    value: Box::new(value),
                    some_span: present.some_span,
                    span: present.span,
                }))
            }
            syntax::Expression::Identifier(identifier) => self.resolve_identifier(identifier),
            syntax::Expression::GenericTypeApplication(application) => {
                if let Some(class) = self.specialized_class(&application.target) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            TOP_LEVEL_USED_AS_VALUE,
                            "an applied generic class cannot be used as a value",
                        )
                        .with_primary_label(application.span, "construct it with an argument list")
                        .with_secondary_label(
                            self.environment
                                .classes
                                .get(class)
                                .expect("specialized body target must name a generated class")
                                .name_span,
                            "generic class declared here",
                        ),
                    );
                } else {
                    self.report_unsupported_generic_application(&application.target);
                }
                None
            }
            syntax::Expression::GenericStaticSelection(selection) => {
                self.resolve_specialized_static_value(selection)
            }
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
                let item = self
                    .environment
                    .language_items
                    .string_literals
                    .language_item?;
                let data = *self
                    .environment
                    .language_items
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
                let operator = match unary.operator {
                    syntax::UnaryOperator::Negate => ResolvedUnaryOperator::Negate,
                    syntax::UnaryOperator::LogicalNot => ResolvedUnaryOperator::LogicalNot,
                    syntax::UnaryOperator::BitwiseComplement => {
                        ResolvedUnaryOperator::BitwiseComplement
                    }
                    syntax::UnaryOperator::Dereference => {
                        unreachable!("dereference returned above")
                    }
                };
                let selection = operator.protocol().and_then(|protocol| {
                    self.specialized_operator_selection(unary.span, protocol)
                        .unwrap_or_else(|| self.select_unary_operator(operator, &operand))
                });
                Some(ResolvedExpression::Unary(ResolvedUnaryExpr {
                    operator,
                    operator_span: unary.operator_span,
                    operand: Box::new(operand),
                    selection,
                    span: unary.span,
                }))
            }
            syntax::Expression::Binary(binary) => {
                let left = self.resolve_expression(&binary.left);
                let right = self.resolve_expression(&binary.right);
                match (left, right) {
                    (Some(left), Some(right)) => {
                        let operator = match binary.operator {
                            syntax::BinaryOperator::Add => ResolvedBinaryOperator::Add,
                            syntax::BinaryOperator::Subtract => ResolvedBinaryOperator::Subtract,
                            syntax::BinaryOperator::Multiply => ResolvedBinaryOperator::Multiply,
                            syntax::BinaryOperator::Divide => ResolvedBinaryOperator::Divide,
                            syntax::BinaryOperator::Remainder => ResolvedBinaryOperator::Remainder,
                            syntax::BinaryOperator::ShiftLeft => ResolvedBinaryOperator::ShiftLeft,
                            syntax::BinaryOperator::ShiftRight => {
                                ResolvedBinaryOperator::ShiftRight
                            }
                            syntax::BinaryOperator::BitwiseAnd => {
                                ResolvedBinaryOperator::BitwiseAnd
                            }
                            syntax::BinaryOperator::BitwiseOr => ResolvedBinaryOperator::BitwiseOr,
                            syntax::BinaryOperator::BitwiseXor => {
                                ResolvedBinaryOperator::BitwiseXor
                            }
                            syntax::BinaryOperator::Equal => ResolvedBinaryOperator::Equal,
                            syntax::BinaryOperator::NotEqual => ResolvedBinaryOperator::NotEqual,
                            syntax::BinaryOperator::LessThan => ResolvedBinaryOperator::LessThan,
                            syntax::BinaryOperator::LessEqual => ResolvedBinaryOperator::LessEqual,
                            syntax::BinaryOperator::GreaterThan => {
                                ResolvedBinaryOperator::GreaterThan
                            }
                            syntax::BinaryOperator::GreaterEqual => {
                                ResolvedBinaryOperator::GreaterEqual
                            }
                        };
                        let selection = self
                            .specialized_operator_selection(binary.span, operator.protocol())
                            .unwrap_or_else(|| {
                                self.select_binary_operator(operator, &left, &right)
                            });
                        Some(ResolvedExpression::Binary(ResolvedBinaryExpr {
                            left: Box::new(left),
                            operator,
                            operator_span: binary.operator_span,
                            right: Box::new(right),
                            selection,
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
                if self.is_grouped_function_value_cast(cast) {
                    let _ = self.resolve_expression(&cast.source);
                    self.report_grouped_function_value_call(cast);
                    return None;
                }
                let source = self.resolve_expression(&cast.source);
                let target = self.resolve_view_target(&cast.target);
                match (source, target) {
                    (Some(source), Some(target)) => {
                        let target_mode = match cast.target_mode {
                            syntax::ObjectCastTargetMode::Plain => {
                                ResolvedObjectCastTargetMode::Plain
                            }
                            syntax::ObjectCastTargetMode::Shared { shared_span } => {
                                ResolvedObjectCastTargetMode::Shared { shared_span }
                            }
                        };
                        let optional_box_target =
                            if matches!(target_mode, ResolvedObjectCastTargetMode::Shared { .. }) {
                                let optional_depth = match self.resolved_shared_target(&source) {
                                    Some(ResolvedSharedTarget::OptionalBox(source_target)) => self
                                        .type_interner
                                        .optional_box(source_target)
                                        .map(|metadata| metadata.optional_depth),
                                    _ => None,
                                };
                                let object_leaf = match target.kind {
                                    ResolvedTypeKind::Class(class) => {
                                        Some(ResolvedObjectTarget::Class(class))
                                    }
                                    ResolvedTypeKind::Interface(interface) => {
                                        Some(ResolvedObjectTarget::Interface(interface))
                                    }
                                    ResolvedTypeKind::Obj => Some(ResolvedObjectTarget::Obj),
                                    _ => None,
                                };
                                optional_depth.zip(object_leaf).map(|(depth, leaf)| {
                                    self.type_interner.intern_optional_object_box_cast_target(
                                        depth,
                                        leaf,
                                        cast.target.span,
                                    )
                                })
                            } else {
                                None
                            };
                        Some(ResolvedExpression::ObjectCast(ResolvedObjectCastExpr {
                            source: Box::new(source),
                            target,
                            target_mode,
                            optional_box_target,
                            target_span: cast.target.span,
                            span: cast.span,
                        }))
                    }
                    _ => None,
                }
            }
            syntax::Expression::Allocation(allocation) => self.resolve_allocation(allocation),
            syntax::Expression::OptionalBoxAllocation(allocation) => {
                self.resolve_optional_box_allocation(allocation)
            }
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
                    syntax::ArrayConstructionArguments::Indexed(initializer) => {
                        let length = self.resolve_expression(&initializer.length);
                        let binding = self.resolve_indexed_array_binding(initializer);
                        let element = self.resolve_expression(&initializer.element);
                        self.scopes
                            .pop()
                            .expect("an indexed array initializer owns one lexical scope");
                        ResolvedArrayConstructionArguments::Indexed(
                            ResolvedIndexedArrayInitializer {
                                left_paren_span: initializer.left_paren_span,
                                length: Box::new(length?),
                                semicolon_span: initializer.semicolon_span,
                                binding,
                                arrow_span: initializer.arrow_span,
                                element: Box::new(element?),
                                right_paren_span: initializer.right_paren_span,
                            },
                        )
                    }
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
            syntax::Expression::BracketProjection(projection) => {
                self.resolve_bracket_projection(projection)
            }
        }
    }

    fn resolve_indexed_array_binding(
        &mut self,
        initializer: &syntax::IndexedArrayInitializer,
    ) -> ResolvedLocal {
        self.begin_scoped_local_binding(
            &initializer.binding,
            ResolvedType {
                kind: ResolvedTypeKind::I64,
                span: initializer.binding.span,
            },
            "indexed array binding",
        )
    }

    /// Declares an immutable-by-construction local in a fresh lexical scope.
    ///
    /// The owning construct decides when to leave the scope and, during type
    /// checking, whether assignment is legal. Resolution only centralizes
    /// identity allocation, source metadata, and lexical visibility here.
    fn begin_scoped_local_binding(
        &mut self,
        name: &syntax::Name,
        ty: ResolvedType,
        binding_kind: &'static str,
    ) -> ResolvedLocal {
        let local = ResolvedLocal {
            id: LocalId::new(self.callable, self.locals.len()),
            name: name.text.to_string(),
            name_span: name.span,
            type_syntax: ty,
            span: name.span,
        };
        self.scopes.push(HashMap::new());
        let declared = self.declare_binding(
            &local.name,
            BindingSymbol {
                id: BindingId::Local(local.id),
                ty: local.type_syntax.kind,
                name_span: local.name_span,
            },
            binding_kind,
        );
        debug_assert!(declared, "a fresh scoped-local scope has no bindings");
        self.locals.push(local.clone());
        local
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
                kind: TopLevelSymbolKind::Function(function),
                ..
            }) => return self.resolve_top_level_function_reference(function, identifier.span),
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
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::ClassTemplate(_),
                ..
            }) => self.report_raw_generic_type(&identifier.name.text, identifier.span),
            TopLevelLookup::Found(TopLevelSymbol {
                kind: TopLevelSymbolKind::InterfaceTemplate(_),
                name_span,
            }) => self.diagnostics.push(
                Diagnostic::error(
                    RAW_GENERIC_TYPE,
                    format!(
                        "generic interface `{}` requires type arguments",
                        identifier.name.text
                    ),
                )
                .with_primary_label(identifier.span, "type arguments cannot be omitted")
                .with_secondary_label(name_span, "template declared here"),
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

    fn report_raw_generic_type(&mut self, name: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::error(
                RAW_GENERIC_TYPE,
                format!("generic class `{name}` requires type arguments"),
            )
            .with_primary_label(span, "supply the template's type arguments"),
        );
    }
}
