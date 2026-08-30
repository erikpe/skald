//! Target-independent execution-dependency and whole-program root contract.
//!
//! This facade owns semantic vocabulary, deterministic comparison, and the
//! shared read-only extraction of executable and lifecycle dependencies. Root
//! collection, closure solving, and dumps will become focused sibling owners
//! behind it as those capabilities are implemented. Extraction performs no
//! reachability pruning or MIR transformation.
//!
//! Maintenance rule: every new MIR operation that can select executable work,
//! and every new implicit lifecycle variant, must update dependency extraction
//! and exhaustive coverage in the same change.

mod definitions;
mod error;
mod extract;
mod lifecycle;
mod model;
mod target;

pub(crate) use crate::mir::mir_execution_node_key;
pub(crate) use definitions::MirExecutableDefinitionView;
pub(crate) use error::MirDependencyExtractionError;
pub(crate) use extract::{
    extract_final_dependencies, extract_final_dependency_parts, extract_preliminary_dependencies,
    MirDependencyExtraction,
};
pub(crate) use model::{
    mir_dependency_edge_kind_key, mir_reachability_root_reason_key, mir_span_key,
    MirCallableAddressFormation, MirDependencyEdge, MirDependencyEdgeKind, MirDependencyRecord,
    MirDependencyRegion, MirDependencyTarget, MirIndirectCallSite, MirReachabilityRoot,
    MirReachabilityRootReason, MirReachabilityRootTarget, MirRetainedDefinition, MirRuntimeEntity,
    MirSemanticDeclaration,
};

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
