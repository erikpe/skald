//! Shared-owner target discovery and explicit dereference resolution.

use super::*;

impl CallableResolver<'_, '_> {
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
        let kind = match expression {
            ResolvedExpression::Binding(binding) => self.binding_type(binding.binding)?,
            ResolvedExpression::FieldAccess(access) => {
                self.environment
                    .classes
                    .get(access.field.class())?
                    .field(access.field)?
                    .type_syntax
                    .kind
            }
            ResolvedExpression::Allocation(allocation) => {
                return Some(ResolvedSharedTarget::Class(allocation.class))
            }
            ResolvedExpression::DirectCall(call) => {
                self.environment
                    .functions
                    .get(call.function)?
                    .return_type
                    .kind
            }
            ResolvedExpression::MethodCall(call) => {
                self.environment
                    .classes
                    .get(call.method.class())?
                    .method(call.method)?
                    .return_type
                    .kind
            }
            ResolvedExpression::InterfaceCall(call) => {
                self.environment
                    .interfaces
                    .get(call.interface)?
                    .requirements
                    .get(call.requirement.index())?
                    .return_type
                    .kind
            }
            ResolvedExpression::ObjectCast(cast)
                if matches!(
                    cast.target_mode,
                    ResolvedObjectCastTargetMode::Shared { .. }
                ) =>
            {
                match cast.target.kind {
                    ResolvedTypeKind::Class(class) => {
                        return Some(ResolvedSharedTarget::Class(class))
                    }
                    ResolvedTypeKind::Interface(interface) => {
                        return Some(ResolvedSharedTarget::Interface(interface))
                    }
                    ResolvedTypeKind::Obj => return Some(ResolvedSharedTarget::Obj),
                    _ => return None,
                }
            }
            ResolvedExpression::Grouped(grouped) => {
                return self.resolved_shared_target(&grouped.expression)
            }
            _ => return None,
        };
        match kind {
            ResolvedTypeKind::Shared(target) => Some(target),
            _ => None,
        }
    }

    fn binding_type(&self, binding: BindingId) -> Option<ResolvedTypeKind> {
        if binding == BindingId::Receiver(self.callable) {
            return self.receiver_class.map(ResolvedTypeKind::Class);
        }
        self.scopes
            .iter()
            .rev()
            .flat_map(HashMap::values)
            .find(|symbol| symbol.id == binding)
            .map(|symbol| symbol.ty)
    }
}
