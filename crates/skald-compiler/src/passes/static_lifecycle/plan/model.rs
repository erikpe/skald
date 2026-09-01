//! Planned phase product, inspection report, and planning failure model.

use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::{
    identity::{StaticFieldId, StaticInitializerId},
    mir::{
        MirExecutionNode, MirPlannedLifecycle, PreliminaryMirProgram, PreliminaryMirStaticField,
        PreliminaryMirStaticInitializer, StaticAccessKind, StaticActivationAuthority,
        StaticEffectPhase, StaticLifecycleAuthority,
    },
    source::Span,
};

pub use crate::mir::StaticLifecyclePlan;

use super::super::{
    activation::StaticActivationAnalysis,
    analysis::{StaticEffectAnalysis, StaticEffectEdge},
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
    pub root_effect: MirExecutionNode,
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

/// Source-rich effect and activation evidence retained for deterministic
/// inspection of lifecycle planning. The exact field set is also carried by
/// the compact backend-consumable certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLifecyclePlanningReport {
    analysis: StaticEffectAnalysis,
    activation: StaticActivationAnalysis,
}

impl StaticLifecyclePlanningReport {
    pub(crate) const fn new(
        analysis: StaticEffectAnalysis,
        activation: StaticActivationAnalysis,
    ) -> Self {
        Self {
            analysis,
            activation,
        }
    }

    pub const fn analysis(&self) -> &StaticEffectAnalysis {
        &self.analysis
    }

    pub(crate) const fn activation(&self) -> &StaticActivationAnalysis {
        &self.activation
    }

    #[cfg(test)]
    pub(crate) fn activation_mut_for_test(&mut self) -> &mut StaticActivationAnalysis {
        &mut self.activation
    }
}

/// Preliminary MIR plus canonical lifecycle definitions, activation order,
/// compact proof, and planning-only analysis evidence.
///
/// The wrapped preliminary program remains private, so no backend can consume
/// initializer bodies before lifecycle coordinator synthesis. Consuming this
/// product for synthesis drops the planning report at the phase boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedMirProgram {
    preliminary: PreliminaryMirProgram,
    lifecycle: MirPlannedLifecycle,
    report: StaticLifecyclePlanningReport,
}

impl PlannedMirProgram {
    pub(crate) const fn new(
        preliminary: PreliminaryMirProgram,
        lifecycle: MirPlannedLifecycle,
        report: StaticLifecyclePlanningReport,
    ) -> Self {
        Self {
            preliminary,
            lifecycle,
            report,
        }
    }

    pub const fn lifecycle_mir(&self) -> &MirPlannedLifecycle {
        &self.lifecycle
    }

    pub const fn planning_report(&self) -> &StaticLifecyclePlanningReport {
        &self.report
    }

    pub fn authority(&self) -> &StaticLifecycleAuthority {
        self.lifecycle.proof().authority()
    }

    pub fn activation_authority(&self) -> &StaticActivationAuthority {
        self.lifecycle.proof().activation()
    }

    pub fn lifecycle(&self) -> &StaticLifecyclePlan {
        self.lifecycle.plan()
    }

    pub fn static_fields(&self) -> impl ExactSizeIterator<Item = &PreliminaryMirStaticField> {
        self.preliminary.static_fields()
    }

    pub fn static_initializers(
        &self,
    ) -> impl ExactSizeIterator<Item = &PreliminaryMirStaticInitializer> {
        self.preliminary.static_initializers()
    }

    /// Returns the typed storage, values, blocks, and publication boundary for
    /// one program-owned lifecycle initializer identity.
    pub fn static_initializer(
        &self,
        id: StaticInitializerId,
    ) -> Option<&PreliminaryMirStaticInitializer> {
        self.preliminary.static_initializer(id)
    }

    pub fn has_static_initializers(&self) -> bool {
        self.preliminary.has_static_initializers()
    }

    pub(crate) const fn preliminary(&self) -> &PreliminaryMirProgram {
        &self.preliminary
    }

    pub(crate) fn into_executable_parts(self) -> (PreliminaryMirProgram, MirPlannedLifecycle) {
        (self.preliminary, self.lifecycle)
    }

    #[cfg(test)]
    pub(crate) fn preliminary_mut_for_test(&mut self) -> &mut PreliminaryMirProgram {
        &mut self.preliminary
    }

    #[cfg(test)]
    pub(crate) fn lifecycle_mut_for_test(&mut self) -> &mut MirPlannedLifecycle {
        &mut self.lifecycle
    }

    #[cfg(test)]
    pub(crate) fn planning_report_mut_for_test(&mut self) -> &mut StaticLifecyclePlanningReport {
        &mut self.report
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
