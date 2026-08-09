//! Planned static-lifetime phase product and dependency evidence.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    identity::StaticFieldId,
    mir::{
        MirProgram, PreliminaryMirProgram, PreliminaryMirStaticField,
        PreliminaryMirStaticInitializer,
    },
    source::Span,
};

use super::super::{
    StaticAccessKind, StaticEffectAnalysis, StaticEffectEdge, StaticEffectNode, StaticEffectPhase,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StaticLifetimePhase {
    Initialization,
    Destruction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLifetimeEvidence {
    pub root: StaticFieldId,
    pub root_span: Span,
    pub phase: StaticLifetimePhase,
    pub root_effect: StaticEffectNode,
    pub target: StaticFieldId,
    pub target_span: Span,
    pub access: StaticAccessKind,
    pub effect_phase: StaticEffectPhase,
    pub access_span: Span,
    pub witness: Vec<StaticEffectEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLifetimeDependency {
    /// The field that must be live before `dependent` is activated.
    pub prerequisite: StaticFieldId,
    /// The field whose initialization or destruction reaches `prerequisite`.
    pub dependent: StaticFieldId,
    pub evidence: StaticLifetimeEvidence,
}

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
}

/// Preliminary MIR plus the completed, source-diagnosed lifetime analysis.
///
/// The wrapped `PreliminaryMirProgram` remains private, so a backend cannot
/// accidentally consume initializer bodies before lifecycle MIR synthesis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedMirProgram {
    preliminary: PreliminaryMirProgram,
    effects: StaticEffectAnalysis,
    dependencies: Vec<StaticLifetimeDependency>,
    lifecycle: StaticLifecyclePlan,
}

impl PlannedMirProgram {
    pub(crate) fn new(
        preliminary: PreliminaryMirProgram,
        effects: StaticEffectAnalysis,
        dependencies: Vec<StaticLifetimeDependency>,
        lifecycle: StaticLifecyclePlan,
    ) -> Self {
        Self {
            preliminary,
            effects,
            dependencies,
            lifecycle,
        }
    }

    pub fn effects(&self) -> &StaticEffectAnalysis {
        &self.effects
    }

    pub fn dependencies(&self) -> &[StaticLifetimeDependency] {
        &self.dependencies
    }

    pub fn lifecycle(&self) -> &StaticLifecyclePlan {
        &self.lifecycle
    }

    pub fn static_fields(&self) -> impl ExactSizeIterator<Item = &PreliminaryMirStaticField> {
        self.preliminary.static_fields()
    }

    pub fn static_initializers(
        &self,
    ) -> impl ExactSizeIterator<Item = &PreliminaryMirStaticInitializer> {
        self.preliminary.static_initializers()
    }

    pub fn has_static_initializers(&self) -> bool {
        self.preliminary.has_static_initializers()
    }

    /// Converts only a product needing no lifecycle synthesis into the legacy
    /// final-MIR path. Explicit initializers remain unavailable to backends.
    pub fn try_into_final(self) -> Result<MirProgram, Box<Self>> {
        if self.has_static_initializers() {
            return Err(Box::new(self));
        }
        Ok(self
            .preliminary
            .try_into_final()
            .expect("initializer-free preliminary MIR must convert to final MIR"))
    }

    pub(crate) const fn preliminary(&self) -> &PreliminaryMirProgram {
        &self.preliminary
    }
}

#[derive(Debug)]
pub struct StaticLifecyclePlanningFailure {
    dependencies: Vec<StaticLifetimeDependency>,
    diagnostics: Diagnostics,
}

impl StaticLifecyclePlanningFailure {
    pub(crate) fn new(
        dependencies: Vec<StaticLifetimeDependency>,
        diagnostics: Diagnostics,
    ) -> Self {
        Self {
            dependencies,
            diagnostics,
        }
    }

    pub fn dependencies(&self) -> &[StaticLifetimeDependency] {
        &self.dependencies
    }

    pub fn diagnostics(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter()
    }

    pub fn into_diagnostics(self) -> Diagnostics {
        self.diagnostics
    }
}
