//! Logical module paths, filesystem providers, and request-local provenance.
//!
//! Physical paths support provider lookup and diagnostics but never replace
//! logical paths or typed identities as semantic identity.

mod graph;
mod metadata;
mod path;
mod provenance;
mod provider;

pub use graph::{
    dump_module_graph, load_module_graph, CompilerDependencyEvidence, CompilerDependencyKind,
    LoadedModule, ModuleGraph, ModuleGraphLoadFailure, ModuleImportEdge,
};
pub(crate) use graph::{
    load_module_graph_measured, MeasuredModuleGraphLoad, ModuleLoadMeasurementOptions,
    ModuleLoadMeasurements, ModuleParseStage,
};
pub use metadata::{ProgramModuleTable, ProgramModuleTableError};
pub use path::{ModulePath, ModulePathError, ModulePathErrorKind};
pub use provenance::{ModuleProvenance, ModuleSourceLocation};
pub use provider::{
    normalize_provider_roots, CandidateLookupError, CandidateLookupErrorKind, CandidateResolution,
    ModuleCandidate, NormalizedProvider, NormalizedRootSpelling, ProviderNormalizationError,
    ProviderNormalizationErrorKind, ProviderRootConfiguration, ProviderRootKind, ProviderSet,
};

#[cfg(test)]
mod tests;
