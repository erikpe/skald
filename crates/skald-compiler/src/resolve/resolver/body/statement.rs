//! Statement sequencing, lexical scopes, and binding declarations.

use super::*;
use crate::identity::LocalId;

impl CallableResolver<'_, '_> {
    pub(super) fn resolve_block(&mut self, block: &syntax::Block, nested: bool) -> ResolvedBlock {
        if nested {
            self.scopes.push(HashMap::new());
        }
        let resolved = self.resolve_block_in_current_scope(block, !nested);
        if nested {
            self.scopes
                .pop()
                .expect("nested block must have a lexical scope");
        }
        resolved
    }

    fn resolve_block_in_current_scope(
        &mut self,
        block: &syntax::Block,
        allow_root_base_initialization: bool,
    ) -> ResolvedBlock {
        let statements = block
            .statements
            .iter()
            .enumerate()
            .filter_map(|(index, statement)| {
                self.resolve_statement(statement, allow_root_base_initialization && index == 0)
            })
            .collect();
        ResolvedBlock {
            statements,
            span: block.span,
        }
    }

    fn resolve_statement(
        &mut self,
        statement: &syntax::Statement,
        first_root_statement: bool,
    ) -> Option<ResolvedStatement> {
        match statement {
            syntax::Statement::BaseInitialization(statement) => self
                .resolve_base_initialization(statement, first_root_statement)
                .map(ResolvedStatement::BaseInitialization),
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
            syntax::Statement::ObjectAssignment(assignment) => {
                self.resolve_object_assignment(assignment)
            }
        }
    }

    fn resolve_base_initialization(
        &mut self,
        statement: &syntax::BaseInitializationStatement,
        first_root_statement: bool,
    ) -> Option<ResolvedBaseInitialization> {
        let target = match self.base_initialization {
            BaseInitializationPolicy::Required { base } if first_root_statement => {
                let base_declaration = self
                    .environment
                    .classes
                    .get(base)
                    .expect("resolved direct base must exist");
                if base_declaration.initializers.is_empty() {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_BASE_INITIALIZATION,
                            format!(
                                "base class `{}` has no ordinary initializer",
                                base_declaration.name
                            ),
                        )
                        .with_primary_label(
                            statement.super_span,
                            "this base initialization has no callable target",
                        ),
                    );
                    None
                } else {
                    Some(base)
                }
            }
            BaseInitializationPolicy::Required { .. } | BaseInitializationPolicy::Forbidden => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_BASE_INITIALIZATION,
                        "`super(...)` is allowed only as the first statement of a derived ordinary initializer",
                    )
                    .with_primary_label(statement.super_span, "misplaced base initialization"),
                );
                None
            }
        };

        let mut arguments = Vec::with_capacity(statement.arguments.len());
        let mut valid = true;
        for argument in &statement.arguments {
            match self.resolve_expression(argument) {
                Some(argument) => arguments.push(argument),
                None => valid = false,
            }
        }
        match (target, valid) {
            (Some(base), true) => Some(ResolvedBaseInitialization {
                base,
                arguments,
                super_span: statement.super_span,
                span: statement.span,
            }),
            _ => None,
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
                name: local.name.text.to_string(),
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

    fn resolve_field_assignment(
        &mut self,
        assignment: &syntax::FieldAssignmentStatement,
    ) -> Option<ResolvedFieldAssignment> {
        let receiver = self.resolve_member_object_receiver(&assignment.place);
        let selected = receiver.and_then(|receiver| {
            self.select_member(receiver.class(), &assignment.place.member)
                .map(|member| {
                    let receiver = self
                        .project_receiver_to_declaring_class(receiver, member.declaring_class());
                    (receiver, member)
                })
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

    fn resolve_object_assignment(
        &mut self,
        assignment: &syntax::ObjectAssignmentStatement,
    ) -> Option<ResolvedStatement> {
        if let Some(operator_span) = dereference_operator_through_groups(&assignment.place) {
            if self.resolve_expression(&assignment.place).is_some() {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_POINTEE_ASSIGNMENT,
                        "a complete shared pointee cannot be replaced",
                    )
                    .with_primary_label(operator_span, "whole-pointee assignment is not supported")
                    .with_secondary_label(
                        assignment.equal_span,
                        "replace the shared owner or assign one of the pointee's fields",
                    ),
                );
            }
            return None;
        }

        if let Some(place) = terminal_member_through_groups(&assignment.place) {
            let field_assignment = syntax::FieldAssignmentStatement {
                place: place.clone(),
                equal_span: assignment.equal_span,
                value: assignment.value.clone(),
                span: assignment.span,
            };
            return self
                .resolve_field_assignment(&field_assignment)
                .map(ResolvedStatement::FieldAssignment);
        }

        if let Some(identifier) = binding_identifier_through_groups(&assignment.place) {
            if let Some(binding) = self.lookup_binding(&identifier.name.text) {
                if matches!(
                    binding.ty,
                    ResolvedTypeKind::I64
                        | ResolvedTypeKind::U64
                        | ResolvedTypeKind::U8
                        | ResolvedTypeKind::F64
                        | ResolvedTypeKind::Bool
                ) {
                    let BindingId::Local(destination) = binding.id else {
                        self.diagnostics.push(
                            Diagnostic::error(
                                INVALID_LOCAL_ASSIGNMENT,
                                "a primitive value parameter cannot be reassigned",
                            )
                            .with_primary_label(
                                identifier.name.span,
                                "only a primitive `var` local is replaceable",
                            )
                            .with_secondary_label(binding.name_span, "parameter declared here"),
                        );
                        let _ = self.resolve_expression(&assignment.value);
                        return None;
                    };
                    let source = self.resolve_expression(&assignment.value);
                    return source.map(|source| {
                        ResolvedStatement::PrimitiveLocalAssignment(
                            ResolvedPrimitiveLocalAssignment {
                                destination,
                                equal_span: assignment.equal_span,
                                source,
                                span: assignment.span,
                            },
                        )
                    });
                }
                if matches!(binding.ty, ResolvedTypeKind::Array(_)) {
                    let destination = self.resolve_expression(&assignment.place)?;
                    let source = self.resolve_expression(&assignment.value)?;
                    return Some(ResolvedStatement::ArrayAssignment(
                        ResolvedArrayAssignment {
                            destination,
                            equal_span: assignment.equal_span,
                            source,
                            span: assignment.span,
                        },
                    ));
                }
                if let ResolvedTypeKind::Shared(target) = binding.ty {
                    let source = self.resolve_expression(&assignment.value)?;
                    return Some(ResolvedStatement::SharedAssignment(
                        ResolvedSharedAssignment {
                            destination: binding.id,
                            target,
                            equal_span: assignment.equal_span,
                            source,
                            span: assignment.span,
                        },
                    ));
                }
                if matches!(
                    binding.ty,
                    ResolvedTypeKind::Optional { .. } | ResolvedTypeKind::OptionalShared { .. }
                ) {
                    let source = self.resolve_expression(&assignment.value)?;
                    return Some(ResolvedStatement::OptionalAssignment(
                        ResolvedOptionalAssignment {
                            destination: binding.id,
                            target: binding.ty,
                            equal_span: assignment.equal_span,
                            source,
                            span: assignment.span,
                        },
                    ));
                }
            }
        }

        if is_array_projection_through_groups(&assignment.place) {
            let destination = self.resolve_expression(&assignment.place)?;
            let source = self.resolve_expression(&assignment.value)?;
            return Some(ResolvedStatement::ArrayAssignment(
                ResolvedArrayAssignment {
                    destination,
                    equal_span: assignment.equal_span,
                    source,
                    span: assignment.span,
                },
            ));
        }

        let destination = self.resolve_object_place(&assignment.place);
        let source = self.resolve_expression(&assignment.value);
        match (destination, source) {
            (Some(destination), Some(source)) => Some(ResolvedStatement::ObjectAssignment(
                ResolvedObjectAssignment {
                    destination,
                    equal_span: assignment.equal_span,
                    source,
                    span: assignment.span,
                },
            )),
            _ => None,
        }
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
}

fn is_array_projection_through_groups(expression: &syntax::Expression) -> bool {
    match expression {
        syntax::Expression::ArrayProjection(_) => true,
        syntax::Expression::Grouped(grouped) => {
            is_array_projection_through_groups(&grouped.expression)
        }
        _ => false,
    }
}

fn dereference_operator_through_groups(expression: &syntax::Expression) -> Option<Span> {
    match expression {
        syntax::Expression::Unary(unary)
            if unary.operator == syntax::UnaryOperator::Dereference =>
        {
            Some(unary.operator_span)
        }
        syntax::Expression::Grouped(grouped) => {
            dereference_operator_through_groups(&grouped.expression)
        }
        _ => None,
    }
}

fn binding_identifier_through_groups(
    expression: &syntax::Expression,
) -> Option<&syntax::IdentifierExpr> {
    match expression {
        syntax::Expression::Identifier(identifier) => Some(identifier),
        syntax::Expression::Grouped(grouped) => {
            binding_identifier_through_groups(&grouped.expression)
        }
        _ => None,
    }
}

fn terminal_member_through_groups(
    expression: &syntax::Expression,
) -> Option<&syntax::MemberAccessExpr> {
    match expression {
        syntax::Expression::Grouped(grouped) => terminal_member_through_groups(&grouped.expression),
        syntax::Expression::MemberAccess(member) => Some(member),
        _ => None,
    }
}
