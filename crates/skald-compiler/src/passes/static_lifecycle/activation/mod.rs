//! Entry-rooted static-activation analysis boundary.
//!
//! This module owns only field-activation policy and its deterministic result
//! vocabulary. Neutral execution identities, dependency kinds, lifecycle
//! expansion, and possible-target selection remain owned by
//! `passes::reachability`. Extraction and fixed-point solving are added in
//! later implementation slices; declaring this module does not change eager
//! static-lifecycle behavior.

mod model;

pub(super) use model::{
    static_activation_edge_key, static_activation_node_key, StaticActivationAnalysis,
    StaticActivationAnalysisParts, StaticActivationCounts, StaticActivationEdge,
    StaticActivationExecution, StaticActivationField, StaticActivationNode, StaticActivationRoot,
    StaticActivationTrigger, StaticActivationWitness,
};

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
