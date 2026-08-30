//! Whole-program static-effect analysis over preliminary or final MIR.

mod dump;
pub(super) mod extract;
mod model;
pub(super) mod root_effects;
pub(super) mod roots;
mod solve;

pub use dump::dump_static_effects;
pub(super) use dump::write_node;
pub(super) use model::{edge_key, span_key};
pub use model::{
    StaticAccessEvidence, StaticEffectAnalysis, StaticEffectEdge, StaticEffectEdgeKind,
    StaticEffectSummary, StaticFunctionValueCandidates, StaticFunctionValueTarget,
};

use crate::mir::{PreliminaryMirProgram, StaticLifecycleAuthority};

#[cfg(test)]
use crate::mir::{
    StaticAccessKind, StaticArrayLifecycleOperation, StaticClassLifecycleOperation,
    StaticEffectNode, StaticEffectPhase,
};

pub(super) fn infer_static_effects_with_roots(
    program: &PreliminaryMirProgram,
) -> (StaticEffectAnalysis, StaticLifecycleAuthority) {
    let graph = extract::extract(program);
    let root_effects = root_effects::analyze(program, &graph)
        .expect("verified preliminary MIR must have valid lifecycle-root identities");
    let effects = solve::solve(graph);
    (effects, root_effects)
}

/// Infers direct and transitive static-field effects for every executable MIR
/// body and every compiler-generated lifecycle operation in the closed program.
pub fn infer_static_effects(program: &PreliminaryMirProgram) -> StaticEffectAnalysis {
    solve::solve(extract::extract(program))
}

#[cfg(test)]
mod tests;
