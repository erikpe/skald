//! Shared-owner target discovery and explicit dereference resolution.

use super::*;

impl CallableResolver<'_, '_> {
    pub(super) fn report_implicit_shared_member_access(
        &mut self,
        span: Span,
        target: ResolvedSharedTarget,
    ) {
        self.diagnostics.push(
            Diagnostic::error(
                IMPLICIT_SHARED_DEREFERENCE,
                "shared owner member access requires explicit dereference",
            )
            .with_primary_label(
                span,
                format!(
                    "this expression has type `{}`",
                    self.resolved_shared_target_name(target)
                ),
            )
            .with_note("use `owner->member` to cross one shared edge")
            .with_note("use `(*owner).member` when grouping is clearer"),
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
            ResolvedExpression::Unwrap(unwrap) => {
                return self.resolved_optional_shared_target(&unwrap.source)
            }
            _ => return None,
        };
        match kind {
            ResolvedTypeKind::Shared(target) => Some(target),
            _ => None,
        }
    }

    fn resolved_optional_shared_target(
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
            ResolvedExpression::Grouped(grouped) => {
                return self.resolved_optional_shared_target(&grouped.expression)
            }
            _ => return None,
        };
        match kind {
            ResolvedTypeKind::OptionalShared { target, .. } => Some(target),
            _ => None,
        }
    }

    pub(super) fn resolved_optional_class(
        &self,
        expression: &ResolvedExpression,
    ) -> Option<ClassId> {
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
            ResolvedExpression::Grouped(grouped) => {
                return self.resolved_optional_class(&grouped.expression)
            }
            _ => return None,
        };
        match kind {
            ResolvedTypeKind::Optional {
                payload: ResolvedOptionalPayload::Class(class),
                ..
            } => Some(class),
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
