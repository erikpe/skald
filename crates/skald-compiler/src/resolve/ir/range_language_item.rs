//! Validated canonical range-protocol identities.

use crate::{
    identity::{InterfaceTemplateId, InterfaceTemplateRequirementId, TypeParameterId},
    source::Span,
};

/// Request-local identities from the canonical `std::range` module.
///
/// The product intentionally contains only the successor protocol until the
/// ordinary `Range<T>` class is implemented. Later range milestones can extend
/// this cohesive product without making current consumers rediscover names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedRangeLanguageItem {
    pub successor_template: InterfaceTemplateId,
    pub successor_output_parameter: TypeParameterId,
    pub successor_requirement: InterfaceTemplateRequirementId,
    pub successor_declaration_span: Span,
    pub requiring_spans: Vec<Span>,
}
