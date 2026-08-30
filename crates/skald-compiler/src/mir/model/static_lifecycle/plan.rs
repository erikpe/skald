//! Executable lifecycle definitions, order, and transition schema.

use std::fmt;

use crate::{
    identity::{StaticFieldId, StaticInitializerId},
    source::Span,
};

use super::super::MirType;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLifecyclePlan {
    activation: Vec<StaticFieldId>,
    shutdown: Vec<StaticFieldId>,
}

impl StaticLifecyclePlan {
    pub(crate) fn new(activation: Vec<StaticFieldId>) -> Self {
        let shutdown = activation.iter().rev().copied().collect();
        Self {
            activation,
            shutdown,
        }
    }

    pub fn activation(&self) -> &[StaticFieldId] {
        &self.activation
    }

    pub fn shutdown(&self) -> &[StaticFieldId] {
        &self.shutdown
    }

    #[cfg(test)]
    pub(crate) fn activation_mut_for_test(&mut self) -> &mut Vec<StaticFieldId> {
        &mut self.activation
    }

    #[cfg(test)]
    pub(crate) fn shutdown_mut_for_test(&mut self) -> &mut Vec<StaticFieldId> {
        &mut self.shutdown
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
pub struct MirStaticLifecycleIndices {
    pub activation: usize,
    pub shutdown: usize,
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
    pub indices: MirStaticLifecycleIndices,
    pub span: Span,
}
