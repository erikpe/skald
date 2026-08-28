//! Validation and HIR lowering for canonical concise-range provenance.

use super::*;
use crate::{
    hir::{
        HirCanonicalRangeOrigin, HirConstructionOrigin, HirRangeProtocolEvidence,
        HirRangeProtocolRealization,
    },
    identity::{
        InitializerId, InterfaceId, InterfaceRequirementId, InterfaceTemplateRequirementId,
    },
    resolve::{
        ResolvedCanonicalRangeOrigin, ResolvedConstructExpr, ResolvedConstructionMode,
        ResolvedConstructionOrigin, ResolvedPrimitiveType, ResolvedRangeProtocolEvidence,
        ResolvedRangeProtocolRealization, ResolvedTypeKind,
    },
    typeck::program::{lower_type_kind, INVALID_RANGE_CONSTRUCTION_ORIGIN},
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_construction_origin(
        &mut self,
        construction: &ResolvedConstructExpr,
        initializer: InitializerId,
    ) -> Option<HirConstructionOrigin> {
        match &construction.origin {
            ResolvedConstructionOrigin::Explicit => self.check_explicit_origin(construction),
            ResolvedConstructionOrigin::CanonicalRangeSyntax(origin) => self
                .validate_range_origin(construction, origin, initializer)
                .map(HirConstructionOrigin::CanonicalRangeSyntax),
        }
    }

    pub(super) fn check_construction_origin_without_initializer(
        &mut self,
        construction: &ResolvedConstructExpr,
    ) -> Option<HirConstructionOrigin> {
        match construction.origin {
            ResolvedConstructionOrigin::Explicit => self.check_explicit_origin(construction),
            ResolvedConstructionOrigin::CanonicalRangeSyntax(ref origin) => {
                self.report_invalid_range_origin(
                    origin.operator_span,
                    "range syntax must use canonical initializer construction",
                );
                None
            }
        }
    }

    fn check_explicit_origin(
        &mut self,
        construction: &ResolvedConstructExpr,
    ) -> Option<HirConstructionOrigin> {
        if self
            .program
            .range_expression_spans
            .contains(&construction.callee_span)
        {
            self.report_invalid_range_origin(
                construction.callee_span,
                "range syntax lost its canonical construction provenance",
            );
            None
        } else {
            Some(HirConstructionOrigin::Explicit)
        }
    }

    fn validate_range_origin(
        &mut self,
        construction: &ResolvedConstructExpr,
        origin: &ResolvedCanonicalRangeOrigin,
        initializer: InitializerId,
    ) -> Option<HirCanonicalRangeOrigin> {
        if !self.range_origin_is_valid(construction, origin, initializer) {
            self.report_invalid_range_origin(
                origin.operator_span,
                "resolved range construction does not match canonical range evidence",
            );
            return None;
        }

        Some(HirCanonicalRangeOrigin {
            operator_span: origin.operator_span,
            range_template: origin.range_template,
            range_class: origin.range_class,
            initializer: origin.initializer,
            endpoint_type: lower_type_kind(origin.endpoint_type),
            ordering: lower_protocol_evidence(origin.ordering),
            successor: lower_protocol_evidence(origin.successor),
            iterable: origin.iterable,
        })
    }

    fn range_origin_is_valid(
        &self,
        construction: &ResolvedConstructExpr,
        origin: &ResolvedCanonicalRangeOrigin,
        initializer: InitializerId,
    ) -> bool {
        let Some(language_item) = self.program.range_language_item.as_ref() else {
            return false;
        };
        let ResolvedConstructionMode::Initialize { arguments } = &construction.mode else {
            return false;
        };
        if construction.class != origin.range_class
            || construction.callee_span != origin.operator_span
            || !self
                .program
                .range_expression_spans
                .contains(&origin.operator_span)
            || initializer != origin.initializer
            || origin.range_template != language_item.range_template
            || arguments.len() != 2
            || arguments.iter().any(|argument| {
                self.static_expression_type(argument) != lower_type_kind(origin.endpoint_type)
            })
        {
            return false;
        }

        let Some(class) = self.program.class(origin.range_class) else {
            return false;
        };
        if class.initializers.first().map(|declaration| declaration.id) != Some(origin.initializer)
        {
            return false;
        }
        let Some(specialization) = self
            .program
            .generic_specializations
            .for_class(origin.range_class)
        else {
            return false;
        };
        if specialization.key.template != origin.range_template
            || specialization.key.arguments.as_slice() != [origin.endpoint_type]
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
        ordering == Some(origin.ordering.interface)
            && successor == Some(origin.successor.interface)
            && iterable == Some(origin.iterable)
            && closed_requirement(
                self.program,
                origin.ordering.interface,
                language_item.range_ordering_requirement,
                origin.ordering.requirement,
            )
            && closed_requirement(
                self.program,
                origin.successor.interface,
                language_item.successor_requirement,
                origin.successor.requirement,
            )
            && valid_realization(origin.endpoint_type, origin.ordering)
            && valid_realization(origin.endpoint_type, origin.successor)
    }

    fn report_invalid_range_origin(&mut self, span: crate::source::Span, message: &'static str) {
        self.diagnostics.push(
            Diagnostic::error(
                INVALID_RANGE_CONSTRUCTION_ORIGIN,
                "invalid canonical range construction provenance",
            )
            .with_primary_label(span, message),
        );
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
            ResolvedRangeProtocolRealization::PrimitiveIntrinsic(ResolvedPrimitiveType::I64),
        ) | (
            ResolvedTypeKind::U64,
            ResolvedRangeProtocolRealization::PrimitiveIntrinsic(ResolvedPrimitiveType::U64),
        ) | (
            ResolvedTypeKind::U8,
            ResolvedRangeProtocolRealization::PrimitiveIntrinsic(ResolvedPrimitiveType::U8),
        ) | (
            ResolvedTypeKind::Class(_),
            ResolvedRangeProtocolRealization::ClassWitness
        )
    )
}

fn lower_protocol_evidence(evidence: ResolvedRangeProtocolEvidence) -> HirRangeProtocolEvidence {
    let realization = match evidence.realization {
        ResolvedRangeProtocolRealization::ClassWitness => HirRangeProtocolRealization::ClassWitness,
        ResolvedRangeProtocolRealization::PrimitiveIntrinsic(primitive) => {
            let ty = match primitive {
                ResolvedPrimitiveType::I64 => Type::I64,
                ResolvedPrimitiveType::U64 => Type::U64,
                ResolvedPrimitiveType::U8 => Type::U8,
                ResolvedPrimitiveType::F64 => Type::F64,
                ResolvedPrimitiveType::Bool => Type::Bool,
            };
            HirRangeProtocolRealization::PrimitiveIntrinsic(ty)
        }
    };
    HirRangeProtocolEvidence {
        interface: evidence.interface,
        requirement: evidence.requirement,
        realization,
    }
}
