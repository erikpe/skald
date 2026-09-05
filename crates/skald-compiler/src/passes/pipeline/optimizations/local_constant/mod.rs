//! Seal-local convergent constant analysis.
//!
//! The facade deliberately exposes immutable answers, not graph or worklist
//! machinery. Every answer belongs to the verified callable snapshot supplied
//! to [`solve_local_constants`] and must be discarded after a MIR rewrite.

mod carrier;
mod graph;
mod logical;
mod solve;

// The public-within-optimizer facade is intentionally staged one milestone
// before its first production consumer.
#[allow(unused_imports)]
pub(in crate::passes::pipeline::optimizations) use solve::{
    solve_local_constants, LocalConstantAnalysisError, LocalConstantFact, LocalConstantIdentity,
    LocalConstantProvenance, LocalConstantProvenanceCategory, LocalConstantSolution,
    LogicalSelection, LogicalSelectionKind, RetainedCheckedFailure,
};

#[cfg(test)]
mod tests;
