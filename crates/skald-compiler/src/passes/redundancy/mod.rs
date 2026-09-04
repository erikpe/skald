//! Read-only opportunity censuses over explicitly verified MIR products.
//!
//! These analyses describe optimization opportunities without registering a
//! pass, mutating MIR, or becoming part of ordinary compilation. Distinct
//! entry points make proof-rich inspection and normalized final analysis
//! explicit at call sites.

mod cast_model;
mod count;
mod cse_model;
mod local_cse;
mod model;
mod primitive_cast;
mod scalar_spill;
mod site;

pub use cast_model::{
    PrimitiveCastBlocker, PrimitiveCastCallableObservation, PrimitiveCastConsumer,
    PrimitiveCastCount, PrimitiveCastDisposition, PrimitiveCastObservation,
    PrimitiveCastObservationCounts, PrimitiveCastShape,
};
pub use cse_model::{
    LocalCseBlocker, LocalCseCallableObservation, LocalCseConsumer, LocalCseCount,
    LocalCseExcludedFamily, LocalCseObservation, LocalCseObservationCounts,
    LocalCseOperationFamily, LocalCseOutcome,
};
pub use local_cse::{
    analyze_local_primitive_common_subexpressions,
    analyze_proof_local_primitive_common_subexpressions,
};
pub use model::{
    ScalarSpillBlocker, ScalarSpillCallableObservation, ScalarSpillConsumer, ScalarSpillCount,
    ScalarSpillDepth, ScalarSpillProvenanceCounts, ScalarSpillProvenanceObservation,
    ScalarSpillUnlock,
};
pub use primitive_cast::{
    analyze_proof_redundant_primitive_casts, analyze_redundant_primitive_casts,
};
pub use scalar_spill::{analyze_proof_scalar_spill_provenance, analyze_scalar_spill_provenance};
pub use site::{
    RedundancySiteClassification, RedundancySiteExample,
    REDUNDANCY_SITE_EXAMPLES_PER_CLASSIFICATION,
};

#[cfg(test)]
mod tests;
