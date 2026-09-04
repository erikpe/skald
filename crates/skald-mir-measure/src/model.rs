//! Canonical machine report shared by JSON and human renderers.

use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReportFormat {
    #[default]
    Human,
    Json,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MeasurementReport {
    pub(crate) schema: u64,
    pub(crate) corpus: CorpusIdentity,
    pub(crate) compiler: CompilerIdentity,
    pub(crate) configuration: Configuration,
    pub(crate) schedule: Vec<ScheduleOccurrence>,
    pub(crate) workloads: Vec<WorkloadReport>,
    pub(crate) totals: Totals,
}

impl MeasurementReport {
    pub fn workloads(&self) -> &[WorkloadReport] {
        &self.workloads
    }

    pub fn totals(&self) -> &Totals {
        &self.totals
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct CorpusIdentity {
    pub(crate) name: String,
    pub(crate) version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct CompilerIdentity {
    pub(crate) revision: String,
    pub(crate) dirty: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct Configuration {
    pub(crate) target: &'static str,
    pub(crate) runtime_trace: &'static str,
    pub(crate) mir_profile: &'static str,
    pub(crate) mir_exclusions: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct ScheduleOccurrence {
    pub(crate) position: usize,
    pub(crate) pass: String,
    pub(crate) occurrence: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkloadReport {
    pub(crate) id: String,
    pub(crate) category: String,
    pub(crate) compilation: CompilationContext,
    pub(crate) native_runs: Vec<NativeRunContext>,
    pub(crate) snapshots: Vec<SnapshotReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) operational: Option<OperationalContext>,
}

impl WorkloadReport {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn snapshots(&self) -> &[SnapshotReport] {
        &self.snapshots
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct CompilationContext {
    pub(crate) kind: &'static str,
    pub(crate) identity: String,
    pub(crate) entry: String,
    pub(crate) provider_roots: Vec<String>,
    pub(crate) standard_library: &'static str,
    pub(crate) compiler_arguments: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) golden_build: Option<String>,
    pub(crate) artifacts: ArtifactContext,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct ArtifactContext {
    pub(crate) assembly_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) executable_bytes: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct NativeRunContext {
    pub(crate) identity: String,
    pub(crate) arguments: Vec<EncodedBytes>,
    pub(crate) stdin: StdinContext,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "encoding", content = "value", rename_all = "kebab-case")]
pub(crate) enum EncodedBytes {
    Utf8(String),
    Hex(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct StdinContext {
    pub(crate) origin: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
    pub(crate) byte_count: u64,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct OperationalContext {
    pub(crate) compile_nanoseconds: u64,
    pub(crate) native_nanoseconds: Vec<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct SnapshotReport {
    pub(crate) name: String,
    pub(crate) structure: StructureCounts,
    pub(crate) scalar_spill: CandidateCounts,
    pub(crate) redundant_casts: CandidateCounts,
    pub(crate) local_cse: CandidateCounts,
    pub(crate) overlaps: Vec<OverlapCount>,
    pub(crate) callables: Vec<CallableCounts>,
    pub(crate) saturated: bool,
}

impl SnapshotReport {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn scalar_spill(&self) -> &CandidateCounts {
        &self.scalar_spill
    }

    pub const fn redundant_casts(&self) -> &CandidateCounts {
        &self.redundant_casts
    }

    pub const fn local_cse(&self) -> &CandidateCounts {
        &self.local_cse
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub(crate) struct StructureCounts {
    pub(crate) definitions: u64,
    pub(crate) executable_definitions: u64,
    pub(crate) blocks: u64,
    pub(crate) instructions: u64,
    pub(crate) values: u64,
    pub(crate) storages: u64,
    pub(crate) saturated: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CandidateCounts {
    pub(crate) inspected: u64,
    pub(crate) interesting: u64,
    pub(crate) proven: u64,
    pub(crate) blocked: u64,
    pub(crate) non_candidates: u64,
    pub(crate) affected_callables: u64,
    pub(crate) supporting_values: u64,
    pub(crate) supporting_instructions: u64,
    pub(crate) removable_values_upper_bound: u64,
    pub(crate) removable_instructions_upper_bound: u64,
    pub(crate) outcomes: Vec<NamedCount>,
    pub(crate) primary_blockers: Vec<NamedCount>,
    pub(crate) barriers: Vec<NamedCount>,
    pub(crate) consumers: Vec<NamedCount>,
    pub(crate) unlocks: Vec<NamedCount>,
    pub(crate) details: Vec<NamedCount>,
    pub(crate) examples: Vec<Example>,
    pub(crate) saturated: bool,
}

impl CandidateCounts {
    pub const fn proven(&self) -> u64 {
        self.proven
    }

    pub const fn saturated(&self) -> bool {
        self.saturated
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct NamedCount {
    pub(crate) name: String,
    pub(crate) sites: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct OverlapCount {
    pub(crate) enabler: &'static str,
    pub(crate) consumer: String,
    pub(crate) sites: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct Example {
    pub(crate) callable: String,
    pub(crate) classification: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub(crate) struct CallableCounts {
    pub(crate) identity: String,
    pub(crate) kind: String,
    pub(crate) label: String,
    pub(crate) scalar_spill: CandidateCounts,
    pub(crate) redundant_casts: CandidateCounts,
    pub(crate) local_cse: CandidateCounts,
    pub(crate) saturated: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct Totals {
    pub(crate) snapshots: Vec<SnapshotReport>,
    pub(crate) workload_categories: Vec<CategoryCoverage>,
    pub(crate) saturated: bool,
}

impl Totals {
    pub fn snapshots(&self) -> &[SnapshotReport] {
        &self.snapshots
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct CategoryCoverage {
    pub(crate) category: String,
    pub(crate) workloads_with_proven_candidates: Vec<String>,
}
