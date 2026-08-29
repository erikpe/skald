use std::{
    fmt,
    path::{Path, PathBuf},
};

use crate::{
    backend::{RuntimeTracePolicy, Target},
    module::{ModulePath, ProviderRootConfiguration},
};

/// The selected source identity from which reachable compilation begins.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntrySelector {
    File(PathBuf),
    Module(ModulePath),
}

impl EntrySelector {
    /// Resolves the two mutually exclusive entry-option forms.
    ///
    /// Filesystem existence and module-provider lookup are deliberately
    /// deferred to loading.
    pub fn from_options(
        positional_file: Option<PathBuf>,
        logical_module: Option<ModulePath>,
    ) -> Result<Self, EntrySelectionError> {
        match (positional_file, logical_module) {
            (Some(path), None) => Ok(Self::File(path)),
            (None, Some(path)) => Ok(Self::Module(path)),
            (None, None) => Err(EntrySelectionError::Missing),
            (Some(_), Some(_)) => Err(EntrySelectionError::Conflicting),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntrySelectionError {
    Missing,
    Conflicting,
}

impl fmt::Display for EntrySelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => {
                formatter.write_str("exactly one file or logical module entry is required")
            }
            Self::Conflicting => {
                formatter.write_str("file and logical module entries are mutually exclusive")
            }
        }
    }
}

impl std::error::Error for EntrySelectionError {}

/// Selection of the standard-library provider for one request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StandardLibrarySelection {
    Default,
    Replacement(PathBuf),
    Disabled,
}

impl StandardLibrarySelection {
    /// Resolves replacement and disabling options without touching the
    /// filesystem.
    pub fn from_options(
        replacement_root: Option<PathBuf>,
        disabled: bool,
    ) -> Result<Self, StandardLibrarySelectionError> {
        match (replacement_root, disabled) {
            (None, false) => Ok(Self::Default),
            (Some(path), false) => Ok(Self::Replacement(path)),
            (None, true) => Ok(Self::Disabled),
            (Some(_), true) => Err(StandardLibrarySelectionError::Conflicting),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardLibrarySelectionError {
    Conflicting,
}

impl fmt::Display for StandardLibrarySelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "a replacement standard-library root and disabled standard library are mutually exclusive",
        )
    }
}

impl std::error::Error for StandardLibrarySelectionError {}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ArtifactKind {
    #[default]
    Executable,
    Assembly,
}

/// Driver-owned output selection for one compilation request.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArtifactOptions {
    kind: ArtifactKind,
    output: Option<PathBuf>,
    runtime_trace: RuntimeTracePolicy,
}

impl ArtifactOptions {
    pub fn new(kind: ArtifactKind, output: Option<PathBuf>) -> Self {
        Self {
            kind,
            output,
            runtime_trace: RuntimeTracePolicy::Enabled,
        }
    }

    pub fn with_runtime_trace_policy(mut self, policy: RuntimeTracePolicy) -> Self {
        self.runtime_trace = policy;
        self
    }

    pub const fn kind(&self) -> ArtifactKind {
        self.kind
    }

    pub fn output(&self) -> Option<&Path> {
        self.output.as_deref()
    }

    pub const fn runtime_trace(&self) -> RuntimeTracePolicy {
        self.runtime_trace
    }
}

/// Process-dependent paths captured once at request construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationEnvironment {
    working_directory: PathBuf,
    default_standard_library_root: PathBuf,
}

impl CompilationEnvironment {
    pub fn new(working_directory: PathBuf, default_standard_library_root: PathBuf) -> Self {
        Self {
            working_directory,
            default_standard_library_root,
        }
    }

    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }

    pub fn default_standard_library_root(&self) -> &Path {
        &self.default_standard_library_root
    }
}

/// Complete typed input to the request-based compiler driver.
///
/// Construction records configuration only. Provider normalization,
/// filesystem validation, reachable loading, and compilation remain explicit
/// driver operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationRequest {
    entry: EntrySelector,
    module_roots: Vec<PathBuf>,
    standard_library: StandardLibrarySelection,
    target: Target,
    artifact: ArtifactOptions,
    environment: CompilationEnvironment,
}

impl CompilationRequest {
    pub fn new(
        entry: EntrySelector,
        module_roots: Vec<PathBuf>,
        standard_library: StandardLibrarySelection,
        target: Target,
        artifact: ArtifactOptions,
        environment: CompilationEnvironment,
    ) -> Self {
        Self {
            entry,
            module_roots,
            standard_library,
            target,
            artifact,
            environment,
        }
    }

    pub const fn entry(&self) -> &EntrySelector {
        &self.entry
    }

    pub fn module_roots(&self) -> &[PathBuf] {
        &self.module_roots
    }

    pub const fn standard_library(&self) -> &StandardLibrarySelection {
        &self.standard_library
    }

    pub const fn target(&self) -> Target {
        self.target
    }

    pub const fn artifact(&self) -> &ArtifactOptions {
        &self.artifact
    }

    pub const fn runtime_trace(&self) -> RuntimeTracePolicy {
        self.artifact.runtime_trace()
    }

    pub const fn environment(&self) -> &CompilationEnvironment {
        &self.environment
    }

    /// Expands request root selections into provider normalization inputs.
    ///
    /// This performs no filesystem access. The provider layer remains the
    /// owner of normalization, coalescing, validation, and identity order.
    pub fn provider_root_configurations(&self) -> Vec<ProviderRootConfiguration> {
        let mut configurations = self
            .module_roots
            .iter()
            .cloned()
            .map(ProviderRootConfiguration::module_root)
            .collect::<Vec<_>>();
        match &self.standard_library {
            StandardLibrarySelection::Default => {
                configurations.push(ProviderRootConfiguration::standard_library(
                    self.environment.default_standard_library_root.clone(),
                ));
            }
            StandardLibrarySelection::Replacement(root) => {
                configurations.push(ProviderRootConfiguration::standard_library(root.clone()));
            }
            StandardLibrarySelection::Disabled => {}
        }
        configurations
    }
}
