//! Recursive object-place resolution and projected-member diagnostics.

use super::call::ClassReceiver;
use super::*;

impl CallableResolver<'_, '_> {
    pub(super) fn resolve_object_receiver(
        &mut self,
        expression: &syntax::Expression,
    ) -> Option<ResolvedObjectReceiver> {
        match expression {
            syntax::Expression::Unary(unary)
                if unary.operator == syntax::UnaryOperator::Dereference =>
            {
                let dereference = self.resolve_dereference(
                    &unary.operand,
                    ResolvedDereferenceOperator::Star,
                    unary.operator_span,
                    unary.span,
                )?;
                self.object_receiver_from_dereference(dereference)
            }
            syntax::Expression::ObjectCast(_) => {
                let resolved = self.resolve_expression(expression)?;
                let ResolvedExpression::ObjectCast(cast) = resolved else {
                    unreachable!("object-cast syntax must resolve to an object-cast expression")
                };
                if matches!(
                    cast.target_mode,
                    crate::resolve::ResolvedObjectCastTargetMode::Shared { .. }
                ) {
                    let target = match cast.target.kind {
                        ResolvedTypeKind::Class(class) => ResolvedSharedTarget::Class(class),
                        ResolvedTypeKind::Interface(interface) => {
                            ResolvedSharedTarget::Interface(interface)
                        }
                        ResolvedTypeKind::Obj => ResolvedSharedTarget::Obj,
                        _ => unreachable!("shared cast target must be an object view"),
                    };
                    self.report_implicit_shared_member_access(cast.span, target);
                    return None;
                }
                let ResolvedTypeKind::Class(class) = cast.target.kind else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_MEMBER_SELECTION,
                            "ordinary member selection requires a class cast target",
                        )
                        .with_primary_label(
                            cast.target_span,
                            "this target does not declare class fields or methods",
                        ),
                    );
                    return None;
                };
                Some(ResolvedObjectReceiver::from_cast(cast, class))
            }
            syntax::Expression::Unwrap(_) => {
                let resolved = self.resolve_expression(expression)?;
                let ResolvedExpression::Unwrap(unwrap) = resolved else {
                    unreachable!("unwrap syntax must resolve to an unwrap expression")
                };
                let Some(class) = self.resolved_optional_class(&unwrap.source) else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_MEMBER_SELECTION,
                            "member selection requires an inline class payload",
                        )
                        .with_primary_label(
                            unwrap.span,
                            "this unwrap does not produce an inline class place",
                        ),
                    );
                    return None;
                };
                Some(ResolvedObjectReceiver::from_optional_payload(unwrap, class))
            }
            syntax::Expression::Grouped(grouped) => Some(
                self.resolve_object_receiver(&grouped.expression)?
                    .with_span(grouped.span),
            ),
            syntax::Expression::BracketProjection(_) => {
                let resolved = self.resolve_expression(expression)?;
                match resolved {
                    ResolvedExpression::ArrayProjection(projection) => {
                        let Some(ResolvedTypeKind::Class(class)) = self.resolved_expression_type(
                            &ResolvedExpression::ArrayProjection(projection.clone()),
                        ) else {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    INVALID_MEMBER_SELECTION,
                                    "member selection requires an exact-class array element",
                                )
                                .with_primary_label(
                                    projection.span,
                                    "this projected element is not an inline class",
                                ),
                            );
                            return None;
                        };
                        let span = projection.span;
                        Some(ResolvedObjectReceiver::ArrayElement {
                            projection,
                            projections: Vec::new(),
                            class,
                            span,
                        })
                    }
                    producer @ (ResolvedExpression::MethodCall(_)
                    | ResolvedExpression::InterfaceCall(_)) => {
                        if let Some(target) = self.resolved_shared_target(&producer) {
                            self.report_implicit_shared_member_access(expression.span(), target);
                            return None;
                        }
                        let Some(ResolvedTypeKind::Class(class)) =
                            self.resolved_expression_type(&producer)
                        else {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    INVALID_MEMBER_SELECTION,
                                    "bracket result is not an exact inline class",
                                )
                                .with_primary_label(
                                    expression.span(),
                                    "only an exact-class result can be a produced member receiver",
                                ),
                            );
                            return None;
                        };
                        Some(ResolvedObjectReceiver::from_produced(producer, class))
                    }
                    _ => unreachable!(
                        "bracket projection syntax must resolve to an array projection or structural call"
                    ),
                }
            }
            syntax::Expression::StringLiteral(_) => {
                let producer = self.resolve_expression(expression)?;
                let ResolvedExpression::StringLiteral(literal) = producer else {
                    unreachable!("string-literal syntax must retain its resolved node")
                };
                Some(ResolvedObjectReceiver::from_produced(
                    ResolvedExpression::StringLiteral(literal),
                    literal.class,
                ))
            }
            syntax::Expression::Allocation(_) => {
                let source = self.resolve_expression(expression)?;
                let ResolvedExpression::Allocation(allocation) = &source else {
                    unreachable!("allocation syntax must resolve as allocation")
                };
                self.report_implicit_shared_member_access(
                    expression.span(),
                    ResolvedSharedTarget::Class(allocation.class),
                );
                None
            }
            syntax::Expression::Call(_) => {
                let producer = self.resolve_expression(expression)?;
                if let Some(target) = self.resolved_shared_target(&producer) {
                    self.report_implicit_shared_member_access(expression.span(), target);
                    return None;
                }
                let Some(ResolvedTypeKind::Class(class)) = self.resolved_expression_type(&producer)
                else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_MEMBER_SELECTION,
                            "call result is not an exact inline class",
                        )
                        .with_primary_label(
                            expression.span(),
                            "only an exact-class result can be a produced member receiver",
                        ),
                    );
                    return None;
                };
                Some(ResolvedObjectReceiver::from_produced(producer, class))
            }
            syntax::Expression::MemberAccess(member) => {
                if matches!(member.operator, syntax::MemberAccessOperator::Dot { .. }) {
                    match self.class_receiver(&member.receiver) {
                        ClassReceiver::Class(_) => {
                            let resolved = self.resolve_expression(expression)?;
                            let ResolvedExpression::StaticFieldAccess(access) = resolved else {
                                unreachable!(
                                    "class-selected object receiver must be a static field"
                                )
                            };
                            return self.object_receiver_from_static_field_access(access);
                        }
                        ClassReceiver::Diagnosed => return None,
                        ClassReceiver::NotClass => {}
                    }
                }
                let receiver = self.resolve_member_object_receiver(member)?;
                let selected = self.select_member(receiver.class(), &member.member)?;
                let receiver =
                    self.project_receiver_to_declaring_class(receiver, selected.declaring_class());
                match selected {
                    SelectedClassMember::Field(field) => self.project_receiver_field(
                        receiver,
                        field,
                        member.span,
                        member.member.span,
                    ),
                    SelectedClassMember::Method(method) => {
                        let declaration = self
                            .environment
                            .classes
                            .get(method.class())
                            .and_then(|class| class.method(method))
                            .expect("member symbols must reference declaration metadata");
                        self.diagnostics.push(
                            Diagnostic::error(
                                INVALID_MEMBER_SELECTION,
                                format!(
                                    "method `{}` cannot be used as an object place",
                                    declaration.name
                                ),
                            )
                            .with_primary_label(member.member.span, "expected a class field here")
                            .with_secondary_label(declaration.name_span, "method declared here"),
                        );
                        None
                    }
                    SelectedClassMember::StaticField(field) => {
                        self.report_object_selected_static_field(
                            field,
                            &member.member,
                            INVALID_MEMBER_SELECTION,
                            "object-selected static field",
                        );
                        None
                    }
                }
            }
            syntax::Expression::GenericStaticSelection(_) => {
                let resolved = self.resolve_expression(expression)?;
                let ResolvedExpression::StaticFieldAccess(access) = resolved else {
                    unreachable!("generic static object receiver must be a static field")
                };
                self.object_receiver_from_static_field_access(access)
            }
            _ => self
                .resolve_object_place(expression)
                .map(ResolvedObjectReceiver::from_place),
        }
    }

    pub(super) fn object_receiver_from_static_field_access(
        &mut self,
        access: ResolvedStaticFieldAccessExpr,
    ) -> Option<ResolvedObjectReceiver> {
        let declaration = self
            .environment
            .classes
            .get(access.field.class())
            .and_then(|class| class.static_field(access.field))
            .expect("resolved static field access must retain declaration metadata");
        let ResolvedTypeKind::Class(class) = declaration.type_syntax.kind else {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_MEMBER_SELECTION,
                    format!(
                        "static field `{}` does not contain an inline class object",
                        declaration.name
                    ),
                )
                .with_primary_label(access.span, "expected an exact-class static field")
                .with_secondary_label(declaration.type_syntax.span, "field type declared here"),
            );
            return None;
        };
        Some(ResolvedObjectReceiver::from_static_field(
            access.field,
            class,
            access.span,
        ))
    }

    pub(super) fn resolve_member_object_receiver(
        &mut self,
        member: &syntax::MemberAccessExpr,
    ) -> Option<ResolvedObjectReceiver> {
        match member.operator {
            syntax::MemberAccessOperator::Dot { .. } => {
                self.resolve_object_receiver(&member.receiver)
            }
            syntax::MemberAccessOperator::Arrow {
                span: operator_span,
            } => {
                let span = self.cover(member.receiver.span(), operator_span);
                let dereference = self.resolve_dereference(
                    &member.receiver,
                    ResolvedDereferenceOperator::Arrow,
                    operator_span,
                    span,
                )?;
                self.object_receiver_from_dereference(dereference)
            }
        }
    }

    pub(super) fn object_receiver_from_dereference(
        &mut self,
        dereference: ResolvedDereferenceExpr,
    ) -> Option<ResolvedObjectReceiver> {
        let ResolvedSharedTarget::Class(class) = dereference.target else {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_MEMBER_SELECTION,
                    "ordinary member selection requires a shared class target",
                )
                .with_primary_label(
                    dereference.operator_span,
                    "this dereference does not select a class place",
                ),
            );
            return None;
        };
        let span = dereference.span;
        Some(ResolvedObjectReceiver::Dereference {
            dereference: Box::new(dereference),
            projections: Vec::new(),
            class,
            span,
        })
    }

    pub(super) fn resolve_object_place(
        &mut self,
        expression: &syntax::Expression,
    ) -> Option<ResolvedObjectPlace> {
        match expression {
            syntax::Expression::Identifier(identifier) => self.resolve_binding_place(identifier),
            syntax::Expression::SelfValue(self_value) => self.resolve_self_place(self_value.span),
            syntax::Expression::Grouped(grouped) => Some(
                self.resolve_object_place(&grouped.expression)?
                    .with_span(grouped.span),
            ),
            syntax::Expression::MemberAccess(member) => {
                let receiver = self.resolve_object_place(&member.receiver)?;
                let selected = self.select_member(receiver.class, &member.member)?;
                let receiver =
                    self.project_to_declaring_class(receiver, selected.declaring_class());
                match selected {
                    SelectedClassMember::Field(field) => {
                        self.project_field(receiver, field, member.span, member.member.span)
                    }
                    SelectedClassMember::Method(method) => {
                        let declaration = self
                            .environment
                            .classes
                            .get(method.class())
                            .and_then(|class| class.method(method))
                            .expect("member symbols must reference declaration metadata");
                        self.diagnostics.push(
                            Diagnostic::error(
                                INVALID_MEMBER_SELECTION,
                                format!(
                                    "method `{}` cannot be used as an object place",
                                    declaration.name
                                ),
                            )
                            .with_primary_label(member.member.span, "expected a class field here")
                            .with_secondary_label(declaration.name_span, "method declared here"),
                        );
                        None
                    }
                    SelectedClassMember::StaticField(field) => {
                        self.report_object_selected_static_field(
                            field,
                            &member.member,
                            INVALID_MEMBER_SELECTION,
                            "object-selected static field",
                        );
                        None
                    }
                }
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

    fn resolve_binding_place(
        &mut self,
        identifier: &syntax::IdentifierExpr,
    ) -> Option<ResolvedObjectPlace> {
        let Some(binding) = self.lookup_binding(&identifier.name.text) else {
            self.report_unknown(&identifier.name.text, identifier.span, "unknown object");
            return None;
        };
        let class = match binding.ty {
            ResolvedTypeKind::Class(class) => class,
            ResolvedTypeKind::Shared(target) => {
                self.report_implicit_shared_member_access(identifier.span, target);
                return None;
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_MEMBER_SELECTION,
                        format!("binding `{}` is not an object", identifier.name.text),
                    )
                    .with_primary_label(identifier.span, "member access requires an object")
                    .with_secondary_label(binding.name_span, "binding declared here"),
                );
                return None;
            }
        };
        Some(ResolvedObjectPlace::root(
            binding.id,
            class,
            identifier.span,
        ))
    }

    fn resolve_self_place(&mut self, span: Span) -> Option<ResolvedObjectPlace> {
        let class = self.receiver_class.or_else(|| {
            self.diagnostics.push(
                Diagnostic::error(SELF_OUTSIDE_MEMBER, "`self` is not available here")
                    .with_primary_label(span, "only an initializer or instance method has `self`"),
            );
            None
        })?;
        Some(ResolvedObjectPlace::root(
            BindingId::Receiver(self.callable),
            class,
            span,
        ))
    }

    fn project_field(
        &mut self,
        receiver: ResolvedObjectPlace,
        field: FieldId,
        span: Span,
        member_span: Span,
    ) -> Option<ResolvedObjectPlace> {
        let declaration = self
            .environment
            .classes
            .get(field.class())
            .and_then(|class| class.field(field))
            .expect("member symbols must reference declaration metadata");
        let ResolvedTypeKind::Class(class) = declaration.type_syntax.kind else {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_MEMBER_SELECTION,
                    format!("field `{}` does not contain an object", declaration.name),
                )
                .with_primary_label(
                    member_span,
                    "member access cannot continue through this field",
                )
                .with_secondary_label(declaration.type_syntax.span, "field has a primitive type"),
            );
            return None;
        };
        Some(receiver.project_field(field, class, span))
    }

    fn project_receiver_field(
        &mut self,
        receiver: ResolvedObjectReceiver,
        field: FieldId,
        span: Span,
        member_span: Span,
    ) -> Option<ResolvedObjectReceiver> {
        let declaration = self
            .environment
            .classes
            .get(field.class())
            .and_then(|class| class.field(field))
            .expect("member symbols must reference declaration metadata");
        if let ResolvedTypeKind::Shared(crate::resolve::ResolvedSharedTarget::Class(class)) =
            declaration.type_syntax.kind
        {
            self.report_implicit_shared_member_access(
                member_span,
                ResolvedSharedTarget::Class(class),
            );
            return None;
        }
        let ResolvedTypeKind::Class(class) = declaration.type_syntax.kind else {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_MEMBER_SELECTION,
                    format!("field `{}` does not contain an object", declaration.name),
                )
                .with_primary_label(
                    member_span,
                    "member access cannot continue through this field",
                )
                .with_secondary_label(declaration.type_syntax.span, "field has a primitive type"),
            );
            return None;
        };
        Some(receiver.project_field(field, class, span))
    }

    pub(super) fn project_receiver_to_declaring_class(
        &self,
        mut receiver: ResolvedObjectReceiver,
        declaring_class: ClassId,
    ) -> ResolvedObjectReceiver {
        if receiver.class() == declaring_class {
            return receiver;
        }
        let span = receiver.span();
        for base in self
            .environment
            .hierarchy
            .base_chain(receiver.class())
            .expect("resolved member receiver must have valid ancestry")
        {
            receiver = receiver.project_base(base, span);
            if base == declaring_class {
                return receiver;
            }
        }
        unreachable!("selected inherited member owner must be in the receiver base chain")
    }

    pub(super) fn project_to_declaring_class(
        &self,
        mut receiver: ResolvedObjectPlace,
        declaring_class: ClassId,
    ) -> ResolvedObjectPlace {
        if receiver.class == declaring_class {
            return receiver;
        }
        let span = receiver.span;
        for base in self
            .environment
            .hierarchy
            .base_chain(receiver.class)
            .expect("resolved member receiver must have valid ancestry")
        {
            receiver = receiver.project_base(base, span);
            if base == declaring_class {
                return receiver;
            }
        }
        unreachable!("selected inherited member owner must be in the receiver base chain")
    }
}
