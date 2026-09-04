//! Read-only opportunity censuses over verified final MIR.
//!
//! These analyses describe optimization opportunities without registering a
//! pass, mutating MIR, or becoming part of ordinary compilation.

mod model;
mod scalar_spill;

pub use model::{
    ScalarSpillBlocker, ScalarSpillCallableObservation, ScalarSpillConsumer, ScalarSpillCount,
    ScalarSpillDepth, ScalarSpillProvenanceCounts, ScalarSpillProvenanceObservation,
    ScalarSpillUnlock,
};
pub use scalar_spill::analyze_scalar_spill_provenance;

#[cfg(test)]
mod tests;
