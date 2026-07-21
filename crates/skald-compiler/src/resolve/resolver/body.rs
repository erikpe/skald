//! Lexical binding and expression/member resolution for one callable body.

use super::*;
use crate::{
    diagnostics::Diagnostic,
    identity::{BindingId, CallableId, LocalId},
};

pub(super) struct ResolvedCallableBody {
    pub(super) locals: Vec<ResolvedLocal>,
    pub(super) body: ResolvedBlock,
}

#[derive(Clone, Copy)]
pub(super) struct BodyResolutionEnvironment<'program> {
    top_levels: &'program HashMap<String, TopLevelSymbol>,
    classes: &'program ResolvedClassDeclarationTable,
    class_symbols: &'program [ClassSymbols],
}

impl<'program> BodyResolutionEnvironment<'program> {
    pub(super) fn new(
        top_levels: &'program HashMap<String, TopLevelSymbol>,
        classes: &'program ResolvedClassDeclarationTable,
        class_symbols: &'program [ClassSymbols],
    ) -> Self {
        Self {
            top_levels,
            classes,
            class_symbols,
        }
    }
}

pub(super) fn resolve_callable_body(
    callable: CallableId,
    receiver_class: Option<ClassId>,
    parameters: &[ResolvedParameter],
    body: &syntax::Block,
    environment: BodyResolutionEnvironment<'_>,
    diagnostics: &mut Diagnostics,
) -> ResolvedCallableBody {
    CallableResolver::new(
        callable,
        receiver_class,
        parameters,
        environment,
        diagnostics,
    )
    .resolve(body)
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
    scopes: Vec<HashMap<String, BindingSymbol>>,
    locals: Vec<ResolvedLocal>,
}

impl<'program, 'diagnostics> CallableResolver<'program, 'diagnostics> {
    fn new(
        callable: CallableId,
        receiver_class: Option<ClassId>,
        parameters: &[ResolvedParameter],
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
            scopes: vec![parameters],
            locals: Vec::new(),
        }
    }

    fn resolve(mut self, body: &syntax::Block) -> ResolvedCallableBody {
        let body = self.resolve_block(body, false);
        ResolvedCallableBody {
            locals: self.locals,
            body,
        }
    }

    fn resolve_block(&mut self, block: &syntax::Block, nested: bool) -> ResolvedBlock {
        if nested {
            self.scopes.push(HashMap::new());
        }
        let statements = block
            .statements
            .iter()
            .filter_map(|statement| self.resolve_statement(statement))
            .collect();
        if nested {
            self.scopes
                .pop()
                .expect("nested block must have a lexical scope");
        }
        ResolvedBlock {
            statements,
            span: block.span,
        }
    }

    fn resolve_statement(&mut self, statement: &syntax::Statement) -> Option<ResolvedStatement> {
        match statement {
            syntax::Statement::Local(local) => {
                self.resolve_local(local).map(ResolvedStatement::Local)
            }
            syntax::Statement::Return(statement) => {
                let value = match &statement.value {
                    Some(value) => Some(self.resolve_expression(value)?),
                    None => None,
                };
                Some(ResolvedStatement::Return(ResolvedReturn {
                    value,
                    span: statement.span,
                }))
            }
            syntax::Statement::Expression(statement) => {
                let expression = self.resolve_expression(&statement.expression)?;
                Some(ResolvedStatement::Expression(ResolvedExpressionStatement {
                    expression,
                    span: statement.span,
                }))
            }
            syntax::Statement::Conditional(conditional) => self
                .resolve_conditional(conditional)
                .map(ResolvedStatement::Conditional),
            syntax::Statement::Block(block) => {
                Some(ResolvedStatement::Block(self.resolve_block(block, true)))
            }
            syntax::Statement::FieldAssignment(assignment) => self
                .resolve_field_assignment(assignment)
                .map(ResolvedStatement::FieldAssignment),
        }
    }

    fn resolve_conditional(
        &mut self,
        conditional: &syntax::ConditionalStatement,
    ) -> Option<ResolvedConditional> {
        let source_arms = std::iter::once(&conditional.if_arm).chain(&conditional.elif_arms);
        let mut arms = Vec::with_capacity(1 + conditional.elif_arms.len());
        let mut valid = true;
        for arm in source_arms {
            let condition = self.resolve_expression(&arm.condition);
            let body = self.resolve_block(&arm.body, true);
            match condition {
                Some(condition) => arms.push(ResolvedConditionalArm {
                    condition,
                    body,
                    span: arm.span,
                }),
                None => valid = false,
            }
        }
        let else_block = conditional
            .else_block
            .as_ref()
            .map(|block| self.resolve_block(block, true));
        valid.then_some(ResolvedConditional {
            arms,
            else_block,
            span: conditional.span,
        })
    }

    fn resolve_local(&mut self, local: &syntax::LocalDecl) -> Option<ResolvedLocalDecl> {
        // Resolve before declaration so a local never sees itself in either its
        // type or initializer. Type names use the top-level namespace directly.
        let ty = self.resolve_type(&local.type_syntax);
        let initializer = self.resolve_expression(&local.initializer);
        let ty = ty?;
        let id = LocalId::new(self.callable, self.locals.len());
        let symbol = BindingSymbol {
            id: BindingId::Local(id),
            ty: ty.kind,
            name_span: local.name.span,
        };
        let declared = self.declare_binding(&local.name.text, symbol, "local binding");
        if declared {
            self.locals.push(ResolvedLocal {
                id,
                name: local.name.text.clone(),
                name_span: local.name.span,
                type_syntax: ty,
                span: local.span,
            });
        }
        match (declared, initializer) {
            (true, Some(initializer)) => Some(ResolvedLocalDecl {
                local: id,
                initializer,
                span: local.span,
            }),
            _ => None,
        }
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

    fn resolve_field_access(
        &mut self,
        member: &syntax::MemberAccessExpr,
    ) -> Option<ResolvedExpression> {
        let receiver = self.resolve_object_place(&member.receiver)?;
        match self.select_member(receiver.class, &member.member)? {
            OrdinaryMemberSymbolKind::Field(field) => {
                Some(ResolvedExpression::FieldAccess(ResolvedFieldAccessExpr {
                    receiver,
                    field,
                    member_span: member.member.span,
                    span: member.span,
                }))
            }
            OrdinaryMemberSymbolKind::Method(method) => {
                let declaration = self
                    .environment
                    .classes
                    .get(method.class())
                    .and_then(|class| class.method(method))
                    .expect("member symbols must reference declaration metadata");
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_MEMBER_SELECTION,
                        format!("method `{}` cannot be used as a value", declaration.name),
                    )
                    .with_primary_label(member.member.span, "call the method with `(...)`")
                    .with_secondary_label(declaration.name_span, "method declared here"),
                );
                None
            }
        }
    }

    fn resolve_call(&mut self, call: &syntax::CallExpr) -> Option<ResolvedExpression> {
        let target = self.resolve_call_target(&call.callee);
        let mut arguments = Vec::with_capacity(call.arguments.len());
        let mut valid = true;
        for argument in &call.arguments {
            match self.resolve_expression(argument) {
                Some(argument) => arguments.push(argument),
                None => valid = false,
            }
        }
        let target = target?;
        if !valid {
            return None;
        }
        Some(match target {
            CallTarget::Function(function) => {
                ResolvedExpression::DirectCall(ResolvedDirectCallExpr {
                    function,
                    callee_span: call.callee.span(),
                    arguments,
                    span: call.span,
                })
            }
            CallTarget::Constructor { class, initializer } => {
                ResolvedExpression::Construct(ResolvedConstructExpr {
                    class,
                    initializer,
                    callee_span: call.callee.span(),
                    arguments,
                    span: call.span,
                })
            }
            CallTarget::Method {
                receiver,
                method,
                member_span,
            } => ResolvedExpression::MethodCall(ResolvedMethodCallExpr {
                receiver,
                method,
                member_span,
                arguments,
                span: call.span,
            }),
        })
    }

    fn resolve_call_target(&mut self, callee: &syntax::Expression) -> Option<CallTarget> {
        match callee {
            syntax::Expression::Identifier(identifier) => {
                if let Some(binding) = self.lookup_binding(&identifier.name.text) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_CALL_TARGET,
                            format!("binding `{}` is not callable", identifier.name.text),
                        )
                        .with_primary_label(identifier.span, "called here")
                        .with_secondary_label(binding.name_span, "binding declared here"),
                    );
                    return None;
                }
                match self
                    .environment
                    .top_levels
                    .get(&identifier.name.text)
                    .copied()
                {
                    Some(TopLevelSymbol {
                        kind: TopLevelSymbolKind::Function(function),
                        ..
                    }) => Some(CallTarget::Function(function)),
                    Some(TopLevelSymbol {
                        kind: TopLevelSymbolKind::Class(class),
                        ..
                    }) => {
                        let initializer = self.environment.class_symbols[class.index()]
                            .initializer
                            .or_else(|| {
                                self.diagnostics.push(
                                    Diagnostic::error(
                                        INVALID_CONSTRUCTION_TARGET,
                                        format!(
                                            "class `{}` has no initializer",
                                            identifier.name.text
                                        ),
                                    )
                                    .with_primary_label(
                                        identifier.span,
                                        "construction requires an explicit `init` declaration",
                                    ),
                                );
                                None
                            })?;
                        Some(CallTarget::Constructor { class, initializer })
                    }
                    None => {
                        self.report_unknown(
                            &identifier.name.text,
                            identifier.span,
                            "unknown function or class",
                        );
                        None
                    }
                }
            }
            syntax::Expression::MemberAccess(member) => {
                let receiver = self.resolve_object_place(&member.receiver)?;
                match self.select_member(receiver.class, &member.member)? {
                    OrdinaryMemberSymbolKind::Method(method) => Some(CallTarget::Method {
                        receiver,
                        method,
                        member_span: member.member.span,
                    }),
                    OrdinaryMemberSymbolKind::Field(field) => {
                        let declaration = self
                            .environment
                            .classes
                            .get(field.class())
                            .and_then(|class| class.field(field))
                            .expect("member symbols must reference declaration metadata");
                        self.diagnostics.push(
                            Diagnostic::error(
                                INVALID_CALL_TARGET,
                                format!("field `{}` is not callable", declaration.name),
                            )
                            .with_primary_label(member.member.span, "called here")
                            .with_secondary_label(declaration.name_span, "field declared here"),
                        );
                        None
                    }
                }
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(INVALID_CALL_TARGET, "invalid call target")
                        .with_primary_label(
                            callee.span(),
                            "expected a function, class, or ungrouped method selection",
                        ),
                );
                None
            }
        }
    }

    fn resolve_field_assignment(
        &mut self,
        assignment: &syntax::FieldAssignmentStatement,
    ) -> Option<ResolvedFieldAssignment> {
        let receiver = self.resolve_object_place(&assignment.place.receiver);
        let selected = receiver.and_then(|receiver| {
            self.select_member(receiver.class, &assignment.place.member)
                .map(|member| (receiver, member))
        });
        let value = self.resolve_expression(&assignment.value);
        let (receiver, selected, value) = match (selected, value) {
            (Some((receiver, selected)), Some(value)) => (receiver, selected, value),
            _ => return None,
        };
        let OrdinaryMemberSymbolKind::Field(field) = selected else {
            let OrdinaryMemberSymbolKind::Method(method) = selected else {
                unreachable!()
            };
            let declaration = self
                .environment
                .classes
                .get(method.class())
                .and_then(|class| class.method(method))
                .expect("member symbols must reference declaration metadata");
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_MEMBER_SELECTION,
                    format!("method `{}` cannot be assigned", declaration.name),
                )
                .with_primary_label(assignment.place.member.span, "expected a field here")
                .with_secondary_label(declaration.name_span, "method declared here"),
            );
            return None;
        };
        Some(ResolvedFieldAssignment {
            receiver,
            field,
            member_span: assignment.place.member.span,
            equal_span: assignment.equal_span,
            value,
            span: assignment.span,
        })
    }

    fn resolve_object_place(
        &mut self,
        expression: &syntax::Expression,
    ) -> Option<ResolvedObjectPlace> {
        match expression {
            syntax::Expression::Identifier(identifier) => {
                let Some(binding) = self.lookup_binding(&identifier.name.text) else {
                    self.report_unknown(&identifier.name.text, identifier.span, "unknown object");
                    return None;
                };
                let ResolvedTypeKind::Class(class) = binding.ty else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_MEMBER_SELECTION,
                            format!("binding `{}` is not an object", identifier.name.text),
                        )
                        .with_primary_label(identifier.span, "member access requires an object")
                        .with_secondary_label(binding.name_span, "binding declared here"),
                    );
                    return None;
                };
                Some(ResolvedObjectPlace {
                    binding: binding.id,
                    class,
                    span: identifier.span,
                })
            }
            syntax::Expression::SelfValue(self_value) => {
                let class = self.receiver_class.or_else(|| {
                    self.diagnostics.push(
                        Diagnostic::error(SELF_OUTSIDE_MEMBER, "`self` is not available here")
                            .with_primary_label(
                                self_value.span,
                                "only an initializer or instance method has `self`",
                            ),
                    );
                    None
                })?;
                Some(ResolvedObjectPlace {
                    binding: BindingId::Receiver(self.callable),
                    class,
                    span: self_value.span,
                })
            }
            syntax::Expression::Grouped(grouped) => {
                let mut place = self.resolve_object_place(&grouped.expression)?;
                place.span = grouped.span;
                Some(place)
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_MEMBER_SELECTION,
                        "member receiver must be an object place",
                    )
                    .with_primary_label(
                        expression.span(),
                        "expected an object local, `self`, or grouping around one",
                    ),
                );
                None
            }
        }
    }

    fn select_member(
        &mut self,
        class: ClassId,
        name: &syntax::Name,
    ) -> Option<OrdinaryMemberSymbolKind> {
        let symbols = &self.environment.class_symbols[class.index()];
        symbols
            .ordinary
            .get(&name.text)
            .map(|member| member.kind)
            .or_else(|| {
                let class_name = &self
                    .environment
                    .classes
                    .get(class)
                    .expect("resolved object place must reference a class")
                    .name;
                self.diagnostics.push(
                    Diagnostic::error(
                        UNKNOWN_MEMBER,
                        format!("class `{class_name}` has no member `{}`", name.text),
                    )
                    .with_primary_label(name.span, "unknown member"),
                );
                None
            })
    }

    fn declare_binding(
        &mut self,
        name: &str,
        symbol: BindingSymbol,
        binding_kind: &'static str,
    ) -> bool {
        let scope = self
            .scopes
            .last_mut()
            .expect("callable resolver must always have an active scope");
        if let Some(previous) = scope.get(name) {
            self.diagnostics.push(
                Diagnostic::error(
                    DUPLICATE_BINDING,
                    format!("duplicate {binding_kind} `{name}`"),
                )
                .with_primary_label(symbol.name_span, "redeclared here")
                .with_secondary_label(previous.name_span, "first declared here"),
            );
            return false;
        }
        scope.insert(name.to_owned(), symbol);
        true
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

enum CallTarget {
    Function(FunctionId),
    Constructor {
        class: ClassId,
        initializer: InitializerId,
    },
    Method {
        receiver: ResolvedObjectPlace,
        method: MethodId,
        member_span: Span,
    },
}
