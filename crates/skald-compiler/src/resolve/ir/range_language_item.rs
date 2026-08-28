//! Validated canonical range-protocol identities.

use crate::{
    identity::{
        ClassTemplateId, InterfaceTemplateId, InterfaceTemplateRequirementId, TypeParameterId,
    },
    source::Span,
};

/// Request-local identities from the canonical `std::range` module.
///
/// Stable numeric fields identify declaration-order slots inside the canonical
/// class template. Closed specializations can derive their concrete
/// initializer and interface identities from these slots without rediscovering
/// source names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedRangeLanguageItem {
    pub successor_template: InterfaceTemplateId,
    pub successor_output_parameter: TypeParameterId,
    pub successor_requirement: InterfaceTemplateRequirementId,
    pub successor_declaration_span: Span,
    pub range_template: ClassTemplateId,
    pub range_parameter: TypeParameterId,
    pub range_initializer_member: usize,
    pub range_ordering_bound: usize,
    pub range_successor_bound: usize,
    pub range_iterable_claim: usize,
    pub range_declaration_span: Span,
    pub requiring_spans: Vec<Span>,
}
