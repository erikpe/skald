//! Read-only census of redundant ordinary primitive casts.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    identity::CallableId,
    mir::{
        rewrite::{
            value_use_sites_for_definition, MirLocalIdentitySite, MirRewriteError, MirValueUseRole,
        },
        BlockId, MirDefinitionRef, MirInstruction, MirPrimitiveCast, MirPrimitiveCastKind,
        MirPrimitiveType, MirRvalueKind, MirTerminator, ValueId,
    },
    passes::VerifiedFinalMirProgram,
};

use super::cast_model::{
    PrimitiveCastBlocker, PrimitiveCastCallableObservation, PrimitiveCastConsumer,
    PrimitiveCastCount, PrimitiveCastDisposition, PrimitiveCastObservation,
    PrimitiveCastObservationCounts, PrimitiveCastShape,
};
use super::site::{merge_examples, RedundancySiteClassification, RedundancySiteExample};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Composition {
    OriginalInput,
    DirectCast,
    MissingValueDomain,
    CheckedFailure,
    FloatingPayload,
    Unsupported,
}

#[derive(Clone, Copy)]
struct CastSite {
    callable: CallableId,
    block: usize,
    block_id: BlockId,
    instruction: usize,
    result: ValueId,
    operation: MirPrimitiveCast,
    operand: ValueId,
}

/// Measures ordinary primitive-cast redundancy without cloning or mutating
/// the verified final-MIR product.
pub fn analyze_redundant_primitive_casts(
    verified: &VerifiedFinalMirProgram,
) -> PrimitiveCastObservation {
    let mut total = Accumulator::default();
    let mut callables = Vec::new();
    for definition in verified.program().executable_definitions() {
        let callable = definition.callable();
        let observed = analyze_definition(definition)
            .expect("verified final MIR must have coherent callable-local identities");
        if observed.has_observations() {
            total.merge(&observed);
            let affected = u64::from(observed.counts.interesting != 0);
            let examples = observed.examples.clone();
            callables.push(PrimitiveCastCallableObservation::new(
                callable,
                observed.finish(affected),
                examples,
            ));
        }
    }
    let affected_callables = callables
        .iter()
        .filter(|observation| observation.counts().interesting() != 0)
        .count() as u64;
    let examples = total.examples.clone();
    PrimitiveCastObservation::new(total.finish(affected_callables), callables, examples)
}

fn analyze_definition(definition: MirDefinitionRef<'_>) -> Result<Accumulator, MirRewriteError> {
    let casts = cast_sites(definition);
    let by_result = casts
        .iter()
        .map(|site| (site.result, *site))
        .collect::<BTreeMap<_, _>>();
    let mut observed = Accumulator::default();

    for block in definition.body().blocks.iter() {
        if matches!(
            block.terminator,
            Some(MirTerminator::PrimitiveCastRangeCheck { .. })
        ) {
            observed.increment_checked_range_check();
        }
        for instruction in &block.instructions {
            if matches!(
                instruction,
                MirInstruction::Assign(assignment)
                    if matches!(assignment.rvalue.kind, MirRvalueKind::CheckedF64ToInteger { .. })
            ) {
                observed.increment_checked_conversion();
            }
        }
    }

    for site in casts {
        observed.increment_inspected();
        observed.increment_shape(PrimitiveCastShape::new(
            site.operation.kind(),
            site.operation.source,
            site.operation.target,
        ));
        let mut barriers = validation_barriers(definition, site);
        let uses = match value_use_sites_for_definition(definition, site.result) {
            Ok(uses) => uses,
            Err(_) if barriers.contains(&PrimitiveCastBlocker::MalformedIdentity) => {
                observed.increment_consumer(PrimitiveCastConsumer::Other);
                if site.operation.kind() == MirPrimitiveCastKind::Identity {
                    observed.increment_disposition(PrimitiveCastDisposition::Identity);
                    observed.record_interesting(site, None, barriers, true);
                } else {
                    observed.increment_disposition(disposition(site.operation));
                    observed.increment_barrier(PrimitiveCastBlocker::MalformedIdentity);
                    observed.increment_non_candidate();
                }
                continue;
            }
            Err(error) => return Err(error),
        };
        if uses.uses().is_empty() {
            observed.increment_consumer(PrimitiveCastConsumer::Dead);
        } else {
            for use_site in uses.uses() {
                observed.increment_consumer(consumer(use_site.role()));
            }
        }

        let is_identity = site.operation.kind() == MirPrimitiveCastKind::Identity;
        let predecessor = by_result.get(&site.operand).copied();

        if is_identity {
            observed.increment_disposition(PrimitiveCastDisposition::Identity);
            add_replacement_barriers(&mut barriers, &uses, site.block);
            observed.record_interesting(site, predecessor, barriers, true);
            continue;
        }

        let Some(first) = predecessor else {
            observed.increment_disposition(disposition(site.operation));
            observed.increment_non_candidate();
            continue;
        };
        if first.block != site.block {
            observed.increment_disposition(disposition(site.operation));
            barriers.insert(PrimitiveCastBlocker::ControlFlowBoundary);
            observed.increment_barrier(PrimitiveCastBlocker::ControlFlowBoundary);
            observed.increment_non_candidate();
            continue;
        }
        if first.instruction + 1 != site.instruction {
            observed.increment_disposition(disposition(site.operation));
            barriers.insert(PrimitiveCastBlocker::NonAdjacentProvenance);
            observed.increment_barrier(PrimitiveCastBlocker::NonAdjacentProvenance);
            observed.increment_non_candidate();
            continue;
        }
        if first.operation.kind() == MirPrimitiveCastKind::Identity {
            observed.increment_disposition(disposition(site.operation));
            // The earlier identity is already the unique attributable site.
            observed.increment_non_candidate();
            continue;
        }

        match compose(first.operation, site.operation) {
            Composition::OriginalInput => {
                observed.increment_disposition(PrimitiveCastDisposition::RemovableChain);
                add_replacement_barriers(&mut barriers, &uses, site.block);
                observed.record_interesting(site, Some(first), barriers, true);
            }
            Composition::DirectCast => {
                observed.increment_disposition(PrimitiveCastDisposition::RemovableChain);
                let first_uses = value_use_sites_for_definition(definition, first.result)?;
                if first_uses.uses().len() != 1 {
                    barriers.insert(PrimitiveCastBlocker::MultipleUses);
                }
                observed.record_interesting(site, Some(first), barriers, true);
            }
            Composition::MissingValueDomain => {
                observed.increment_disposition(disposition(site.operation));
                barriers.insert(PrimitiveCastBlocker::MissingValueDomainFact);
                observed.record_interesting(site, Some(first), barriers, false);
            }
            Composition::CheckedFailure => {
                observed.increment_disposition(PrimitiveCastDisposition::CheckedFloatingToInteger);
                barriers.insert(PrimitiveCastBlocker::CheckedFailure);
                observed.record_interesting(site, Some(first), barriers, false);
            }
            Composition::FloatingPayload => {
                observed.increment_disposition(disposition(site.operation));
                barriers.insert(PrimitiveCastBlocker::FloatingPayload);
                observed.record_interesting(site, Some(first), barriers, false);
            }
            Composition::Unsupported => {
                observed.increment_disposition(PrimitiveCastDisposition::Unsupported);
                barriers.insert(PrimitiveCastBlocker::UnsupportedComposition);
                observed.record_interesting(site, Some(first), barriers, false);
            }
        }
    }
    Ok(observed)
}

#[cfg(test)]
pub(super) fn analyze_unverified_definition(
    definition: MirDefinitionRef<'_>,
) -> Result<PrimitiveCastObservationCounts, MirRewriteError> {
    analyze_definition(definition).map(|observed| observed.finish(1))
}

fn cast_sites(definition: MirDefinitionRef<'_>) -> Vec<CastSite> {
    definition
        .body()
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(block, body)| {
            body.instructions
                .iter()
                .enumerate()
                .filter_map(move |(instruction, item)| {
                    let MirInstruction::Assign(assignment) = item else {
                        return None;
                    };
                    let MirRvalueKind::PrimitiveCast { operation, operand } =
                        assignment.rvalue.kind
                    else {
                        return None;
                    };
                    Some(CastSite {
                        callable: definition.callable(),
                        block,
                        block_id: body.id,
                        instruction,
                        result: assignment.result,
                        operation,
                        operand,
                    })
                })
        })
        .collect()
}

fn validation_barriers(
    definition: MirDefinitionRef<'_>,
    site: CastSite,
) -> BTreeSet<PrimitiveCastBlocker> {
    let mut barriers = BTreeSet::new();
    if site.result.callable() != definition.callable()
        || site.operand.callable() != definition.callable()
        || definition.value(site.result).is_none()
        || definition.value(site.operand).is_none()
    {
        barriers.insert(PrimitiveCastBlocker::MalformedIdentity);
    }
    if !site.operation.is_semantically_consistent()
        || definition.value(site.operand).map(|value| value.ty)
            != Some(site.operation.source_type())
        || definition.value(site.result).map(|value| value.ty) != Some(site.operation.result_type())
    {
        barriers.insert(PrimitiveCastBlocker::UnsupportedTypeOrOperation);
    }
    barriers
}

fn add_replacement_barriers(
    barriers: &mut BTreeSet<PrimitiveCastBlocker>,
    uses: &crate::mir::rewrite::MirValueUseSites,
    definition_block: usize,
) {
    for use_site in uses.uses() {
        if !use_site.role().is_forwarding_safe() {
            barriers.insert(PrimitiveCastBlocker::ProtectedMetadataOrUse);
        }
        let same_block = match use_site.site() {
            MirLocalIdentitySite::Instruction { block, .. }
            | MirLocalIdentitySite::Terminator(block) => block == definition_block,
            _ => false,
        };
        if !same_block {
            barriers.insert(PrimitiveCastBlocker::ControlFlowBoundary);
        }
    }
}

/// Explicit complete-domain composition relation. This intentionally names
/// unsupported semantic families rather than consulting host representation.
fn compose(first: MirPrimitiveCast, second: MirPrimitiveCast) -> Composition {
    if first.target != second.source
        || !first.is_semantically_consistent()
        || !second.is_semantically_consistent()
    {
        return Composition::Unsupported;
    }
    if first.may_terminate() || second.may_terminate() {
        return Composition::CheckedFailure;
    }
    if first.kind() == MirPrimitiveCastKind::BitReinterpretation
        || second.kind() == MirPrimitiveCastKind::BitReinterpretation
    {
        return Composition::FloatingPayload;
    }
    if first.source == MirPrimitiveType::F64
        || first.target == MirPrimitiveType::F64
        || second.target == MirPrimitiveType::F64
    {
        return Composition::FloatingPayload;
    }

    if first.source.is_integer() && first.target.is_integer() && second.target.is_integer() {
        let loses_high_bits = first.target == MirPrimitiveType::U8
            && first.source != MirPrimitiveType::U8
            && second.target != MirPrimitiveType::U8;
        if loses_high_bits {
            return Composition::MissingValueDomain;
        }
        return if first.source == second.target {
            Composition::OriginalInput
        } else {
            Composition::DirectCast
        };
    }

    if first.source == MirPrimitiveType::Bool
        && first.target.is_integer()
        && second.target == MirPrimitiveType::Bool
    {
        return Composition::OriginalInput;
    }
    if first.target == MirPrimitiveType::Bool && second.target.is_integer() {
        return Composition::MissingValueDomain;
    }
    Composition::Unsupported
}

fn disposition(operation: MirPrimitiveCast) -> PrimitiveCastDisposition {
    match operation.kind() {
        MirPrimitiveCastKind::Identity => PrimitiveCastDisposition::Identity,
        MirPrimitiveCastKind::IntegerBits => match (operation.source, operation.target) {
            (MirPrimitiveType::I64 | MirPrimitiveType::U64, MirPrimitiveType::U8) => {
                PrimitiveCastDisposition::RequiredIntegerNarrowing
            }
            (MirPrimitiveType::U8, MirPrimitiveType::I64 | MirPrimitiveType::U64) => {
                PrimitiveCastDisposition::RequiredIntegerWidening
            }
            _ => PrimitiveCastDisposition::RequiredIntegerBitConversion,
        },
        MirPrimitiveCastKind::ToBool | MirPrimitiveCastKind::FromBool => {
            PrimitiveCastDisposition::BooleanCanonicalization
        }
        MirPrimitiveCastKind::ToF64 => PrimitiveCastDisposition::FloatingNumericConversion,
        MirPrimitiveCastKind::BitReinterpretation => {
            PrimitiveCastDisposition::RawBitReinterpretation
        }
        MirPrimitiveCastKind::CheckedF64ToInteger => {
            PrimitiveCastDisposition::CheckedFloatingToInteger
        }
    }
}

pub(super) fn consumer(role: MirValueUseRole) -> PrimitiveCastConsumer {
    use crate::mir::rewrite::MirValueUseRole::*;
    match role {
        OrdinaryScalarRvalue(_) => PrimitiveCastConsumer::TotalPrimitive,
        OrdinaryPrimitiveCast => PrimitiveCastConsumer::PrimitiveCast,
        OrdinaryStore => PrimitiveCastConsumer::Store,
        OrdinaryCall(_) => PrimitiveCastConsumer::Call,
        OrdinaryReturn => PrimitiveCastConsumer::Return,
        OrdinaryBranch => PrimitiveCastConsumer::ConditionalBranch,
        CheckedProtocol => PrimitiveCastConsumer::CheckedProtocol,
        ProofMetadata => PrimitiveCastConsumer::ProtectedMetadata,
        OwnershipOrLifecycle => PrimitiveCastConsumer::OwnershipOrLifecycle,
        InputOutput => PrimitiveCastConsumer::InputOutput,
        Unknown => PrimitiveCastConsumer::Other,
    }
}

#[derive(Default)]
struct Accumulator {
    counts: PrimitiveCastObservationCounts,
    supporting_values: BTreeSet<ValueId>,
    supporting_instructions: BTreeSet<(CallableId, usize, usize)>,
    examples: Vec<RedundancySiteExample<PrimitiveCastBlocker>>,
}

impl Accumulator {
    fn has_observations(&self) -> bool {
        self.counts.inspected != 0
            || self.counts.excluded_checked_conversions != 0
            || self.counts.excluded_checked_range_checks != 0
    }
    fn increment_inspected(&mut self) {
        add(&mut self.counts.inspected, 1, &mut self.counts.saturated);
    }
    fn increment_non_candidate(&mut self) {
        add(
            &mut self.counts.non_candidates,
            1,
            &mut self.counts.saturated,
        );
    }
    fn increment_checked_conversion(&mut self) {
        add(
            &mut self.counts.excluded_checked_conversions,
            1,
            &mut self.counts.saturated,
        );
        increment(
            &mut self.counts.dispositions,
            PrimitiveCastDisposition::CheckedFloatingToInteger,
            &mut self.counts.saturated,
        );
    }
    fn increment_checked_range_check(&mut self) {
        add(
            &mut self.counts.excluded_checked_range_checks,
            1,
            &mut self.counts.saturated,
        );
    }
    fn increment_shape(&mut self, key: PrimitiveCastShape) {
        increment(&mut self.counts.shapes, key, &mut self.counts.saturated);
    }
    fn increment_disposition(&mut self, key: PrimitiveCastDisposition) {
        increment(
            &mut self.counts.dispositions,
            key,
            &mut self.counts.saturated,
        );
    }
    fn increment_barrier(&mut self, key: PrimitiveCastBlocker) {
        increment(&mut self.counts.barriers, key, &mut self.counts.saturated);
    }
    fn increment_consumer(&mut self, key: PrimitiveCastConsumer) {
        increment(&mut self.counts.consumers, key, &mut self.counts.saturated);
    }

    fn record_interesting(
        &mut self,
        site: CastSite,
        predecessor: Option<CastSite>,
        barriers: BTreeSet<PrimitiveCastBlocker>,
        removable: bool,
    ) {
        add(&mut self.counts.interesting, 1, &mut self.counts.saturated);
        self.supporting_values.insert(site.result);
        self.supporting_instructions
            .insert((site.callable, site.block, site.instruction));
        if let Some(first) = predecessor {
            self.supporting_values.insert(first.result);
            self.supporting_instructions
                .insert((first.callable, first.block, first.instruction));
        }
        for barrier in barriers.iter().copied() {
            self.increment_barrier(barrier);
        }
        let classification = if barriers.is_empty() && removable {
            add(&mut self.counts.proven, 1, &mut self.counts.saturated);
            add(
                &mut self.counts.removable_values_upper_bound,
                1,
                &mut self.counts.saturated,
            );
            add(
                &mut self.counts.removable_instructions_upper_bound,
                1,
                &mut self.counts.saturated,
            );
            RedundancySiteClassification::Proven
        } else {
            add(&mut self.counts.blocked, 1, &mut self.counts.saturated);
            let blocker = barriers
                .iter()
                .next()
                .copied()
                .unwrap_or(PrimitiveCastBlocker::UnsupportedComposition);
            increment(
                &mut self.counts.primary_blockers,
                blocker,
                &mut self.counts.saturated,
            );
            RedundancySiteClassification::Blocked
        };
        merge_examples(
            &mut self.examples,
            &[RedundancySiteExample::new(
                site.callable,
                site.block_id,
                site.instruction,
                Some(site.result),
                classification,
                barriers.into_iter().collect(),
            )],
        );
    }

    fn merge(&mut self, other: &Self) {
        macro_rules! merge_field {
            ($field:ident) => {
                add(
                    &mut self.counts.$field,
                    other.counts.$field,
                    &mut self.counts.saturated,
                );
            };
        }
        merge_field!(inspected);
        merge_field!(interesting);
        merge_field!(proven);
        merge_field!(blocked);
        merge_field!(non_candidates);
        merge_field!(removable_values_upper_bound);
        merge_field!(removable_instructions_upper_bound);
        merge_field!(excluded_checked_conversions);
        merge_field!(excluded_checked_range_checks);
        merge_counts(
            &mut self.counts.shapes,
            &other.counts.shapes,
            &mut self.counts.saturated,
        );
        merge_counts(
            &mut self.counts.dispositions,
            &other.counts.dispositions,
            &mut self.counts.saturated,
        );
        merge_counts(
            &mut self.counts.primary_blockers,
            &other.counts.primary_blockers,
            &mut self.counts.saturated,
        );
        merge_counts(
            &mut self.counts.barriers,
            &other.counts.barriers,
            &mut self.counts.saturated,
        );
        merge_counts(
            &mut self.counts.consumers,
            &other.counts.consumers,
            &mut self.counts.saturated,
        );
        self.supporting_values
            .extend(other.supporting_values.iter().copied());
        self.supporting_instructions
            .extend(other.supporting_instructions.iter().copied());
        merge_examples(&mut self.examples, &other.examples);
    }

    fn finish(mut self, affected_callables: u64) -> PrimitiveCastObservationCounts {
        self.counts.affected_callables = affected_callables;
        self.counts.supporting_values = self.supporting_values.len() as u64;
        self.counts.supporting_instructions = self.supporting_instructions.len() as u64;
        self.counts
    }
}

fn add(total: &mut u64, value: u64, saturated: &mut bool) {
    let (sum, overflow) = total.overflowing_add(value);
    *total = if overflow { u64::MAX } else { sum };
    *saturated |= overflow;
}

fn increment<T: Copy + Eq>(counts: &mut Vec<PrimitiveCastCount<T>>, key: T, saturated: &mut bool) {
    if let Some(count) = counts.iter_mut().find(|count| count.key == key) {
        add(&mut count.sites, 1, saturated);
    } else {
        counts.push(PrimitiveCastCount::new(key, 1));
    }
}

fn merge_counts<T: Copy + Eq>(
    target: &mut Vec<PrimitiveCastCount<T>>,
    source: &[PrimitiveCastCount<T>],
    saturated: &mut bool,
) {
    for source_count in source {
        if let Some(count) = target
            .iter_mut()
            .find(|count| count.key == source_count.key)
        {
            add(&mut count.sites, source_count.sites, saturated);
        } else {
            target.push(*source_count);
        }
    }
}

#[cfg(test)]
#[path = "primitive_cast/tests.rs"]
mod tests;
