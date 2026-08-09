use std::{
    fmt, io,
    path::{Path, PathBuf},
};

use crate::identity::{PackageId, ProviderId};

use super::super::ModulePath;

/// The configuration role through which a filesystem root was supplied.
///
/// Roles are provenance only. They do not establish lookup precedence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderRootKind {
    ModuleRoot,
    StandardLibrary,
}

/// One root spelling supplied by request construction.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProviderRootConfiguration {
    kind: ProviderRootKind,
    path: PathBuf,
}

impl ProviderRootConfiguration {
    pub fn module_root(path: PathBuf) -> Self {
        Self {
            kind: ProviderRootKind::ModuleRoot,
            path,
        }
    }

    pub fn standard_library(path: PathBuf) -> Self {
        Self {
            kind: ProviderRootKind::StandardLibrary,
            path,
        }
    }

    pub const fn kind(&self) -> ProviderRootKind {
        self.kind
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// One configured spelling retained after normalization.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct NormalizedRootSpelling {
    configuration: ProviderRootConfiguration,
    absolute_path: PathBuf,
    lexical_path: PathBuf,
}

impl NormalizedRootSpelling {
    pub(super) fn new(
        configuration: ProviderRootConfiguration,
        absolute_path: PathBuf,
        lexical_path: PathBuf,
    ) -> Self {
        Self {
            configuration,
            absolute_path,
            lexical_path,
        }
    }

    pub const fn configuration(&self) -> &ProviderRootConfiguration {
        &self.configuration
    }

    /// Returns the absolute lexical spelling before root canonicalization.
    pub fn absolute_path(&self) -> &Path {
        &self.absolute_path
    }

    /// Returns the absolute spelling with ordinary `.` and `..` removed.
    ///
    /// Later positional-entry containment uses this lexical form. Root I/O
    /// canonicalization uses `absolute_path` so symlink-plus-`..` behavior
    /// remains the host filesystem's behavior.
    pub fn lexical_path(&self) -> &Path {
        &self.lexical_path
    }
}

/// One normalized filesystem provider in deterministic provider-ID order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedProvider {
    id: ProviderId,
    package_id: PackageId,
    canonical_root: PathBuf,
    display_root: PathBuf,
    spellings: Vec<NormalizedRootSpelling>,
}

impl NormalizedProvider {
    pub(super) fn new(
        id: ProviderId,
        package_id: PackageId,
        canonical_root: PathBuf,
        display_root: PathBuf,
        spellings: Vec<NormalizedRootSpelling>,
    ) -> Self {
        Self {
            id,
            package_id,
            canonical_root,
            display_root,
            spellings,
        }
    }

    pub const fn id(&self) -> ProviderId {
        self.id
    }

    pub const fn package_id(&self) -> PackageId {
        self.package_id
    }

    pub fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    pub fn display_root(&self) -> &Path {
        &self.display_root
    }

    pub fn spellings(&self) -> &[NormalizedRootSpelling] {
        &self.spellings
    }
}

/// Deterministically normalized unordered provider union.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderSet {
    pub(super) providers: Vec<NormalizedProvider>,
}

impl ProviderSet {
    pub(super) fn new(providers: Vec<NormalizedProvider>) -> Self {
        Self { providers }
    }

    pub fn providers(&self) -> &[NormalizedProvider] {
        &self.providers
    }

    pub fn resolve(
        &self,
        module_path: &ModulePath,
    ) -> Result<CandidateResolution, Vec<CandidateLookupError>> {
        super::lookup::resolve_candidates(self, module_path)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderNormalizationErrorKind {
    WorkingDirectoryNotAbsolute,
    Canonicalization(io::ErrorKind),
    NotDirectory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderNormalizationError {
    kind: ProviderNormalizationErrorKind,
    configuration: Option<ProviderRootConfiguration>,
    path: PathBuf,
}

impl ProviderNormalizationError {
    pub(super) fn working_directory(path: PathBuf) -> Self {
        Self {
            kind: ProviderNormalizationErrorKind::WorkingDirectoryNotAbsolute,
            configuration: None,
            path,
        }
    }

    pub(super) fn root(
        kind: ProviderNormalizationErrorKind,
        configuration: ProviderRootConfiguration,
        path: PathBuf,
    ) -> Self {
        Self {
            kind,
            configuration: Some(configuration),
            path,
        }
    }

    pub const fn kind(&self) -> ProviderNormalizationErrorKind {
        self.kind
    }

    pub const fn configuration(&self) -> Option<&ProviderRootConfiguration> {
        self.configuration.as_ref()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Display for ProviderNormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ProviderNormalizationErrorKind::WorkingDirectoryNotAbsolute => write!(
                formatter,
                "captured working directory `{}` is not absolute",
                self.path.display()
            ),
            ProviderNormalizationErrorKind::Canonicalization(kind) => write!(
                formatter,
                "cannot normalize provider root `{}`: {kind:?}",
                self.path.display()
            ),
            ProviderNormalizationErrorKind::NotDirectory => write!(
                formatter,
                "provider root `{}` is not a directory",
                self.path.display()
            ),
        }
    }
}

impl std::error::Error for ProviderNormalizationError {}

/// One provider's source candidate for one exact logical path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleCandidate {
    module_path: ModulePath,
    provider_id: ProviderId,
    package_id: PackageId,
    root_relative_path: PathBuf,
    display_source_path: PathBuf,
    trace_source_path: PathBuf,
    canonical_io_path: PathBuf,
}

impl ModuleCandidate {
    pub(in crate::module) fn new(
        module_path: ModulePath,
        provider_id: ProviderId,
        package_id: PackageId,
        root_relative_path: PathBuf,
        display_source_path: PathBuf,
        canonical_io_path: PathBuf,
    ) -> Self {
        let trace_source_path = root_relative_path.clone();
        Self {
            module_path,
            provider_id,
            package_id,
            root_relative_path,
            display_source_path,
            trace_source_path,
            canonical_io_path,
        }
    }

    pub const fn module_path(&self) -> &ModulePath {
        &self.module_path
    }

    pub const fn provider_id(&self) -> ProviderId {
        self.provider_id
    }

    pub const fn package_id(&self) -> PackageId {
        self.package_id
    }

    pub fn root_relative_path(&self) -> &Path {
        &self.root_relative_path
    }

    pub fn display_source_path(&self) -> &Path {
        &self.display_source_path
    }

    pub fn trace_source_path(&self) -> &Path {
        &self.trace_source_path
    }

    /// Returns the physical target retained only for I/O and diagnostics.
    pub fn canonical_io_path(&self) -> &Path {
        &self.canonical_io_path
    }

    pub(in crate::module) fn with_display_source_path(mut self, path: PathBuf) -> Self {
        self.display_source_path = path;
        self
    }

    pub(in crate::module) fn with_trace_source_path(mut self, path: PathBuf) -> Self {
        self.trace_source_path = path;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CandidateResolution {
    Missing {
        module_path: ModulePath,
    },
    Unique(ModuleCandidate),
    Ambiguous {
        module_path: ModulePath,
        candidates: Vec<ModuleCandidate>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateLookupErrorKind {
    CaseMismatch,
    CaseCollision,
    UnreadableDirectory(io::ErrorKind),
    NonDirectoryComponent,
    SymlinkResolution(io::ErrorKind),
    NonRegularFile,
    UnreadableFile(io::ErrorKind),
    Canonicalization(io::ErrorKind),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateLookupError {
    kind: CandidateLookupErrorKind,
    module_path: ModulePath,
    provider_id: ProviderId,
    path: PathBuf,
    conflicting_paths: Vec<PathBuf>,
}

impl CandidateLookupError {
    pub(super) fn new(
        kind: CandidateLookupErrorKind,
        module_path: ModulePath,
        provider_id: ProviderId,
        path: PathBuf,
        conflicting_paths: Vec<PathBuf>,
    ) -> Self {
        Self {
            kind,
            module_path,
            provider_id,
            path,
            conflicting_paths,
        }
    }

    pub const fn kind(&self) -> CandidateLookupErrorKind {
        self.kind
    }

    pub const fn module_path(&self) -> &ModulePath {
        &self.module_path
    }

    pub const fn provider_id(&self) -> ProviderId {
        self.provider_id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn conflicting_paths(&self) -> &[PathBuf] {
        &self.conflicting_paths
    }
}

impl fmt::Display for CandidateLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot resolve module `{}` from provider {} at `{}`: {:?}",
            self.module_path,
            self.provider_id,
            self.path.display(),
            self.kind
        )
    }
}

impl std::error::Error for CandidateLookupError {}
