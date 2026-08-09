//! Planning failure model and MIR lifecycle-schema facade.

use crate::diagnostics::{Diagnostic, Diagnostics};

pub use crate::mir::{
    PlannedMirProgram, StaticLifecyclePlan, StaticLifetimeDependency, StaticLifetimeEvidence,
    StaticLifetimePhase,
};

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
