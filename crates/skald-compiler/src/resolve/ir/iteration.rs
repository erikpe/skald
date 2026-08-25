//! Validated canonical iteration protocol identities.

use crate::{
    identity::{InterfaceTemplateId, InterfaceTemplateRequirementId, TypeParameterId},
    source::Span,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedIterableLanguageItem {
    pub template: InterfaceTemplateId,
    pub item_parameter: TypeParameterId,
    pub state_parameter: TypeParameterId,
    pub iter_state_requirement: InterfaceTemplateRequirementId,
    pub iter_next_requirement: InterfaceTemplateRequirementId,
    pub declaration_span: Span,
    pub requiring_spans: Vec<Span>,
}
