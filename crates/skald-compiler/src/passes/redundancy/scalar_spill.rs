//! Conservative constant provenance through compiler-owned scalar spills.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    mir::{
        checked_scalar_dominates,
        rewrite::{
            value_use_sites_for_definition, MirLocalIdentitySite, MirRewriteError, MirValueUseRole,
        },
        BlockId, MirAssignment, MirDefinitionRef, MirInstruction, MirPlace, MirPlaceBase,
        MirRvalueKind, MirStorageKind, MirType, StorageId, ValueId,
    },
    passes::{
        pipeline::{
            evaluate_integer_division, evaluate_rvalue, evaluate_shift, CheckedIntegerEvaluation,
            PrimitiveConstant, PrimitiveEvaluation,
        },
        VerifiedFinalMirProgram,
    },
};

use super::model::{
    ScalarSpillBlocker, ScalarSpillCallableObservation, ScalarSpillConsumer, ScalarSpillCount,
    ScalarSpillDepth, ScalarSpillProvenanceCounts, ScalarSpillProvenanceObservation,
    ScalarSpillUnlock,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct InstructionSite {
    block: BlockId,
    instruction: usize,
}

#[derive(Clone, Copy)]
struct AssignmentSite<'mir> {
    site: InstructionSite,
    assignment: &'mir MirAssignment,
}

#[derive(Clone, Copy)]
struct StoreSite {
    site: InstructionSite,
    value: ValueId,
    exact: bool,
}

struct Trace {
    constant: PrimitiveConstant,
    hops: usize,
    barriers: BTreeSet<ScalarSpillBlocker>,
    values: BTreeSet<ValueId>,
    instructions: BTreeSet<InstructionSite>,
}

enum TraceResult {
    Constant(Trace),
    NotConstantShaped,
}

/// Measures constant provenance through exact scalar-spill chains without
/// cloning, mutating, or invalidating the verified final-MIR product.
pub fn analyze_scalar_spill_provenance(
    verified: &VerifiedFinalMirProgram,
) -> ScalarSpillProvenanceObservation {
    let mut total = Accumulator::default();
    let mut callables = Vec::new();
    for definition in verified.program().executable_definitions() {
        let callable = definition.callable();
        let observed = analyze_definition(definition)
            .expect("verified final MIR must have coherent callable-local identities");
        if observed.inspected != 0 {
            total.merge(&observed);
            callables.push(ScalarSpillCallableObservation::new(
                callable,
                observed.finish(1),
            ));
        }
    }
    let affected_callables = callables.len() as u64;
    ScalarSpillProvenanceObservation::new(total.finish(affected_callables), callables)
}

fn analyze_definition(definition: MirDefinitionRef<'_>) -> Result<Accumulator, MirRewriteError> {
    let index = DefinitionIndex::new(definition);
    let mut observed = Accumulator::default();

    for assignment in index.assignments.values().copied() {
        let MirRvalueKind::Load(place) = &assignment.assignment.rvalue.kind else {
            continue;
        };
        let Some(storage) = place.base.local_storage() else {
            continue;
        };
        if definition
            .storage_entries()
            .get(storage.index())
            .is_none_or(|declaration| declaration.kind != MirStorageKind::ScalarSpill)
        {
            continue;
        }

        let uses = value_use_sites_for_definition(definition, assignment.assignment.result)?;
        for use_site in uses.uses() {
            observed.increment_inspected();
            let mut visiting = BTreeSet::new();
            match index.trace_load(assignment, &mut visiting) {
                TraceResult::NotConstantShaped => observed.increment_non_candidate(),
                TraceResult::Constant(mut trace) => {
                    let consumer = consumer(use_site.role());
                    increment(&mut observed.consumers, consumer, &mut observed.saturated);
                    increment(
                        &mut observed.depths,
                        depth(trace.hops),
                        &mut observed.saturated,
                    );
                    if let Some(unlock) = index.unlock(
                        assignment.assignment.result,
                        trace.constant,
                        use_site.site(),
                        consumer,
                    ) {
                        increment(&mut observed.unlocks, unlock, &mut observed.saturated);
                    }
                    add_use_barrier(&mut trace.barriers, use_site.role());
                    if trace.barriers.is_empty() {
                        observed.increment_proven();
                    } else {
                        observed.increment_blocked();
                        increment(
                            &mut observed.primary_blockers,
                            *trace.barriers.first().expect("blocked site has a barrier"),
                            &mut observed.saturated,
                        );
                        for blocker in trace.barriers {
                            increment(&mut observed.barriers, blocker, &mut observed.saturated);
                        }
                    }
                    observed.supporting_values.extend(trace.values);
                    observed.supporting_instructions.extend(trace.instructions);
                }
            }
        }
    }
    Ok(observed)
}

#[cfg(test)]
pub(super) fn analyze_unverified_definition(
    definition: MirDefinitionRef<'_>,
) -> Result<ScalarSpillProvenanceCounts, MirRewriteError> {
    analyze_definition(definition).map(|observed| observed.finish(1))
}

struct DefinitionIndex<'mir> {
    definition: MirDefinitionRef<'mir>,
    assignments: BTreeMap<ValueId, AssignmentSite<'mir>>,
    stores: BTreeMap<StorageId, Vec<StoreSite>>,
}

impl<'mir> DefinitionIndex<'mir> {
    fn new(definition: MirDefinitionRef<'mir>) -> Self {
        let mut assignments = BTreeMap::new();
        let mut stores = BTreeMap::<_, Vec<_>>::new();
        for block in &definition.body().blocks {
            for (instruction, item) in block.instructions.iter().enumerate() {
                let site = InstructionSite {
                    block: block.id,
                    instruction,
                };
                match item {
                    MirInstruction::Assign(assignment) => {
                        assignments.insert(assignment.result, AssignmentSite { site, assignment });
                    }
                    MirInstruction::Store(store) => {
                        if let Some(storage) = store.destination.base.local_storage() {
                            stores.entry(storage).or_default().push(StoreSite {
                                site,
                                value: store.value,
                                exact: store.destination == MirPlace::base(storage),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        Self {
            definition,
            assignments,
            stores,
        }
    }

    fn trace_load(
        &self,
        load: AssignmentSite<'_>,
        visiting: &mut BTreeSet<StorageId>,
    ) -> TraceResult {
        let MirRvalueKind::Load(place) = &load.assignment.rvalue.kind else {
            return TraceResult::NotConstantShaped;
        };
        let Some(storage) = place.base.local_storage() else {
            return TraceResult::NotConstantShaped;
        };
        if !visiting.insert(storage) {
            return TraceResult::NotConstantShaped;
        }

        let mut malformed = false;
        let Some(declaration) = self.definition.storage_entries().get(storage.index()) else {
            return TraceResult::NotConstantShaped;
        };
        malformed |= declaration.id != storage || storage.callable() != self.definition.callable();
        let writes = self
            .stores
            .get(&storage)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut shaped = writes.iter().filter_map(|store| {
            self.trace_store_source(*store, visiting)
                .map(|trace| (*store, trace))
        });
        let Some((selected_store, mut trace)) = shaped.next() else {
            visiting.remove(&storage);
            return TraceResult::NotConstantShaped;
        };
        if shaped.next().is_some() || writes.len() != 1 {
            trace.barriers.insert(ScalarSpillBlocker::AmbiguousWrites);
        }
        if malformed {
            trace.barriers.insert(ScalarSpillBlocker::MalformedIdentity);
        }
        if declaration.kind != MirStorageKind::ScalarSpill
            || !declaration.ty.is_primitive()
            || declaration.ty != load.assignment.rvalue.ty
            || self
                .definition
                .value(load.assignment.result)
                .map(|value| value.ty)
                != Some(declaration.ty)
        {
            trace
                .barriers
                .insert(ScalarSpillBlocker::UnsupportedTypeOrOperation);
        }
        if *place != MirPlace::base(storage) || !selected_store.exact {
            trace.barriers.insert(ScalarSpillBlocker::NoncanonicalPlace);
        }
        if !matches!(place.base, MirPlaceBase::Storage(_)) {
            trace.barriers.insert(ScalarSpillBlocker::AliasExposure);
        }
        if !dominates(self.definition, selected_store.site, load.site) {
            trace.barriers.insert(ScalarSpillBlocker::MissingDominance);
        }
        trace.values.insert(load.assignment.result);
        trace.instructions.insert(load.site);
        trace.instructions.insert(selected_store.site);
        visiting.remove(&storage);
        TraceResult::Constant(trace)
    }

    fn trace_store_source(
        &self,
        store: StoreSite,
        visiting: &mut BTreeSet<StorageId>,
    ) -> Option<Trace> {
        let source = self.assignments.get(&store.value).copied()?;
        let mut trace = match &source.assignment.rvalue.kind {
            MirRvalueKind::ConstantI64(value) => direct(PrimitiveConstant::I64(*value)),
            MirRvalueKind::ConstantU64(value) => direct(PrimitiveConstant::U64(*value)),
            MirRvalueKind::ConstantU8(value) => direct(PrimitiveConstant::U8(*value)),
            MirRvalueKind::ConstantBool(value) => direct(PrimitiveConstant::Bool(*value)),
            MirRvalueKind::Load(_) => match self.trace_load(source, visiting) {
                TraceResult::Constant(mut trace) => {
                    trace.hops += 1;
                    trace
                }
                TraceResult::NotConstantShaped => return None,
            },
            kind => {
                let constant = self.constant_at(source.site, kind, None)?;
                let mut trace = direct(constant);
                trace
                    .barriers
                    .insert(ScalarSpillBlocker::OtherUnsupportedProducer);
                trace
            }
        };
        if self.definition.value(store.value).map(|value| value.ty) != Some(trace.constant.ty()) {
            trace
                .barriers
                .insert(ScalarSpillBlocker::UnsupportedTypeOrOperation);
        }
        if !dominates(self.definition, source.site, store.site) {
            trace.barriers.insert(ScalarSpillBlocker::MissingDominance);
        }
        trace.values.insert(store.value);
        trace.instructions.insert(source.site);
        Some(trace)
    }

    fn constant_at(
        &self,
        site: InstructionSite,
        kind: &MirRvalueKind,
        replacement: Option<(ValueId, PrimitiveConstant)>,
    ) -> Option<PrimitiveConstant> {
        let result = evaluate_rvalue(kind, |value| {
            if replacement.is_some_and(|(selected, _)| selected == value) {
                return replacement.map(|(_, constant)| constant);
            }
            let source = self.assignments.get(&value)?;
            if !dominates(self.definition, source.site, site) {
                return None;
            }
            literal(&source.assignment.rvalue.kind)
        });
        match result {
            PrimitiveEvaluation::Constant(constant) => Some(constant),
            PrimitiveEvaluation::Unsupported => None,
        }
    }

    fn unlock(
        &self,
        selected: ValueId,
        constant: PrimitiveConstant,
        use_site: MirLocalIdentitySite,
        consumer: ScalarSpillConsumer,
    ) -> Option<ScalarSpillUnlock> {
        match consumer {
            ScalarSpillConsumer::PrimitiveCast => Some(ScalarSpillUnlock::CastSimplification),
            ScalarSpillConsumer::ConditionalBranch if constant.ty() == MirType::Bool => {
                Some(ScalarSpillUnlock::BranchFolding)
            }
            ScalarSpillConsumer::CheckedIntegerProtocol => {
                let assignment = self.assignment_at(use_site)?;
                checked_unlock(self, assignment, selected, constant)
                    .then_some(ScalarSpillUnlock::CheckedFolding)
            }
            ScalarSpillConsumer::TotalPrimitive => {
                let assignment = self.assignment_at(use_site)?;
                self.constant_at(
                    assignment.site,
                    &assignment.assignment.rvalue.kind,
                    Some((selected, constant)),
                )
                .map(|_| ScalarSpillUnlock::PrimitiveFolding)
            }
            ScalarSpillConsumer::Store
            | ScalarSpillConsumer::Return
            | ScalarSpillConsumer::Call
            | ScalarSpillConsumer::Other => Some(ScalarSpillUnlock::DirectSubstitution),
            ScalarSpillConsumer::ConditionalBranch => None,
        }
    }

    fn assignment_at(&self, site: MirLocalIdentitySite) -> Option<AssignmentSite<'mir>> {
        let MirLocalIdentitySite::Instruction { block, instruction } = site else {
            return None;
        };
        let item = self
            .definition
            .body()
            .blocks
            .get(block)?
            .instructions
            .get(instruction)?;
        let MirInstruction::Assign(assignment) = item else {
            return None;
        };
        Some(AssignmentSite {
            site: InstructionSite {
                block: self.definition.body().blocks[block].id,
                instruction,
            },
            assignment,
        })
    }
}

fn checked_unlock(
    index: &DefinitionIndex<'_>,
    assignment: AssignmentSite<'_>,
    selected: ValueId,
    replacement: PrimitiveConstant,
) -> bool {
    let constant = |value| {
        (value == selected).then_some(replacement).or_else(|| {
            index
                .assignments
                .get(&value)
                .filter(|source| dominates(index.definition, source.site, assignment.site))
                .and_then(|source| literal(&source.assignment.rvalue.kind))
        })
    };
    match &assignment.assignment.rvalue.kind {
        MirRvalueKind::IntegerDivision {
            operation,
            dividend,
            divisor,
        } => {
            matches!((constant(*dividend), constant(*divisor)), (Some(left), Some(right))
                if matches!(evaluate_integer_division(*operation, left, right), CheckedIntegerEvaluation::Success(_)))
        }
        MirRvalueKind::Shift {
            operation,
            left,
            count,
        } => {
            matches!((constant(*left), constant(*count)), (Some(left), Some(right))
                if matches!(evaluate_shift(*operation, left, right), CheckedIntegerEvaluation::Success(_)))
        }
        _ => false,
    }
}

fn literal(kind: &MirRvalueKind) -> Option<PrimitiveConstant> {
    match kind {
        MirRvalueKind::ConstantI64(value) => Some(PrimitiveConstant::I64(*value)),
        MirRvalueKind::ConstantU64(value) => Some(PrimitiveConstant::U64(*value)),
        MirRvalueKind::ConstantU8(value) => Some(PrimitiveConstant::U8(*value)),
        MirRvalueKind::ConstantBool(value) => Some(PrimitiveConstant::Bool(*value)),
        _ => None,
    }
}

fn direct(constant: PrimitiveConstant) -> Trace {
    Trace {
        constant,
        hops: 0,
        barriers: BTreeSet::new(),
        values: BTreeSet::new(),
        instructions: BTreeSet::new(),
    }
}

fn dominates(
    definition: MirDefinitionRef<'_>,
    first: InstructionSite,
    later: InstructionSite,
) -> bool {
    if first.block == later.block {
        first.instruction < later.instruction
    } else {
        checked_scalar_dominates(definition, first.block, later.block)
    }
}

fn depth(hops: usize) -> ScalarSpillDepth {
    match hops {
        0 => ScalarSpillDepth::Direct,
        1 => ScalarSpillDepth::OneHop,
        _ => ScalarSpillDepth::Transitive,
    }
}

pub(super) fn consumer(role: MirValueUseRole) -> ScalarSpillConsumer {
    match role {
        MirValueUseRole::CheckedProtocol => ScalarSpillConsumer::CheckedIntegerProtocol,
        MirValueUseRole::OrdinaryScalarRvalue(_) => ScalarSpillConsumer::TotalPrimitive,
        MirValueUseRole::OrdinaryPrimitiveCast => ScalarSpillConsumer::PrimitiveCast,
        MirValueUseRole::OrdinaryBranch => ScalarSpillConsumer::ConditionalBranch,
        MirValueUseRole::OrdinaryStore => ScalarSpillConsumer::Store,
        MirValueUseRole::OrdinaryReturn => ScalarSpillConsumer::Return,
        MirValueUseRole::OrdinaryCall(_) => ScalarSpillConsumer::Call,
        MirValueUseRole::ProofMetadata
        | MirValueUseRole::OwnershipOrLifecycle
        | MirValueUseRole::InputOutput
        | MirValueUseRole::Unknown => ScalarSpillConsumer::Other,
    }
}

pub(super) fn add_use_barrier(barriers: &mut BTreeSet<ScalarSpillBlocker>, role: MirValueUseRole) {
    match role {
        MirValueUseRole::CheckedProtocol | MirValueUseRole::ProofMetadata => {
            barriers.insert(ScalarSpillBlocker::ProtectedMetadataOrUse);
        }
        MirValueUseRole::OwnershipOrLifecycle => {
            barriers.insert(ScalarSpillBlocker::LifecycleParticipation);
        }
        MirValueUseRole::InputOutput | MirValueUseRole::Unknown => {
            barriers.insert(ScalarSpillBlocker::UnsupportedTypeOrOperation);
        }
        _ => {}
    }
}

#[derive(Default)]
struct Accumulator {
    inspected: u64,
    interesting: u64,
    proven: u64,
    blocked: u64,
    non_candidates: u64,
    saturated: bool,
    depths: BTreeMap<ScalarSpillDepth, u64>,
    primary_blockers: BTreeMap<ScalarSpillBlocker, u64>,
    barriers: BTreeMap<ScalarSpillBlocker, u64>,
    consumers: BTreeMap<ScalarSpillConsumer, u64>,
    unlocks: BTreeMap<ScalarSpillUnlock, u64>,
    supporting_values: BTreeSet<ValueId>,
    supporting_instructions: BTreeSet<InstructionSite>,
}

impl Accumulator {
    fn increment_inspected(&mut self) {
        add(&mut self.inspected, 1, &mut self.saturated);
    }
    fn increment_non_candidate(&mut self) {
        add(&mut self.non_candidates, 1, &mut self.saturated);
    }
    fn increment_proven(&mut self) {
        add(&mut self.interesting, 1, &mut self.saturated);
        add(&mut self.proven, 1, &mut self.saturated);
    }
    fn increment_blocked(&mut self) {
        add(&mut self.interesting, 1, &mut self.saturated);
        add(&mut self.blocked, 1, &mut self.saturated);
    }
    fn merge(&mut self, other: &Self) {
        for (target, value) in [
            (&mut self.inspected, other.inspected),
            (&mut self.interesting, other.interesting),
            (&mut self.proven, other.proven),
            (&mut self.blocked, other.blocked),
            (&mut self.non_candidates, other.non_candidates),
        ] {
            add(target, value, &mut self.saturated);
        }
        merge_map(&mut self.depths, &other.depths, &mut self.saturated);
        merge_map(
            &mut self.primary_blockers,
            &other.primary_blockers,
            &mut self.saturated,
        );
        merge_map(&mut self.barriers, &other.barriers, &mut self.saturated);
        merge_map(&mut self.consumers, &other.consumers, &mut self.saturated);
        merge_map(&mut self.unlocks, &other.unlocks, &mut self.saturated);
        self.supporting_values.extend(&other.supporting_values);
        self.supporting_instructions
            .extend(&other.supporting_instructions);
        self.saturated |= other.saturated;
    }
    fn finish(self, affected_callables: u64) -> ScalarSpillProvenanceCounts {
        ScalarSpillProvenanceCounts {
            inspected: self.inspected,
            interesting: self.interesting,
            proven: self.proven,
            blocked: self.blocked,
            non_candidates: self.non_candidates,
            affected_callables,
            supporting_values: self.supporting_values.len() as u64,
            supporting_instructions: self.supporting_instructions.len() as u64,
            removable_values_upper_bound: self.proven,
            removable_instructions_upper_bound: self.proven,
            saturated: self.saturated,
            depths: counts(self.depths),
            primary_blockers: counts(self.primary_blockers),
            barriers: counts(self.barriers),
            consumers: counts(self.consumers),
            unlocks: counts(self.unlocks),
        }
    }
}

fn add(target: &mut u64, value: u64, saturated: &mut bool) {
    let (sum, overflow) = target.overflowing_add(value);
    if overflow {
        *target = u64::MAX;
        *saturated = true;
    } else {
        *target = sum;
    }
}
fn increment<T: Ord>(map: &mut BTreeMap<T, u64>, key: T, saturated: &mut bool) {
    add(map.entry(key).or_default(), 1, saturated);
}
fn merge_map<T: Copy + Ord>(
    target: &mut BTreeMap<T, u64>,
    source: &BTreeMap<T, u64>,
    saturated: &mut bool,
) {
    for (key, value) in source {
        add(target.entry(*key).or_default(), *value, saturated);
    }
}
fn counts<T>(map: BTreeMap<T, u64>) -> Vec<ScalarSpillCount<T>> {
    map.into_iter()
        .map(|(key, sites)| ScalarSpillCount::new(key, sites))
        .collect()
}
