//! Target-independent execution-dependency and whole-program root contract.
//!
//! This facade currently owns only semantic vocabulary and deterministic
//! comparison. Root collection, dependency extraction, possible-target
//! resolution, closure solving, and dumps will become focused sibling owners
//! behind this facade as those capabilities are implemented. It performs no
//! analysis or transformation today.
//!
//! Maintenance rule: every new MIR operation that can select executable work,
//! and every new implicit lifecycle variant, must update dependency extraction
//! and exhaustive coverage in the same change.

mod model;

pub(crate) use crate::mir::mir_execution_node_key;
pub(crate) use model::{
    mir_dependency_edge_kind_key, mir_reachability_root_reason_key, mir_span_key,
    MirDependencyEdge, MirDependencyEdgeKind, MirDependencyTarget, MirReachabilityRoot,
    MirReachabilityRootReason, MirReachabilityRootTarget, MirRetainedDefinition, MirRuntimeEntity,
    MirSemanticDeclaration,
};

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
