//! Real-driver compilation and verified-checkpoint projection.

use skald_compiler::{
    backend::{RuntimeTracePolicy, Target},
    driver::{
        compile_request_to_assembly_observed_inspected, ArtifactKind, ArtifactOptions,
        CompilationEnvironment, CompilationInspectors, CompilationRequest, EntrySelector,
        StandardLibrarySelection,
    },
    identity::CallableId,
    mir::{MirPrimitiveCastKind, MirProgram},
    passes::{
        analyze_local_primitive_common_subexpressions, analyze_redundant_primitive_casts,
        analyze_scalar_spill_provenance, LocalCseObservationCounts, MirPipelineCheckpoint,
        MirPipelineCheckpointLabel, PrimitiveCastObservationCounts, ScalarSpillProvenanceCounts,
        ScalarSpillUnlock,
    },
    reporting::NoopObserver,
};
use std::{collections::BTreeMap, fmt, path::Path, time::Instant};

use crate::{
    aggregate,
    corpus::{ByteString, Corpus, NativeRun, Workload, WorkloadKind},
    digest::sha256_hex,
    model::{
        ArtifactContext, CallableCounts, CandidateCounts, CompilationContext, Configuration,
        CorpusIdentity, EncodedBytes, Example, MeasurementReport, NamedCount, NativeRunContext,
        OperationalContext, OverlapCount, ScheduleOccurrence, SnapshotReport, StdinContext,
        StructureCounts, WorkloadReport,
    },
    revision,
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
            snapshot: snapshot("internal", checkpoint.verified().program(), checkpoint),
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

fn snapshot(
    name: &str,
    program: &MirProgram,
    checkpoint: MirPipelineCheckpoint<'_>,
) -> SnapshotReport {
    let spill = analyze_scalar_spill_provenance(checkpoint.verified());
    let casts = analyze_redundant_primitive_casts(checkpoint.verified());
    let cse = analyze_local_primitive_common_subexpressions(checkpoint.verified());
    let mut scalar_spill = spill_counts(spill.counts());
    let mut redundant_casts = cast_counts(casts.counts());
    let mut local_cse = cse_counts(cse.counts());
    scalar_spill.examples = proven_examples(
        spill
            .callables()
            .iter()
            .map(|observation| (observation.callable(), observation.counts().proven())),
    );
    redundant_casts.examples = proven_examples(
        casts
            .callables()
            .iter()
            .map(|observation| (observation.callable(), observation.counts().proven())),
    );
    local_cse.examples = proven_examples(
        cse.callables()
            .iter()
            .map(|observation| (observation.callable(), observation.counts().proven())),
    );
    let overlaps = spill
        .counts()
        .unlocks()
        .iter()
        .map(|count| OverlapCount {
            enabler: "scalar-spill",
            consumer: overlap_consumer(count.key()).to_owned(),
            sites: count.sites(),
        })
        .collect();
    let callables = callable_counts(program, &spill, &casts, &cse);
    let structure = structure_counts(program);
    let saturated = structure.saturated
        || scalar_spill.saturated
        || redundant_casts.saturated
        || local_cse.saturated;
    SnapshotReport {
        name: name.to_owned(),
        structure,
        scalar_spill,
        redundant_casts,
        local_cse,
        overlaps,
        callables,
        saturated,
    }
}

fn structure_counts(program: &MirProgram) -> StructureCounts {
    let mut definitions = usize_to_u64(program.declarations.len());
    let mut saturated = false;
    for class in program.classes.iter() {
        let class_definitions = class
            .initializers
            .len()
            .saturating_add(class.methods.len())
            .saturating_add(class.static_fields.len())
            .saturating_add(usize::from(class.copy_constructor_declaration.is_some()))
            .saturating_add(usize::from(class.copy_assignment_declaration.is_some()))
            .saturating_add(usize::from(class.destruction.destructor.is_some()));
        aggregate::add(
            &mut definitions,
            usize_to_u64(class_definitions),
            &mut saturated,
        );
    }
    let mut executable_definitions = 0_u64;
    let mut blocks = 0_u64;
    let mut instructions = 0_u64;
    let mut values = 0_u64;
    let mut storages = 0_u64;
    for definition in program.executable_definitions() {
        aggregate::add(&mut executable_definitions, 1, &mut saturated);
        aggregate::add(
            &mut blocks,
            usize_to_u64(definition.body().blocks.len()),
            &mut saturated,
        );
        aggregate::add(
            &mut values,
            usize_to_u64(definition.values().len()),
            &mut saturated,
        );
        aggregate::add(
            &mut storages,
            usize_to_u64(definition.storage_entries().len()),
            &mut saturated,
        );
        for block in &definition.body().blocks {
            aggregate::add(
                &mut instructions,
                usize_to_u64(block.instructions.len()),
                &mut saturated,
            );
        }
    }
    StructureCounts {
        definitions,
        executable_definitions,
        blocks,
        instructions,
        values,
        storages,
        saturated,
    }
}

fn callable_counts(
    program: &MirProgram,
    spill: &skald_compiler::passes::ScalarSpillProvenanceObservation,
    casts: &skald_compiler::passes::PrimitiveCastObservation,
    cse: &skald_compiler::passes::LocalCseObservation,
) -> Vec<CallableCounts> {
    let mut callables = BTreeMap::<CallableId, CallableCounts>::new();
    for observation in spill.callables() {
        let entry = callable_entry(program, &mut callables, observation.callable());
        entry.scalar_spill = spill_counts(observation.counts());
    }
    for observation in casts.callables() {
        let entry = callable_entry(program, &mut callables, observation.callable());
        entry.redundant_casts = cast_counts(observation.counts());
    }
    for observation in cse.callables() {
        let entry = callable_entry(program, &mut callables, observation.callable());
        entry.local_cse = cse_counts(observation.counts());
    }
    for entry in callables.values_mut() {
        entry.saturated = entry.scalar_spill.saturated
            || entry.redundant_casts.saturated
            || entry.local_cse.saturated;
    }
    callables.into_values().collect()
}

fn callable_entry<'a>(
    program: &MirProgram,
    callables: &'a mut BTreeMap<CallableId, CallableCounts>,
    callable: CallableId,
) -> &'a mut CallableCounts {
    callables.entry(callable).or_insert_with(|| CallableCounts {
        identity: callable.to_string(),
        kind: callable_kind(callable).to_owned(),
        label: callable_label(program, callable),
        ..CallableCounts::default()
    })
}

fn callable_kind(callable: CallableId) -> &'static str {
    match callable {
        CallableId::Function(_) => "function",
        CallableId::StaticInitializer(_) => "static-initializer",
        CallableId::Initializer(_) => "initializer",
        CallableId::CopyConstructor(_) => "copy-constructor",
        CallableId::CopyAssignment(_) => "copy-assignment",
        CallableId::Destructor(_) => "destructor",
        CallableId::Method(_) => "method",
    }
}

fn callable_label(program: &MirProgram, callable: CallableId) -> String {
    match callable {
        CallableId::Function(id) => program
            .declarations
            .get(id)
            .map(|declaration| declaration.name.clone()),
        CallableId::Method(id) => program.method(id).map(|method| {
            format!(
                "{}.{}",
                program
                    .class(id.class())
                    .map_or("<unknown-class>", |class| class.name.as_str()),
                method.name
            )
        }),
        CallableId::StaticInitializer(id) => program.static_field(id.field()).map(|field| {
            format!(
                "{}.{}::<static-init>",
                program
                    .class(id.class())
                    .map_or("<unknown-class>", |class| class.name.as_str()),
                field.name
            )
        }),
        CallableId::Initializer(_)
        | CallableId::CopyConstructor(_)
        | CallableId::CopyAssignment(_)
        | CallableId::Destructor(_) => callable.class().and_then(|class| {
            program
                .class(class)
                .map(|class| format!("{}::<{}>", class.name, callable_kind(callable)))
        }),
    }
    .unwrap_or_else(|| callable.to_string())
}

fn spill_counts(counts: &ScalarSpillProvenanceCounts) -> CandidateCounts {
    CandidateCounts {
        inspected: counts.inspected(),
        interesting: counts.interesting(),
        proven: counts.proven(),
        blocked: counts.blocked(),
        non_candidates: counts.non_candidates(),
        affected_callables: counts.affected_callables(),
        supporting_values: counts.supporting_values(),
        supporting_instructions: counts.supporting_instructions(),
        removable_values_upper_bound: counts.removable_values_upper_bound(),
        removable_instructions_upper_bound: counts.removable_instructions_upper_bound(),
        outcomes: named_counts(counts.depths()),
        primary_blockers: named_counts(counts.primary_blockers()),
        barriers: named_counts(counts.barriers()),
        consumers: named_counts(counts.consumers()),
        unlocks: named_counts(counts.unlocks()),
        details: Vec::new(),
        examples: Vec::new(),
        saturated: counts.saturated(),
    }
}

fn cast_counts(counts: &PrimitiveCastObservationCounts) -> CandidateCounts {
    let mut details = counts
        .shapes()
        .iter()
        .map(|count| NamedCount {
            name: format!(
                "{}:{}->{}",
                cast_kind_name(count.key().kind()),
                count.key().source().name(),
                count.key().target().name()
            ),
            sites: count.sites(),
        })
        .collect::<Vec<_>>();
    details.push(NamedCount {
        name: "excluded-checked-conversions".to_owned(),
        sites: counts.excluded_checked_conversions(),
    });
    details.push(NamedCount {
        name: "excluded-checked-range-checks".to_owned(),
        sites: counts.excluded_checked_range_checks(),
    });
    details.sort_by(|left, right| left.name.cmp(&right.name));
    CandidateCounts {
        inspected: counts.inspected(),
        interesting: counts.interesting(),
        proven: counts.proven(),
        blocked: counts.blocked(),
        non_candidates: counts.non_candidates(),
        affected_callables: counts.affected_callables(),
        supporting_values: counts.supporting_values(),
        supporting_instructions: counts.supporting_instructions(),
        removable_values_upper_bound: counts.removable_values_upper_bound(),
        removable_instructions_upper_bound: counts.removable_instructions_upper_bound(),
        outcomes: named_counts(counts.dispositions()),
        primary_blockers: named_counts(counts.primary_blockers()),
        barriers: named_counts(counts.barriers()),
        consumers: named_counts(counts.consumers()),
        unlocks: Vec::new(),
        details,
        examples: Vec::new(),
        saturated: counts.saturated(),
    }
}

fn cse_counts(counts: &LocalCseObservationCounts) -> CandidateCounts {
    let mut details = named_counts(counts.operation_families());
    details.extend(counts.excluded_families().iter().map(|count| NamedCount {
        name: format!("excluded-{}", debug_name(count.key())),
        sites: count.sites(),
    }));
    details.push(NamedCount {
        name: "replaceable-uses".to_owned(),
        sites: counts.replaceable_uses(),
    });
    details.push(NamedCount {
        name: "maximum-repetitions-per-key".to_owned(),
        sites: counts.maximum_repetitions_per_key(),
    });
    details.sort_by(|left, right| left.name.cmp(&right.name));
    CandidateCounts {
        inspected: counts.inspected(),
        interesting: counts.interesting(),
        proven: counts.proven(),
        blocked: counts.blocked(),
        non_candidates: counts.non_candidates(),
        affected_callables: counts.affected_callables(),
        supporting_values: counts.supporting_values(),
        supporting_instructions: counts.supporting_instructions(),
        removable_values_upper_bound: counts.removable_values_upper_bound(),
        removable_instructions_upper_bound: counts.removable_instructions_upper_bound(),
        outcomes: named_counts(counts.outcomes()),
        primary_blockers: named_counts(counts.primary_blockers()),
        barriers: named_counts(counts.barriers()),
        consumers: named_counts(counts.consumers()),
        unlocks: vec![NamedCount {
            name: "scalar-spill-constant-equivalence".to_owned(),
            sites: counts.scalar_spill_unlocks(),
        }],
        details,
        examples: Vec::new(),
        saturated: counts.saturated(),
    }
}

fn named_counts<T: Copy + fmt::Debug>(
    counts: &[skald_compiler::passes::ScalarSpillCount<T>],
) -> Vec<NamedCount> {
    let mut named = counts
        .iter()
        .map(|count| NamedCount {
            name: debug_name(count.key()),
            sites: count.sites(),
        })
        .collect::<Vec<_>>();
    named.sort_by(|left, right| left.name.cmp(&right.name));
    named
}

fn debug_name(value: impl fmt::Debug) -> String {
    let source = format!("{value:?}");
    let mut result = String::with_capacity(source.len());
    for (index, character) in source.chars().enumerate() {
        if character.is_ascii_uppercase() && index > 0 {
            result.push('-');
        }
        result.push(character.to_ascii_lowercase());
    }
    result
}

fn cast_kind_name(kind: MirPrimitiveCastKind) -> &'static str {
    match kind {
        MirPrimitiveCastKind::Identity => "identity",
        MirPrimitiveCastKind::IntegerBits => "integer-bits",
        MirPrimitiveCastKind::ToBool => "to-bool",
        MirPrimitiveCastKind::ToF64 => "to-f64",
        MirPrimitiveCastKind::FromBool => "from-bool",
        MirPrimitiveCastKind::BitReinterpretation => "bit-reinterpretation",
        MirPrimitiveCastKind::CheckedF64ToInteger => "checked-f64-to-integer",
    }
}

fn overlap_consumer(unlock: ScalarSpillUnlock) -> &'static str {
    match unlock {
        ScalarSpillUnlock::CheckedFolding => "checked-folding",
        ScalarSpillUnlock::PrimitiveFolding => "primitive-folding",
        ScalarSpillUnlock::CastSimplification => "cast-simplification",
        ScalarSpillUnlock::BranchFolding => "branch-folding",
        ScalarSpillUnlock::CommonSubexpression => "local-cse",
        ScalarSpillUnlock::DirectSubstitution => "none",
    }
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

fn proven_examples(callables: impl IntoIterator<Item = (CallableId, u64)>) -> Vec<Example> {
    callables
        .into_iter()
        .filter(|(_, proven)| *proven > 0)
        .map(|(callable, _)| Example {
            callable: callable.to_string(),
            classification: "proven".to_owned(),
        })
        .collect()
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn elapsed_u64(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
