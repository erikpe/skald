use std::{
    fmt,
    path::{Path, PathBuf},
};

use crate::{
    backend::{RuntimeTracePolicy, Target},
    module::{ModulePath, ProviderRootConfiguration},
    passes::{
        resolve_mir_pass_schedule, MirOptimizationProfile, MirPassSchedule, MirPassScheduleError,
    },
};

/// Target-independent final-MIR optimization policy for one compilation.
///
/// Disabled names are kept in lexical order without duplicates so equivalent
/// option sequences have equal request identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptimizationOptions {
    profile: MirOptimizationProfile,
    disabled_passes: Vec<String>,
}

impl MirOptimizationOptions {
    pub const fn new(profile: MirOptimizationProfile) -> Self {
        Self {
            profile,
            disabled_passes: Vec::new(),
        }
    }

    pub fn with_disabled_pass(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        match self.disabled_passes.binary_search(&name) {
            Ok(_) => {}
            Err(position) => self.disabled_passes.insert(position, name),
        }
        self
    }

    pub const fn profile(&self) -> MirOptimizationProfile {
        self.profile
    }

    pub fn disabled_passes(&self) -> &[String] {
        &self.disabled_passes
    }

    pub(crate) fn resolve_schedule(
        &self,
    ) -> Result<MirPassSchedule, MirOptimizationConfigurationError> {
        match resolve_mir_pass_schedule(
            self.profile,
            self.disabled_passes.iter().map(String::as_str),
        ) {
            Ok(schedule) => Ok(schedule),
            Err(MirPassScheduleError::UnknownNames { names, known_names }) => {
                Err(MirOptimizationConfigurationError {
                    names,
                    known_names,
                    mandatory_normalization: false,
                })
            }
            Err(MirPassScheduleError::MandatoryNormalizationSelection) => {
                Err(MirOptimizationConfigurationError {
                    names: vec!["proof-provenance-normalization".to_owned()],
                    known_names: crate::passes::available_mir_passes()
                        .into_iter()
                        .map(|descriptor| descriptor.name())
                        .collect(),
                    mandatory_normalization: true,
                })
            }
            Err(
                error @ (MirPassScheduleError::InvalidRegistry(_)
                | MirPassScheduleError::UnknownIdentity { .. }
                | MirPassScheduleError::WrongStageOrder { .. }
                | MirPassScheduleError::RepeatedProofTransition { .. }
                | MirPassScheduleError::ProofTransitionAfterFinal { .. }),
            ) => {
                panic!("invalid compiler-owned final-MIR pass policy: {error}")
            }
        }
    }
}

impl Default for MirOptimizationOptions {
    fn default() -> Self {
        Self::new(MirOptimizationProfile::Default)
    }
}

/// Unknown stable pass names in target-independent optimization policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptimizationConfigurationError {
    names: Vec<String>,
    known_names: Vec<&'static str>,
    mandatory_normalization: bool,
}

impl MirOptimizationConfigurationError {
    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn known_names(&self) -> &[&'static str] {
        &self.known_names
    }
}

impl fmt::Display for MirOptimizationConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mandatory_normalization {
            return formatter.write_str(
                "mandatory proof-provenance normalization cannot be selected, disabled, or repeated",
            );
        }
        write!(
            formatter,
            "unknown MIR pass name{}: {}",
            if self.names.len() == 1 { "" } else { "s" },
            self.names
                .iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )?;
        if self.known_names.is_empty() {
            formatter.write_str("; no MIR passes are registered")
        } else {
            write!(
                formatter,
                "; known MIR passes: {}",
                self.known_names
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

impl std::error::Error for MirOptimizationConfigurationError {}

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
    mir_optimization: MirOptimizationOptions,
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
            mir_optimization: MirOptimizationOptions::default(),
            environment,
        }
    }

    pub fn with_mir_optimization(mut self, options: MirOptimizationOptions) -> Self {
        self.mir_optimization = options;
        self
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

    pub const fn mir_optimization(&self) -> &MirOptimizationOptions {
        &self.mir_optimization
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
