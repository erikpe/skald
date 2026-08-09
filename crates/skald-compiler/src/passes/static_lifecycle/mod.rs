//! Whole-program static-field effect inference over preliminary MIR.
//!
//! This pass owns target-independent call/lifecycle graph construction and
//! transitive effect propagation. Lifetime ordering and cycle diagnostics are
//! deliberately left to the subsequent planning pass.

mod dump;
mod extract;
mod model;
mod solve;

pub use dump::dump_static_effects;
pub use model::{
    StaticAccessEvidence, StaticAccessKind, StaticArrayLifecycleOperation,
    StaticClassLifecycleOperation, StaticEffectAnalysis, StaticEffectEdge, StaticEffectEdgeKind,
    StaticEffectNode, StaticEffectPhase, StaticEffectSummary,
};

use crate::mir::PreliminaryMirProgram;

/// Infers direct and transitive static-field effects for every executable MIR
/// body and every compiler-generated lifecycle operation in the closed program.
pub fn infer_static_effects(program: &PreliminaryMirProgram) -> StaticEffectAnalysis {
    solve::solve(extract::extract(program))
}

#[cfg(test)]
mod tests;
