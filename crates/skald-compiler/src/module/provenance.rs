use std::path::{Path, PathBuf};

use crate::{
    identity::{ModuleId, PackageId, ProviderId},
    source::SourceId,
};

use super::ModulePath;

/// Filesystem spellings retained for one module source instance.
///
/// These paths support loading and diagnostics. They do not participate in
/// logical module identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleSourceLocation {
    root_relative_path: PathBuf,
    display_source_path: PathBuf,
    canonical_io_path: Option<PathBuf>,
}

impl ModuleSourceLocation {
    pub fn new(
        root_relative_path: PathBuf,
        display_source_path: PathBuf,
        canonical_io_path: Option<PathBuf>,
    ) -> Self {
        Self {
            root_relative_path,
            display_source_path,
            canonical_io_path,
        }
    }

    /// Returns the lexical path below the provider root.
    pub fn root_relative_path(&self) -> &Path {
        &self.root_relative_path
    }

    /// Returns the stable user-facing source spelling selected by loading.
    pub fn display_source_path(&self) -> &Path {
        &self.display_source_path
    }

    /// Returns the resolved I/O target, when loading retained one.
    pub fn canonical_io_path(&self) -> Option<&Path> {
        self.canonical_io_path.as_deref()
    }
}

/// Request-local provenance for one loaded logical module instance.
///
/// Logical path plus request-local identities establish semantic ownership.
/// Canonical filesystem targets are retained only for I/O, diagnostics, and
/// possible byte caching; they never define module identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleProvenance {
    module_id: ModuleId,
    module_path: ModulePath,
    source_id: SourceId,
    provider_id: ProviderId,
    package_id: PackageId,
    source_location: ModuleSourceLocation,
}

impl ModuleProvenance {
    pub fn new(
        module_id: ModuleId,
        module_path: ModulePath,
        source_id: SourceId,
        provider_id: ProviderId,
        package_id: PackageId,
        source_location: ModuleSourceLocation,
    ) -> Self {
        Self {
            module_id,
            module_path,
            source_id,
            provider_id,
            package_id,
            source_location,
        }
    }

    pub const fn module_id(&self) -> ModuleId {
        self.module_id
    }

    pub fn module_path(&self) -> &ModulePath {
        &self.module_path
    }

    pub const fn source_id(&self) -> SourceId {
        self.source_id
    }

    pub const fn provider_id(&self) -> ProviderId {
        self.provider_id
    }

    pub const fn package_id(&self) -> PackageId {
        self.package_id
    }

    pub const fn source_location(&self) -> &ModuleSourceLocation {
        &self.source_location
    }
}
