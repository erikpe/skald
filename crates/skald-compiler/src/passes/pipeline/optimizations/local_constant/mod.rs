//! Seal-local convergent constant analysis.
//!
//! The facade deliberately exposes immutable answers, not graph or worklist
//! machinery. Every answer belongs to the verified callable snapshot supplied
//! to [`solve_local_constants`] and must be discarded after a MIR rewrite.

mod carrier;
mod evidence;
mod graph;
mod logical;
mod solve;
mod view;

pub(in crate::passes::pipeline::optimizations) use evidence::{
    checked_carrier_plan_evidence, CheckedCarrierPlanEvidence, CheckedCarrierPlanRole,
};
// Later roadmap stages still exercise solution queries not yet used by the
// migrated primitive and checked consumers.
#[allow(unused_imports)]
pub(in crate::passes::pipeline::optimizations) use solve::{
    solve_local_constants, LocalConstantAnalysisError, LocalConstantFact, LocalConstantIdentity,
    LocalConstantProvenance, LocalConstantProvenanceCategory, LocalConstantSolution,
    LogicalSelection, LogicalSelectionKind, RetainedCheckedFailure,
};
pub(in crate::passes::pipeline::optimizations) use view::BlockLocalConstantView;

#[cfg(test)]
mod tests;
