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

use crate::{
    mir::{StaticLifecycleAuthority, VerifiedPreliminaryMirProgram},
    passes::reachability::MirDependencyExtraction,
};

#[cfg(test)]
use crate::mir::{
    MirArrayLifecycleOperation, MirClassLifecycleOperation, MirExecutionNode, StaticAccessKind,
    StaticEffectPhase,
};

#[cfg(test)]
pub(super) fn infer_static_effects_with_roots(
    program: &VerifiedPreliminaryMirProgram,
) -> (StaticEffectAnalysis, StaticLifecycleAuthority) {
    let graph = extract::extract(program);
    infer_static_effects_with_roots_from_graph(program, graph)
}

pub(super) fn infer_static_effects_with_roots_for_fields_from_dependencies(
    program: &VerifiedPreliminaryMirProgram,
    dependencies: &MirDependencyExtraction,
    active_fields: &[crate::identity::StaticFieldId],
) -> (StaticEffectAnalysis, StaticLifecycleAuthority) {
    let graph = extract::extract_from_dependencies(dependencies);
    let root_effects = root_effects::analyze_for_fields(program, &graph, active_fields)
        .expect("verified preliminary MIR must have valid lifecycle-root identities");
    let effects = solve::solve(graph);
    (effects, root_effects)
}

#[cfg(test)]
fn infer_static_effects_with_roots_from_graph(
    program: &VerifiedPreliminaryMirProgram,
    graph: extract::ExtractedGraph,
) -> (StaticEffectAnalysis, StaticLifecycleAuthority) {
    let root_effects = root_effects::analyze(program, &graph)
        .expect("verified preliminary MIR must have valid lifecycle-root identities");
    let effects = solve::solve(graph);
    (effects, root_effects)
}

/// Infers direct and transitive static-field effects for every executable MIR
/// body and every compiler-generated lifecycle operation in the verified
/// closed program.
///
/// Raw preliminary MIR cannot bypass its verification boundary:
///
/// ```compile_fail
/// use skald_compiler::{
///     mir::PreliminaryMirProgram,
///     passes::static_lifecycle::infer_static_effects,
/// };
///
/// fn analyze(program: &PreliminaryMirProgram) {
///     let _ = infer_static_effects(program);
/// }
/// ```
pub fn infer_static_effects(program: &VerifiedPreliminaryMirProgram) -> StaticEffectAnalysis {
    solve::solve(extract::extract(program))
}

#[cfg(test)]
mod tests;
