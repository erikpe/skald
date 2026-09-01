//! Entry-rooted static-activation analysis boundary.
//!
//! This module owns only field-activation policy and its deterministic result
//! vocabulary. Neutral execution identities, dependency kinds, lifecycle
//! expansion, and possible-target selection remain owned by
//! `passes::reachability`. The fixed point computed here is the semantic input
//! to exact lifecycle planning.

mod dump;
mod error;
mod model;
mod solve;

use crate::{
    mir::PreliminaryMirProgram,
    passes::reachability::{extract_preliminary_dependencies, MirDependencyExtraction},
};

#[cfg(test)]
pub(super) use dump::dump_static_activation;
pub(super) use error::StaticActivationAnalysisError;

#[cfg(test)]
pub(super) use model::StaticActivationTrigger;
pub(super) use model::{
    static_activation_edge_key, static_activation_node_key, StaticActivationAnalysis,
    StaticActivationAnalysisParts, StaticActivationEdge, StaticActivationExecution,
    StaticActivationField, StaticActivationNode, StaticActivationRoot, StaticActivationWitness,
};

pub(super) fn analyze_static_activation(
    program: &PreliminaryMirProgram,
) -> Result<StaticActivationAnalysis, StaticActivationAnalysisError> {
    let dependencies = extract_preliminary_dependencies(program)?;
    analyze_static_activation_from_dependencies(program, &dependencies)
}

pub(super) fn analyze_static_activation_from_dependencies(
    program: &PreliminaryMirProgram,
    dependencies: &MirDependencyExtraction,
) -> Result<StaticActivationAnalysis, StaticActivationAnalysisError> {
    solve::analyze_static_activation_from_dependencies(program, dependencies)
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
