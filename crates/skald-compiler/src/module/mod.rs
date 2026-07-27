//! Logical module paths, filesystem providers, and request-local provenance.
//!
//! Physical paths support provider lookup and diagnostics but never replace
//! logical paths or typed identities as semantic identity.

mod path;
mod provenance;
mod provider;

pub use path::{ModulePath, ModulePathError, ModulePathErrorKind};
pub use provenance::{ModuleProvenance, ModuleSourceLocation};
pub use provider::{
    normalize_provider_roots, CandidateLookupError, CandidateLookupErrorKind, CandidateResolution,
    ModuleCandidate, NormalizedProvider, NormalizedRootSpelling, ProviderNormalizationError,
    ProviderNormalizationErrorKind, ProviderRootConfiguration, ProviderRootKind, ProviderSet,
};

#[cfg(test)]
mod tests;
