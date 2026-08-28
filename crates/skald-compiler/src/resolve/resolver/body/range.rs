//! Canonical construction evidence for concise range expressions.

use super::*;
use crate::identity::{InterfaceId, InterfaceRequirementId, InterfaceTemplateRequirementId};

impl CallableResolver<'_, '_> {
    pub(super) fn resolve_range_expression(
        &mut self,
        range: &syntax::RangeExpr,
    ) -> Option<ResolvedExpression> {
        // Keep source order explicit even though resolution performs no evaluation.
        let lower = self.resolve_expression(&range.lower);
        let upper = self.resolve_expression(&range.upper);
        let (Some(lower), Some(upper)) = (lower, upper) else {
            return None;
        };
        let lower_type = self.resolved_expression_type(&lower)?;
        let upper_type = self.resolved_expression_type(&upper)?;
        if lower_type != upper_type {
            self.diagnostics.push(
                Diagnostic::error(
                    RANGE_ENDPOINT_TYPE_MISMATCH,
                    "range endpoints must have exactly the same static type",
                )
                .with_primary_label(range.operator_span, "exact endpoint types differ here")
                .with_secondary_label(range.lower.span(), "lower endpoint")
                .with_secondary_label(range.upper.span(), "upper endpoint"),
            );
            return None;
        }

        let environment = self.environment.language_items.range?;
        let range_class = self
            .environment
            .specialization
            .and_then(|specialization| specialization.range_class(range.operator_span))
            .or_else(|| {
                self.environment
                    .lookup
                    .specialized_class(range.operator_span)
            });
        let Some(range_class) = range_class else {
            if self
                .environment
                .lookup
                .specialization_at(range.operator_span)
                .is_some()
            {
                // Specialization validation owns the precise missing-bound or
                // capability diagnostic for an attempted canonical Range<T>.
                return None;
            }
            self.diagnostics.push(
                Diagnostic::error(
                    UNSUPPORTED_RANGE_APPLICATION,
                    "the endpoint type does not satisfy the canonical range bounds",
                )
                .with_primary_label(range.operator_span, "range construction is unavailable")
                .with_secondary_label(range.lower.span(), "endpoint type selected here"),
            );
            return None;
        };
        let specialization = self
            .environment
            .lookup
            .class_specialization(range_class)
            .expect("range syntax selects a closed generic class");
        if specialization.key.template != environment.language_item.range_template
            || specialization.key.arguments.as_slice() != [lower_type]
        {
            return None;
        }
        let class = self
            .environment
            .classes
            .get(range_class)
            .expect("complete range specialization has a declaration");
        let initializer = class
            .initializers
            .first()
            .expect("validated range member slot closes in every specialization")
            .id;
        let ordering_interface = specialization.closed_interface_bounds
            [environment.language_item.range_ordering_bound]
            .expect("complete range specialization closes ordering");
        let successor_interface = specialization.closed_interface_bounds
            [environment.language_item.range_successor_bound]
            .expect("complete range specialization closes successor");
        let iterable = specialization.closed_interface_claims
            [environment.language_item.range_iterable_claim]
            .expect("complete range specialization closes Iterable");

        let ordering_requirement = closed_requirement(
            environment.applications,
            ordering_interface,
            environment.language_item.range_ordering_requirement,
        )?;
        let successor_requirement = closed_requirement(
            environment.applications,
            successor_interface,
            environment.language_item.successor_requirement,
        )?;
        let realization = primitive_type(lower_type)
            .map(ResolvedRangeProtocolRealization::PrimitiveIntrinsic)
            .unwrap_or(ResolvedRangeProtocolRealization::ClassWitness);

        Some(ResolvedExpression::Range(Box::new(ResolvedRangeExpr {
            lower: Box::new(lower),
            operator_span: range.operator_span,
            upper: Box::new(upper),
            endpoint_type: lower_type,
            range_template: environment.language_item.range_template,
            range_class,
            initializer,
            ordering: ResolvedRangeProtocolEvidence {
                interface: ordering_interface,
                requirement: ordering_requirement,
                realization,
            },
            successor: ResolvedRangeProtocolEvidence {
                interface: successor_interface,
                requirement: successor_requirement,
                realization,
            },
            iterable,
            span: range.span,
        })))
    }
}

fn closed_requirement(
    applications: &GenericInterfaceSpecializationTable,
    interface: InterfaceId,
    template: InterfaceTemplateRequirementId,
) -> Option<InterfaceRequirementId> {
    applications
        .for_interface(interface)?
        .requirement_mappings
        .iter()
        .find_map(|mapping| (mapping.template == template).then_some(mapping.closed))
}

const fn primitive_type(kind: ResolvedTypeKind) -> Option<ResolvedPrimitiveType> {
    match kind {
        ResolvedTypeKind::I64 => Some(ResolvedPrimitiveType::I64),
        ResolvedTypeKind::U64 => Some(ResolvedPrimitiveType::U64),
        ResolvedTypeKind::U8 => Some(ResolvedPrimitiveType::U8),
        _ => None,
    }
}
