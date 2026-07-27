//! Deterministic normalization and exact logical-path candidate lookup.

mod lookup;
mod model;
mod normalize;

pub use model::{
    CandidateLookupError, CandidateLookupErrorKind, CandidateResolution, ModuleCandidate,
    NormalizedProvider, NormalizedRootSpelling, ProviderNormalizationError,
    ProviderNormalizationErrorKind, ProviderRootConfiguration, ProviderRootKind, ProviderSet,
};
pub(in crate::module) use normalize::lexical_normalize;
pub use normalize::normalize_provider_roots;

#[cfg(test)]
mod tests;
