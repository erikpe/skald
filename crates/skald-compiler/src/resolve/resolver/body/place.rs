//! Recursive object-place resolution and projected-member diagnostics.

use super::*;

impl CallableResolver<'_, '_> {
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
                match self.select_member(receiver.class, &member.member)? {
                    OrdinaryMemberSymbolKind::Field(field) => {
                        self.project_field(receiver, field, member.span, member.member.span)
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
        Some(receiver.project(field, class, span))
    }
}
