//! Snapshot-bound proof analysis access and deterministic usage accounting.

mod context;
mod session;
mod usage;

pub(in crate::passes::pipeline) use context::{MirProofPassContext, MirProofTransitionContext};
pub(super) use session::MirProofSnapshotAnalysis;
pub use usage::{MirSnapshotAnalysisKind, MirSnapshotAnalysisUsage};

#[cfg(test)]
mod tests;
