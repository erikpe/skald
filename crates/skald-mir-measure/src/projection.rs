//! Projection of verified compiler observations into the stable report schema.

use skald_compiler::{
    identity::CallableId,
    mir::{MirPrimitiveCastKind, MirProgram},
    passes::{
        analyze_local_primitive_common_subexpressions, analyze_redundant_primitive_casts,
        analyze_scalar_spill_provenance, LocalCseObservationCounts, MirPipelineCheckpoint,
        PrimitiveCastObservationCounts, ScalarSpillProvenanceCounts, ScalarSpillUnlock,
    },
};
use std::{collections::BTreeMap, fmt};

use crate::{
    aggregate,
    model::{
        CallableCounts, CandidateCounts, Example, NamedCount, OverlapCount, SnapshotReport,
        StructureCounts,
    },
};

pub(super) fn snapshot(
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
