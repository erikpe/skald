//! Validation and ordinary construction lowering for structural range-loop sources.

use super::*;
use crate::{
    hir::{HirPrimitiveRangeEvidence, HirRangeProtocolEvidence, HirRangeProtocolRealization},
    identity::{InterfaceId, InterfaceRequirementId, InterfaceTemplateRequirementId},
    resolve::{
        ResolvedConstructExpr, ResolvedConstructionMode, ResolvedPrimitiveType,
        ResolvedRangeForSource, ResolvedRangeProtocolEvidence, ResolvedRangeProtocolRealization,
        ResolvedTypeKind,
    },
    typeck::program::{lower_type_kind, INVALID_RESOLVED_RANGE_SOURCE},
};

impl CallableChecker<'_, '_> {
    pub(super) fn validate_primitive_range_source(
        &mut self,
        source: &ResolvedRangeForSource,
    ) -> Option<HirPrimitiveRangeEvidence> {
        self.validate_range_source(source)?;
        let endpoint = lower_type_kind(source.endpoint_type);
        if !matches!(endpoint, Type::I64 | Type::U64 | Type::U8) {
            return None;
        }
        Some(HirPrimitiveRangeEvidence {
            operator_span: source.operator_span,
            range_template: source.range_template,
            range_class: source.range_class,
            initializer: source.initializer,
            ordering: lower_primitive_protocol_evidence(source.ordering, endpoint)?,
            successor: lower_primitive_protocol_evidence(source.successor, endpoint)?,
            iterable: source.iterable,
        })
    }

    pub(super) fn validate_range_source(&mut self, source: &ResolvedRangeForSource) -> Option<()> {
        if self.range_source_is_valid(source) {
            return Some(());
        }
        self.report_invalid_range_source(
            source.operator_span,
            "resolved range-loop source does not match canonical range evidence",
        );
        None
    }

    pub(super) fn validate_range_selection(
        &mut self,
        source: &ResolvedRangeForSource,
        selection: &crate::resolve::ResolvedIterableSelection,
    ) -> Option<()> {
        if source.iterable == selection.interface
            && source.endpoint_type == selection.item
            && source.endpoint_type == selection.state
        {
            return Some(());
        }
        self.report_invalid_range_source(
            source.operator_span,
            "range-loop protocol selection does not match its canonical source",
        );
        None
    }

    fn range_source_is_valid(&self, source: &ResolvedRangeForSource) -> bool {
        let Some(language_item) = self.program.range_language_item.as_ref() else {
            return false;
        };
        if source.range_template != language_item.range_template
            || Some(source.endpoint_provenance) != self.expected_endpoint_provenance(source)
            || self.static_expression_type(&source.lower) != lower_type_kind(source.endpoint_type)
            || self.static_expression_type(&source.upper) != lower_type_kind(source.endpoint_type)
        {
            return false;
        }
        let Some(class) = self.program.class(source.range_class) else {
            return false;
        };
        if class.initializers.first().map(|declaration| declaration.id) != Some(source.initializer)
        {
            return false;
        }
        let Some(specialization) = self
            .program
            .generic_specializations
            .for_class(source.range_class)
        else {
            return false;
        };
        if specialization.key.template != source.range_template
            || specialization.key.arguments.as_slice() != [source.endpoint_type]
        {
            return false;
        }

        let ordering = specialization
            .closed_interface_bounds
            .get(language_item.range_ordering_bound)
            .copied()
            .flatten();
        let successor = specialization
            .closed_interface_bounds
            .get(language_item.range_successor_bound)
            .copied()
            .flatten();
        let iterable = specialization
            .closed_interface_claims
            .get(language_item.range_iterable_claim)
            .copied()
            .flatten();
        ordering == Some(source.ordering.interface)
            && successor == Some(source.successor.interface)
            && iterable == Some(source.iterable)
            && closed_requirement(
                self.program,
                source.ordering.interface,
                language_item.range_ordering_requirement,
                source.ordering.requirement,
            )
            && closed_requirement(
                self.program,
                source.successor.interface,
                language_item.successor_requirement,
                source.successor.requirement,
            )
            && valid_realization(source.endpoint_type, source.ordering)
            && valid_realization(source.endpoint_type, source.successor)
    }

    fn expected_endpoint_provenance(
        &self,
        source: &ResolvedRangeForSource,
    ) -> Option<[crate::resolve::ResolvedRangeEndpointProvenance; 2]> {
        let independent =
            [crate::resolve::ResolvedRangeEndpointProvenance::SpecializationIndependent; 2];
        let Some(owner) = self.class_owner else {
            return Some(independent);
        };
        let Some(specialization) = self.program.generic_specializations.for_class(owner) else {
            return Some(independent);
        };
        let semantics = self
            .program
            .template_semantics
            .get(specialization.key.template)
            .expect("specialized class owner references template semantics");
        semantics
            .selections
            .iter()
            .find_map(|selection| match selection {
                crate::resolve::ResolvedTemplateSelection::Range {
                    endpoint_provenance,
                    span,
                    ..
                } if *span == source.operator_span => Some(*endpoint_provenance),
                _ => None,
            })
    }

    fn report_invalid_range_source(&mut self, span: crate::source::Span, message: &'static str) {
        self.diagnostics.push(
            Diagnostic::error(
                INVALID_RESOLVED_RANGE_SOURCE,
                "invalid resolved range-loop source",
            )
            .with_primary_label(span, message),
        );
    }
}

pub(super) fn resolved_range_construction(
    source: &ResolvedRangeForSource,
) -> ResolvedConstructExpr {
    ResolvedConstructExpr {
        class: source.range_class,
        callee_span: source.operator_span,
        mode: ResolvedConstructionMode::Initialize {
            arguments: vec![source.lower.clone(), source.upper.clone()],
        },
        span: source.span,
    }
}

fn closed_requirement(
    program: &crate::resolve::ResolvedProgram,
    interface: InterfaceId,
    template: InterfaceTemplateRequirementId,
    requirement: InterfaceRequirementId,
) -> bool {
    program
        .generic_interface_specializations
        .for_interface(interface)
        .is_some_and(|application| {
            application
                .requirement_mappings
                .iter()
                .any(|mapping| mapping.template == template && mapping.closed == requirement)
        })
}

fn valid_realization(endpoint: ResolvedTypeKind, evidence: ResolvedRangeProtocolEvidence) -> bool {
    matches!(
        (endpoint, evidence.realization),
        (
            ResolvedTypeKind::I64,
            ResolvedRangeProtocolRealization::PrimitiveIntrinsic(ResolvedPrimitiveType::I64)
        ) | (
            ResolvedTypeKind::U64,
            ResolvedRangeProtocolRealization::PrimitiveIntrinsic(ResolvedPrimitiveType::U64)
        ) | (
            ResolvedTypeKind::U8,
            ResolvedRangeProtocolRealization::PrimitiveIntrinsic(ResolvedPrimitiveType::U8)
        ) | (
            ResolvedTypeKind::Class(_),
            ResolvedRangeProtocolRealization::ClassWitness
        )
    )
}

fn lower_primitive_protocol_evidence(
    evidence: ResolvedRangeProtocolEvidence,
    endpoint: Type,
) -> Option<HirRangeProtocolEvidence> {
    let ResolvedRangeProtocolRealization::PrimitiveIntrinsic(primitive) = evidence.realization
    else {
        return None;
    };
    let ty = match primitive {
        ResolvedPrimitiveType::I64 => Type::I64,
        ResolvedPrimitiveType::U64 => Type::U64,
        ResolvedPrimitiveType::U8 => Type::U8,
        ResolvedPrimitiveType::F64 | ResolvedPrimitiveType::Bool => return None,
    };
    (ty == endpoint).then_some(HirRangeProtocolEvidence {
        interface: evidence.interface,
        requirement: evidence.requirement,
        realization: HirRangeProtocolRealization::PrimitiveIntrinsic(ty),
    })
}
