//! Shared-owner target discovery and explicit dereference resolution.

use super::*;

impl CallableResolver<'_, '_> {
    /// Returns the static object leaf when `unwrap` removes the final optional
    /// layer of a boxed object view. Intermediate layers deliberately have no
    /// fabricated inline optional identity for interface and `Obj` leaves.
    pub(super) fn resolved_optional_box_object_leaf(
        &self,
        unwrap: &ResolvedUnwrapExpr,
    ) -> Option<ResolvedObjectTarget> {
        let (target, remaining) = self.optional_box_view_state(&unwrap.source)?;
        if remaining != 1 {
            return None;
        }
        self.type_interner.optional_box(target)?.object_leaf
    }

    fn optional_box_view_state(
        &self,
        expression: &ResolvedExpression,
    ) -> Option<(crate::identity::OptionalBoxTypeId, usize)> {
        match expression {
            ResolvedExpression::Dereference(dereference) => {
                let ResolvedSharedTarget::OptionalBox(target) = dereference.target else {
                    return None;
                };
                Some((
                    target,
                    self.type_interner.optional_box(target)?.optional_depth,
                ))
            }
            ResolvedExpression::Unwrap(unwrap) => {
                let (target, remaining) = self.optional_box_view_state(&unwrap.source)?;
                remaining
                    .checked_sub(1)
                    .map(|remaining| (target, remaining))
            }
            ResolvedExpression::Grouped(grouped) => {
                self.optional_box_view_state(&grouped.expression)
            }
            _ => None,
        }
    }

    pub(super) fn report_implicit_shared_member_access(
        &mut self,
        span: Span,
        target: ResolvedSharedTarget,
    ) {
        self.report_implicit_shared_access(
            span,
            target,
            "shared owner member access requires explicit dereference",
            "use `owner->member` to cross one shared edge",
            "use `(*owner).member` when grouping is clearer",
        );
    }

    pub(super) fn report_implicit_shared_bracket_access(
        &mut self,
        span: Span,
        target: ResolvedSharedTarget,
    ) {
        self.report_implicit_shared_access(
            span,
            target,
            "shared owner bracket access requires explicit dereference",
            "use `owner->[...]` to cross one shared edge",
            "use `(*owner)[...]` when grouping is clearer",
        );
    }

    fn report_implicit_shared_access(
        &mut self,
        span: Span,
        target: ResolvedSharedTarget,
        message: &'static str,
        arrow_note: &'static str,
        dereference_note: &'static str,
    ) {
        self.diagnostics.push(
            Diagnostic::error(IMPLICIT_SHARED_DEREFERENCE, message)
                .with_primary_label(
                    span,
                    format!(
                        "this expression has type `{}`",
                        self.resolved_shared_target_name(target)
                    ),
                )
                .with_note(arrow_note)
                .with_note(dereference_note),
        );
    }

    fn resolved_shared_target_name(&self, target: ResolvedSharedTarget) -> String {
        match target {
            ResolvedSharedTarget::Class(class) => format!(
                "shared {}",
                self.environment
                    .classes
                    .get(class)
                    .expect("resolved shared class target must exist")
                    .name
            ),
            ResolvedSharedTarget::Interface(interface) => format!(
                "shared {}",
                self.environment
                    .interfaces
                    .get(interface)
                    .expect("resolved shared interface target must exist")
                    .name
            ),
            ResolvedSharedTarget::Obj => "shared Obj".to_owned(),
            ResolvedSharedTarget::Array(array) => format!("shared array {array}"),
            ResolvedSharedTarget::OptionalBox(target) => {
                format!("shared optional box {target}")
            }
        }
    }

    pub(super) fn resolve_dereference(
        &mut self,
        source: &syntax::Expression,
        operator: ResolvedDereferenceOperator,
        operator_span: Span,
        span: Span,
    ) -> Option<ResolvedDereferenceExpr> {
        let source = self.resolve_expression(source)?;
        let Some(target) = self.resolved_shared_target(&source) else {
            self.diagnostics.push(
                Diagnostic::error(INVALID_DEREFERENCE, "dereference requires a shared owner")
                    .with_primary_label(operator_span, "this operator requires `shared T`")
                    .with_secondary_label(source.span(), "this expression is not a shared owner"),
            );
            return None;
        };
        Some(ResolvedDereferenceExpr {
            source: Box::new(source),
            target,
            operator,
            operator_span,
            span,
        })
    }

    pub(super) fn resolved_shared_target(
        &self,
        expression: &ResolvedExpression,
    ) -> Option<ResolvedSharedTarget> {
        match self.resolved_expression_type(expression)? {
            ResolvedTypeKind::Shared(target) => Some(target),
            _ => None,
        }
    }

    pub(super) fn resolved_optional_class(
        &self,
        expression: &ResolvedExpression,
    ) -> Option<ClassId> {
        if let ResolvedExpression::Unwrap(unwrap) = expression {
            if let Some(ResolvedObjectTarget::Class(class)) =
                self.resolved_optional_box_object_leaf(unwrap)
            {
                return Some(class);
            }
        }
        let kind = self.resolved_expression_type(expression)?;
        match kind {
            ResolvedTypeKind::Optional(optional) => {
                match self.type_interner.optional(optional)?.payload.kind {
                    ResolvedTypeKind::Class(class) => Some(class),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}
