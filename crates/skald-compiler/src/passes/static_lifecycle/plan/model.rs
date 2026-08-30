//! Planned phase product, inspection report, and planning failure model.

use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::mir::model::StaticEffectAnalysis;
use crate::{
    identity::StaticInitializerId,
    mir::{
        MirProgramLifecycle, PreliminaryMirProgram, PreliminaryMirStaticField,
        PreliminaryMirStaticInitializer, StaticLifecycleAuthority,
    },
};

pub use crate::mir::model::{
    StaticLifecyclePlan, StaticLifetimeDependency, StaticLifetimeEvidence, StaticLifetimePhase,
};

/// Analysis evidence retained for deterministic inspection of lifecycle
/// planning, but deliberately excluded from backend-consumable MIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLifecyclePlanningReport {
    analysis: StaticEffectAnalysis,
    dependencies: Vec<StaticLifetimeDependency>,
}

impl StaticLifecyclePlanningReport {
    pub(crate) const fn new(
        analysis: StaticEffectAnalysis,
        dependencies: Vec<StaticLifetimeDependency>,
    ) -> Self {
        Self {
            analysis,
            dependencies,
        }
    }

    pub const fn analysis(&self) -> &StaticEffectAnalysis {
        &self.analysis
    }

    pub fn dependencies(&self) -> &[StaticLifetimeDependency] {
        &self.dependencies
    }
}

/// Preliminary MIR plus executable lifecycle metadata and planning-only
/// analysis evidence.
///
/// The wrapped preliminary program remains private, so no backend can consume
/// initializer bodies before lifecycle coordinator synthesis. Consuming this
/// product for synthesis drops the planning report at the phase boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedMirProgram {
    preliminary: PreliminaryMirProgram,
    lifecycle: MirProgramLifecycle,
    report: StaticLifecyclePlanningReport,
}

impl PlannedMirProgram {
    pub(crate) const fn new(
        preliminary: PreliminaryMirProgram,
        lifecycle: MirProgramLifecycle,
        report: StaticLifecyclePlanningReport,
    ) -> Self {
        Self {
            preliminary,
            lifecycle,
            report,
        }
    }

    pub const fn lifecycle_mir(&self) -> &MirProgramLifecycle {
        &self.lifecycle
    }

    pub const fn planning_report(&self) -> &StaticLifecyclePlanningReport {
        &self.report
    }

    pub fn authority(&self) -> &StaticLifecycleAuthority {
        self.lifecycle.proof().authority()
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

    pub(crate) fn into_executable_parts(self) -> (PreliminaryMirProgram, MirProgramLifecycle) {
        (self.preliminary, self.lifecycle)
    }

    #[cfg(test)]
    pub(crate) fn preliminary_mut_for_test(&mut self) -> &mut PreliminaryMirProgram {
        &mut self.preliminary
    }

    #[cfg(test)]
    pub(crate) fn lifecycle_mut_for_test(&mut self) -> &mut MirProgramLifecycle {
        &mut self.lifecycle
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
