//! Canonical lifecycle definitions, activation order, and transition schema.

use std::fmt;

use crate::{
    identity::{StaticFieldId, StaticInitializerId},
    source::Span,
};

use super::super::MirType;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLifecyclePlan {
    activation: Vec<StaticFieldId>,
}

impl StaticLifecyclePlan {
    pub(crate) fn new(activation: Vec<StaticFieldId>) -> Self {
        Self { activation }
    }

    pub fn activation(&self) -> &[StaticFieldId] {
        &self.activation
    }

    /// Iterates the exact reverse of activation without storing a second order.
    pub fn shutdown(
        &self,
    ) -> impl ExactSizeIterator<Item = StaticFieldId> + DoubleEndedIterator + Clone + '_ {
        self.activation.iter().rev().copied()
    }

    #[cfg(test)]
    pub(crate) fn activation_mut_for_test(&mut self) -> &mut Vec<StaticFieldId> {
        &mut self.activation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirStaticFieldInitialization {
    ZeroDefault,
    Explicit(StaticInitializerId),
}

impl fmt::Display for MirStaticFieldInitialization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDefault => formatter.write_str("zero-default"),
            Self::Explicit(initializer) => write!(formatter, "explicit {initializer}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirStaticLifecycleTransitionKind {
    /// Activates an initializer-free all-zero slot directly as live.
    ActivateZeroDefault,
    BeginInitialization,
    PublishLive,
    BeginDestruction,
    FinishDestruction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirStaticLifecycleTransition {
    pub field: StaticFieldId,
    pub kind: MirStaticLifecycleTransitionKind,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirStaticLifecycleDefinition {
    pub field: StaticFieldId,
    pub ty: MirType,
    pub initialization: MirStaticFieldInitialization,
    pub final_span: Option<Span>,
    pub span: Span,
}

/// Canonical lifecycle definition table ordered by stable static-field identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MirStaticLifecycleDefinitions {
    entries: Vec<MirStaticLifecycleDefinition>,
}

impl MirStaticLifecycleDefinitions {
    pub(super) fn new(mut entries: Vec<MirStaticLifecycleDefinition>) -> Self {
        entries.sort_by_key(|definition| definition.field);
        Self { entries }
    }

    pub(super) fn entries(&self) -> &[MirStaticLifecycleDefinition] {
        &self.entries
    }

    pub(super) fn get(&self, field: StaticFieldId) -> Option<&MirStaticLifecycleDefinition> {
        self.entries
            .binary_search_by_key(&field, |definition| definition.field)
            .ok()
            .map(|index| &self.entries[index])
    }

    #[cfg(test)]
    pub(super) fn entries_mut_for_test(&mut self) -> &mut Vec<MirStaticLifecycleDefinition> {
        &mut self.entries
    }
}
