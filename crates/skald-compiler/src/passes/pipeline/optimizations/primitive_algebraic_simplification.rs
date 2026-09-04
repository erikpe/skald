//! Exact primitive algebraic simplification with guarded value forwarding.

use crate::mir::{
    rewrite::{
        value_use_sites_for_definition, MirCallableEdit, MirLocalIdentitySite, MirRewriteError,
    },
    BlockId, MirDefinitionRef, MirInstruction, ValueId,
};

use super::{
    super::{
        execution::{
            MirPassData, MirPassFailure, MirPassMeasurement, MirProofPassCapability,
            MirProofPassOutcome,
        },
        policy::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
        MirPassIdentity, MirPassStage,
    },
    primitive_algebra::{PrimitiveAlgebraicFacts, PrimitiveAlgebraicReplacement},
    primitive_evaluation::PrimitiveConstant,
};

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(3);
const NAME: &str = "primitive-algebraic-simplification";
const DESCRIPTION: &str = "Simplifies exact primitive MIR algebraic identities.";
const CONSTANT_RESULTS: &str = "constant-result rewrites";
const FORWARDED_USES: &str = "forwarded value uses";
const REMOVED_ASSIGNMENTS: &str = "removed assignment instructions";
const REMOVED_VALUES: &str = "removed value declarations";
const PROTECTED_REJECTIONS: &str = "rejected protected-use candidates";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, MirPassStage::ProofRich, NAME, DESCRIPTION),
    MirPassImplementation::proof_rich(IDENTITY, transform),
);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SimplificationCounts {
    constant_results: usize,
    forwarded_uses: usize,
    removed_assignments: usize,
    removed_values: usize,
    protected_rejections: usize,
}

impl SimplificationCounts {
    const fn has_changes(self) -> bool {
        self.constant_results != 0 || self.removed_assignments != 0
    }

    fn add(&mut self, other: Self) {
        self.constant_results = self.constant_results.saturating_add(other.constant_results);
        self.forwarded_uses = self.forwarded_uses.saturating_add(other.forwarded_uses);
        self.removed_assignments = self
            .removed_assignments
            .saturating_add(other.removed_assignments);
        self.removed_values = self.removed_values.saturating_add(other.removed_values);
        self.protected_rejections = self
            .protected_rejections
            .saturating_add(other.protected_rejections);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CandidateReplacement {
    Constant(PrimitiveConstant),
    Forward(ValueId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Candidate {
    block: BlockId,
    result: ValueId,
    replacement: CandidateReplacement,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ScanResult {
    candidate: Option<Candidate>,
    protected_rejections: usize,
}

fn transform(capability: MirProofPassCapability) -> Result<MirProofPassOutcome, MirPassFailure> {
    let mut processed_callables = 0;
    let mut protected_rejections = 0usize;
    let mut has_candidate = false;

    for definition in capability.verified().program().executable_definitions() {
        processed_callables += 1;
        let scan = scan_definition(definition).map_err(MirPassFailure::Rewrite)?;
        if scan.candidate.is_some() {
            has_candidate = true;
            break;
        }
        protected_rejections = protected_rejections.saturating_add(scan.protected_rejections);
    }

    if !has_candidate {
        let counts = SimplificationCounts {
            protected_rejections,
            ..SimplificationCounts::default()
        };
        return capability.unchanged_with(pass_data(processed_callables, 0, counts));
    }

    let mut changed_callables = 0;
    let mut counts = SimplificationCounts::default();
    let rewritten = capability.rewrite(|_, edit| {
        let callable_counts = simplify_callable(edit)?;
        if callable_counts.has_changes() {
            changed_callables += 1;
        }
        counts.add(callable_counts);
        Ok(())
    })?;

    rewritten.finish(pass_data(0, changed_callables, counts))
}

fn scan_definition(definition: MirDefinitionRef<'_>) -> Result<ScanResult, MirRewriteError> {
    let mut facts = PrimitiveAlgebraicFacts::default();
    let mut protected_rejections = 0usize;

    for block in &definition.body().blocks {
        if let Some(candidate) = scan_block(
            block.id,
            &block.instructions,
            &mut facts,
            &mut protected_rejections,
            |result, source, site| {
                forwarding_is_safe_in_definition(definition, result, source, site)
            },
        )? {
            return Ok(ScanResult {
                candidate: Some(candidate),
                protected_rejections,
            });
        }
    }

    Ok(ScanResult {
        candidate: None,
        protected_rejections,
    })
}

fn forwarding_is_safe_in_definition(
    definition: MirDefinitionRef<'_>,
    result: ValueId,
    source: ValueId,
    result_site: MirLocalIdentitySite,
) -> Result<bool, MirRewriteError> {
    let Some(result_value) = definition.values().get(result.index()) else {
        return Ok(false);
    };
    let Some(source_value) = definition.values().get(source.index()) else {
        return Ok(false);
    };
    if result_value.ty != source_value.ty {
        return Ok(false);
    }

    let source_sites = value_use_sites_for_definition(definition, source)?;
    let result_sites = value_use_sites_for_definition(definition, result)?;
    Ok(definition_precedes(source_sites.definition(), result_site)
        && result_sites.is_forwarding_safe())
}

fn simplify_callable(edit: &mut MirCallableEdit) -> Result<SimplificationCounts, MirRewriteError> {
    let mut counts = SimplificationCounts::default();

    loop {
        let scan = scan_edit(edit)?;
        let Some(candidate) = scan.candidate else {
            counts.protected_rejections = counts
                .protected_rejections
                .saturating_add(scan.protected_rejections);
            break;
        };

        match candidate.replacement {
            CandidateReplacement::Constant(constant) => {
                rewrite_constant_result(edit, candidate.block, candidate.result, constant)?;
                counts.constant_results = counts.constant_results.saturating_add(1);
            }
            CandidateReplacement::Forward(source) => {
                let forwarded = edit.replace_value_uses(candidate.result, source)?;
                remove_assignment(edit, candidate.block, candidate.result)?;
                edit.remove_value(candidate.result)?;
                counts.forwarded_uses = counts.forwarded_uses.saturating_add(forwarded);
                counts.removed_assignments = counts.removed_assignments.saturating_add(1);
                counts.removed_values = counts.removed_values.saturating_add(1);
            }
        }
    }

    Ok(counts)
}

fn scan_edit(edit: &MirCallableEdit) -> Result<ScanResult, MirRewriteError> {
    let mut facts = PrimitiveAlgebraicFacts::default();
    let mut protected_rejections = 0usize;

    for &block in edit.block_order() {
        if let Some(candidate) = scan_block(
            block,
            &edit.block(block)?.instructions,
            &mut facts,
            &mut protected_rejections,
            |result, source, site| forwarding_is_safe_in_edit(edit, result, source, site),
        )? {
            return Ok(ScanResult {
                candidate: Some(candidate),
                protected_rejections,
            });
        }
    }

    Ok(ScanResult {
        candidate: None,
        protected_rejections,
    })
}

fn scan_block(
    block: BlockId,
    instructions: &[MirInstruction],
    facts: &mut PrimitiveAlgebraicFacts,
    protected_rejections: &mut usize,
    mut forwarding_is_safe: impl FnMut(
        ValueId,
        ValueId,
        MirLocalIdentitySite,
    ) -> Result<bool, MirRewriteError>,
) -> Result<Option<Candidate>, MirRewriteError> {
    facts.begin_block();
    for (instruction_index, instruction) in instructions.iter().enumerate() {
        let MirInstruction::Assign(assignment) = instruction else {
            continue;
        };
        let replacement = facts.replacement(&assignment.rvalue.kind, assignment.rvalue.ty);
        if let Some(replacement) = replacement {
            let site = MirLocalIdentitySite::Instruction {
                block: block.index(),
                instruction: instruction_index,
            };
            match replacement {
                PrimitiveAlgebraicReplacement::Constant(constant) => {
                    return Ok(Some(Candidate {
                        block,
                        result: assignment.result,
                        replacement: CandidateReplacement::Constant(constant),
                    }));
                }
                PrimitiveAlgebraicReplacement::Forward(source) => {
                    if forwarding_is_safe(assignment.result, source, site)? {
                        return Ok(Some(Candidate {
                            block,
                            result: assignment.result,
                            replacement: CandidateReplacement::Forward(source),
                        }));
                    }
                    *protected_rejections = protected_rejections.saturating_add(1);
                }
            }
        }
        facts.observe_assignment(assignment);
    }
    Ok(None)
}

fn forwarding_is_safe_in_edit(
    edit: &MirCallableEdit,
    result: ValueId,
    source: ValueId,
    result_site: MirLocalIdentitySite,
) -> Result<bool, MirRewriteError> {
    if edit.value(result)?.ty != edit.value(source)?.ty {
        return Ok(false);
    }
    let source_sites = edit.value_use_sites(source)?;
    let result_sites = edit.value_use_sites(result)?;
    Ok(definition_precedes(source_sites.definition(), result_site)
        && result_sites.is_forwarding_safe())
}

const fn definition_precedes(source: MirLocalIdentitySite, result: MirLocalIdentitySite) -> bool {
    match (source, result) {
        (
            MirLocalIdentitySite::Instruction {
                block: source_block,
                instruction: source_instruction,
            },
            MirLocalIdentitySite::Instruction {
                block: result_block,
                instruction: result_instruction,
            },
        ) => source_block == result_block && source_instruction < result_instruction,
        _ => false,
    }
}

fn rewrite_constant_result(
    edit: &mut MirCallableEdit,
    block: BlockId,
    result: ValueId,
    constant: PrimitiveConstant,
) -> Result<(), MirRewriteError> {
    edit.rewrite_block_instructions(block, |instructions| {
        instructions
            .iter()
            .cloned()
            .map(|mut instruction| {
                if let MirInstruction::Assign(assignment) = &mut instruction {
                    if assignment.result == result {
                        debug_assert_eq!(constant.ty(), assignment.rvalue.ty);
                        assignment.rvalue.kind = constant.into_rvalue_kind();
                    }
                }
                instruction
            })
            .collect()
    })
}

fn remove_assignment(
    edit: &mut MirCallableEdit,
    block: BlockId,
    result: ValueId,
) -> Result<(), MirRewriteError> {
    edit.rewrite_block_instructions(block, |instructions| {
        instructions
            .iter()
            .filter(|instruction| {
                !matches!(instruction, MirInstruction::Assign(assignment) if assignment.result == result)
            })
            .cloned()
            .collect()
    })
}

fn pass_data(
    processed_callables: usize,
    changed_callables: usize,
    counts: SimplificationCounts,
) -> MirPassData {
    let data = if changed_callables == 0 {
        MirPassData::processed(processed_callables)
    } else {
        MirPassData::changed(changed_callables)
    };
    data.with_measurement(MirPassMeasurement::count(
        CONSTANT_RESULTS,
        count(counts.constant_results),
    ))
    .with_measurement(MirPassMeasurement::count(
        FORWARDED_USES,
        count(counts.forwarded_uses),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_ASSIGNMENTS,
        count(counts.removed_assignments),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_VALUES,
        count(counts.removed_values),
    ))
    .with_measurement(MirPassMeasurement::count(
        PROTECTED_REJECTIONS,
        count(counts.protected_rejections),
    ))
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "primitive_algebraic_simplification/tests.rs"]
mod tests;
