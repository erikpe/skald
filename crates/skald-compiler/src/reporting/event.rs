//! Typed identities and owned report events.

use std::{path::PathBuf, time::Duration};

use super::metrics::ReportMetric;

/// The amount of operational observation requested by a consumer.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReportDetail {
    Off,
    Phases,
    Details,
    Trace,
}

impl ReportDetail {
    pub(super) const fn includes(self, requested: Self) -> bool {
        !matches!(self, Self::Off)
            && !matches!(requested, Self::Off)
            && self as u8 >= requested as u8
    }
}

/// A stable operational boundary in compiler or driver orchestration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportPhase {
    ProviderNormalization,
    ModuleLoading,
    Lexing,
    Parsing,
    Resolution,
    TypeChecking,
    PreliminaryMirLowering,
    PreliminaryMirVerification,
    StaticLifecyclePlanning,
    PlannedMirVerification,
    StaticLifecycleSynthesis,
    MirPipeline,
    BackendEmission,
    HostLinking,
    ArtifactPublication,
}

impl ReportPhase {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::ProviderNormalization => "provider normalization",
            Self::ModuleLoading => "module loading",
            Self::Lexing => "lexing",
            Self::Parsing => "parsing",
            Self::Resolution => "resolution",
            Self::TypeChecking => "type checking",
            Self::PreliminaryMirLowering => "preliminary MIR lowering",
            Self::PreliminaryMirVerification => "preliminary MIR verification",
            Self::StaticLifecyclePlanning => "static lifecycle planning",
            Self::PlannedMirVerification => "planned MIR verification",
            Self::StaticLifecycleSynthesis => "static lifecycle synthesis",
            Self::MirPipeline => "MIR pipeline",
            Self::BackendEmission => "backend emission",
            Self::HostLinking => "host linking",
            Self::ArtifactPublication => "artifact publication",
        }
    }
}

/// Whether an observed operation completed or failed at its existing boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportOutcome {
    Completed,
    Failed,
}

impl ReportOutcome {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

/// Which of the loader's two real parser executions produced an observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportModuleStage {
    Discovery,
    Final,
}

impl ReportModuleStage {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Discovery => "discovery",
            Self::Final => "final",
        }
    }
}

/// The extent covered by an aggregate completion event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportScope {
    Compilation,
    Driver,
}

impl ReportScope {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Compilation => "compilation",
            Self::Driver => "driver",
        }
    }
}

/// An artifact category independent of driver request parsing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportArtifactKind {
    Assembly,
    Executable,
    Dump,
}

impl ReportArtifactKind {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Assembly => "assembly",
            Self::Executable => "executable",
            Self::Dump => "dump",
        }
    }
}

/// One owned operational observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReportEvent {
    PhaseStarted {
        phase: ReportPhase,
    },
    PhaseFinished {
        phase: ReportPhase,
        elapsed: Duration,
        outcome: ReportOutcome,
        metrics: Vec<ReportMetric>,
    },
    ModuleParsed {
        module: String,
        stage: ReportModuleStage,
        tokens: u64,
        outcome: ReportOutcome,
    },
    ArtifactPublished {
        kind: ReportArtifactKind,
        path: PathBuf,
    },
    RunFinished {
        scope: ReportScope,
        elapsed: Duration,
        outcome: ReportOutcome,
    },
}

impl ReportEvent {
    pub(super) const fn detail(&self) -> ReportDetail {
        match self {
            Self::ModuleParsed { .. } => ReportDetail::Trace,
            Self::PhaseStarted { .. }
            | Self::PhaseFinished { .. }
            | Self::ArtifactPublished { .. }
            | Self::RunFinished { .. } => ReportDetail::Phases,
        }
    }
}
