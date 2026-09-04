//! Read-only opportunity censuses over verified final MIR.
//!
//! These analyses describe optimization opportunities without registering a
//! pass, mutating MIR, or becoming part of ordinary compilation.

mod cast_model;
mod model;
mod primitive_cast;
mod scalar_spill;

pub use cast_model::{
    PrimitiveCastBlocker, PrimitiveCastCallableObservation, PrimitiveCastConsumer,
    PrimitiveCastCount, PrimitiveCastDisposition, PrimitiveCastObservation,
    PrimitiveCastObservationCounts, PrimitiveCastShape,
};
pub use model::{
    ScalarSpillBlocker, ScalarSpillCallableObservation, ScalarSpillConsumer, ScalarSpillCount,
    ScalarSpillDepth, ScalarSpillProvenanceCounts, ScalarSpillProvenanceObservation,
    ScalarSpillUnlock,
};
pub use primitive_cast::analyze_redundant_primitive_casts;
pub use scalar_spill::analyze_scalar_spill_provenance;

#[cfg(test)]
mod tests;
