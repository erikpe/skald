//! Real-driver compilation and semantic checkpoint selection.

use skald_compiler::{
    backend::{RuntimeTracePolicy, Target},
    driver::{
        compile_request_to_assembly_observed_inspected, ArtifactKind, ArtifactOptions,
        CompilationEnvironment, CompilationInspectors, CompilationRequest, EntrySelector,
        StandardLibrarySelection,
    },
    passes::{MirPipelineCheckpoint, MirPipelineCheckpointLabel},
    reporting::NoopObserver,
};
use std::{fmt, path::Path, time::Instant};

use crate::{
    aggregate,
    corpus::{ByteString, Corpus, NativeRun, Workload, WorkloadKind},
    digest::sha256_hex,
    model::{
        ArtifactContext, CompilationContext, Configuration, CorpusIdentity, EncodedBytes,
        MeasurementReport, NativeRunContext, OperationalContext, ScheduleOccurrence,
        SnapshotReport, StdinContext, WorkloadReport,
    },
    projection, revision,
};

const REACHABILITY_PASS: &str = "whole-world-reachability";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MeasurementOptions {
    operational: bool,
}

impl MeasurementOptions {
    pub const fn with_operational_context(mut self, enabled: bool) -> Self {
        self.operational = enabled;
        self
    }
}

#[derive(Debug)]
pub struct MeasurementError {
    workload: Option<String>,
    message: String,
}

impl MeasurementError {
    fn general(message: impl Into<String>) -> Self {
        Self {
            workload: None,
            message: message.into(),
        }
    }

    fn workload(workload: &Workload, message: impl Into<String>) -> Self {
        Self {
            workload: Some(workload.id.clone()),
            message: message.into(),
        }
    }
}

impl fmt::Display for MeasurementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(workload) = &self.workload {
            write!(formatter, "workload {workload:?}: {}", self.message)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl std::error::Error for MeasurementError {}

pub fn measure_corpus(
    repository_root: impl AsRef<Path>,
    corpus: &Corpus,
    options: MeasurementOptions,
) -> Result<MeasurementReport, MeasurementError> {
    let repository_root = std::fs::canonicalize(repository_root.as_ref()).map_err(|error| {
        MeasurementError::general(format!("could not canonicalize repository root: {error}"))
    })?;
    let compiler = revision::inspect(&repository_root);
    let mut schedule = None;
    let mut workloads = Vec::with_capacity(corpus.workloads().len());
    for workload in corpus.workloads() {
        let measured = measure_workload(&repository_root, workload, options)?;
        match &schedule {
            Some(expected) if expected != &measured.schedule => {
                return Err(MeasurementError::workload(
                    workload,
                    "resolved MIR schedule differs from earlier workloads",
                ));
            }
            None => schedule = Some(measured.schedule.clone()),
            Some(_) => {}
        }
        workloads.push(measured.report);
    }
    let totals = aggregate::totals(&workloads);
    Ok(MeasurementReport {
        schema: 1,
        corpus: CorpusIdentity {
            name: corpus.name().to_owned(),
            version: corpus.version(),
        },
        compiler,
        configuration: Configuration {
            target: Target::X86_64SysV.name(),
            runtime_trace: "omitted",
            mir_profile: "default",
            mir_exclusions: Vec::new(),
        },
        schedule: schedule.unwrap_or_default(),
        workloads,
        totals,
    })
}

struct MeasuredWorkload {
    schedule: Vec<ScheduleOccurrence>,
    report: WorkloadReport,
}

fn measure_workload(
    repository_root: &Path,
    workload: &Workload,
    options: MeasurementOptions,
) -> Result<MeasuredWorkload, MeasurementError> {
    let request = CompilationRequest::new(
        EntrySelector::File(workload.entry.clone()),
        Vec::new(),
        StandardLibrarySelection::Default,
        Target::X86_64SysV,
        ArtifactOptions::new(ArtifactKind::Assembly, None)
            .with_runtime_trace_policy(RuntimeTracePolicy::Omitted),
        CompilationEnvironment::new(repository_root.to_owned(), repository_root.join("std")),
    );
    let mut samples = Vec::new();
    let mut inspector = |checkpoint: MirPipelineCheckpoint<'_>| {
        samples.push(CheckpointSample {
            label: checkpoint.label(),
            snapshot: projection::snapshot("internal", checkpoint.verified().program(), checkpoint),
        });
    };
    let started = Instant::now();
    let mut observer = NoopObserver;
    let artifact = compile_request_to_assembly_observed_inspected(
        &request,
        &mut observer,
        CompilationInspectors::new().with_mir_pipeline(&mut inspector),
    )
    .map_err(|error| {
        MeasurementError::workload(workload, format!("compilation failed: {error:?}"))
    })?;
    let elapsed = elapsed_u64(started.elapsed().as_nanos());
    let (schedule, snapshots) = select_snapshots(workload, samples)?;
    let native_runs = workload.native_runs.iter().map(native_context).collect();
    let kind = match &workload.kind {
        WorkloadKind::Golden { .. } => "golden",
        WorkloadKind::Explicit => "explicit",
    };
    let golden_build = match &workload.kind {
        WorkloadKind::Golden { build } => Some(build.clone()),
        WorkloadKind::Explicit => None,
    };
    let operational = options.operational.then_some(OperationalContext {
        compile_nanoseconds: elapsed,
        native_nanoseconds: Vec::new(),
    });
    let report = WorkloadReport {
        id: workload.id.clone(),
        category: workload.category.clone(),
        compilation: CompilationContext {
            kind,
            identity: workload.identity.clone(),
            entry: workload.entry_relative.clone(),
            provider_roots: Vec::new(),
            standard_library: "repository",
            compiler_arguments: Vec::new(),
            golden_build,
            artifacts: ArtifactContext {
                assembly_bytes: usize_to_u64(artifact.assembly.len()),
                executable_bytes: None,
            },
        },
        native_runs,
        snapshots,
        operational,
    };
    Ok(MeasuredWorkload { schedule, report })
}

#[derive(Clone)]
struct CheckpointSample {
    label: MirPipelineCheckpointLabel,
    snapshot: SnapshotReport,
}

fn select_snapshots(
    workload: &Workload,
    samples: Vec<CheckpointSample>,
) -> Result<(Vec<ScheduleOccurrence>, Vec<SnapshotReport>), MeasurementError> {
    let schedule = samples
        .iter()
        .filter_map(|sample| match sample.label {
            MirPipelineCheckpointLabel::After {
                position,
                pass_name,
                occurrence,
            } => Some(ScheduleOccurrence {
                position,
                pass: pass_name.to_owned(),
                occurrence,
            }),
            MirPipelineCheckpointLabel::Input | MirPipelineCheckpointLabel::Final => None,
        })
        .collect::<Vec<_>>();
    let reachability = schedule
        .iter()
        .enumerate()
        .filter(|(_, occurrence)| occurrence.pass == REACHABILITY_PASS)
        .collect::<Vec<_>>();
    if reachability.len() != 1 {
        return Err(MeasurementError::workload(
            workload,
            format!("default schedule must contain exactly one {REACHABILITY_PASS:?} occurrence"),
        ));
    }
    let (reachability_index, _) = reachability[0];
    if reachability_index + 1 != schedule.len() {
        return Err(MeasurementError::workload(
            workload,
            format!("{REACHABILITY_PASS:?} must be the last default-schedule pass"),
        ));
    }
    let input = samples
        .iter()
        .find(|sample| sample.label == MirPipelineCheckpointLabel::Input)
        .ok_or_else(|| MeasurementError::workload(workload, "missing input MIR checkpoint"))?;
    let reachability_sample_index = samples
        .iter()
        .position(|sample| {
            matches!(sample.label, MirPipelineCheckpointLabel::After { pass_name, .. } if pass_name == REACHABILITY_PASS)
        })
        .expect("resolved reachability occurrence must have a checkpoint");
    let pre_reachability = reachability_sample_index
        .checked_sub(1)
        .and_then(|index| samples.get(index))
        .ok_or_else(|| {
            MeasurementError::workload(workload, "missing checkpoint before reachability")
        })?;
    let final_sample = samples
        .iter()
        .find(|sample| sample.label == MirPipelineCheckpointLabel::Final)
        .ok_or_else(|| MeasurementError::workload(workload, "missing final MIR checkpoint"))?;
    let mut input = input.snapshot.clone();
    input.name = "input".to_owned();
    let mut pre = pre_reachability.snapshot.clone();
    pre.name = "pre-reachability".to_owned();
    let mut final_snapshot = final_sample.snapshot.clone();
    final_snapshot.name = "final".to_owned();
    Ok((schedule, vec![input, pre, final_snapshot]))
}

fn native_context(run: &NativeRun) -> NativeRunContext {
    NativeRunContext {
        identity: run.identity.clone(),
        arguments: run
            .arguments
            .iter()
            .map(|argument| match argument {
                ByteString::Utf8(value) => EncodedBytes::Utf8(value.clone()),
                ByteString::Hex(value) => EncodedBytes::Hex(value.clone()),
            })
            .collect(),
        stdin: StdinContext {
            origin: if run.stdin.bytes.is_empty() && run.stdin.origin == "inline" {
                "none"
            } else {
                run.stdin.origin
            },
            path: run.stdin.path.clone(),
            byte_count: usize_to_u64(run.stdin.bytes.len()),
            sha256: sha256_hex(&run.stdin.bytes),
        },
    }
}

fn elapsed_u64(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
