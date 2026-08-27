//! Conversion of resolved interface expressions into ordinary call receivers.

use crate::{identity::InterfaceId, source::Span};

use super::{
    ResolvedExpression, ResolvedInterfaceReceiver, ResolvedObjectCastTargetMode,
    ResolvedSharedTarget, ResolvedTypeKind,
};

impl ResolvedInterfaceReceiver {
    /// Converts one already-resolved exact-interface expression into the
    /// receiver carrier shared by ordinary interface-call consumers.
    pub(crate) fn from_expression(
        expression: ResolvedExpression,
        interface: InterfaceId,
    ) -> Result<(Self, Span), Box<ResolvedExpression>> {
        let span = expression.span();
        let receiver = match expression {
            ResolvedExpression::Binding(binding) => Self::Binding {
                binding: binding.binding,
                span: binding.span,
            },
            ResolvedExpression::Grouped(grouped) => {
                let (receiver, _) = Self::from_expression(*grouped.expression, interface)?;
                return Ok((receiver, grouped.span));
            }
            ResolvedExpression::ObjectCast(cast)
                if cast.target.kind == ResolvedTypeKind::Interface(interface)
                    && cast.target_mode == ResolvedObjectCastTargetMode::Plain =>
            {
                Self::Cast(Box::new(cast))
            }
            ResolvedExpression::Dereference(dereference)
                if dereference.target == ResolvedSharedTarget::Interface(interface) =>
            {
                Self::Dereference(Box::new(dereference))
            }
            ResolvedExpression::Unwrap(unwrap) => Self::OptionalBoxPayload(Box::new(unwrap)),
            unsupported => return Err(Box::new(unsupported)),
        };
        Ok((receiver, span))
    }
}
