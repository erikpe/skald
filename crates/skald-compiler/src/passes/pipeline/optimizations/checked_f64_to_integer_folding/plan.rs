//! Immutable whole-callable plans for checked floating-cast folding.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::{
    identity::CallableId,
    mir::{
        rewrite::{MirCallableEdit, MirCallableEditSnapshot, MirRewriteError},
        MirDefinitionRef, MirIntegerType, MirTerminationReason,
    },
};

use super::super::{
    checked_f64_to_integer_rewrite::{
        apply_checked_f64_to_integer_protocol, validate_checked_f64_to_integer_protocol,
        CheckedF64ToIntegerProtocolCandidate,
    },
    checked_f64_to_integer_topology::{
        observe_checked_f64_to_integer_topologies, CheckedF64ToIntegerTopologyObservation,
    },
    local_constant::{
        checked_f64_to_integer_carrier_plan_evidence, CheckedCarrierPlanEvidence,
        CheckedCarrierPlanRole, LocalConstantAnalysisError,
    },
};
use crate::passes::pipeline::snapshot_analysis::MirProofPassContext;
#[cfg(test)]
use crate::{mir::MirProgram, passes::pipeline::optimizations::solve_local_constants};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CheckedF64ToIntegerFoldCounts {
    pub(super) i64: usize,
    pub(super) u64: usize,
    pub(super) u8: usize,
    pub(super) propagated_source_folds: usize,
    pub(super) retained_static_failures: usize,
}

impl CheckedF64ToIntegerFoldCounts {
    fn record_candidate(&mut self, candidate: &CheckedF64ToIntegerProtocolCandidate) {
        let count = match candidate.check.relation.target {
            MirIntegerType::I64 => &mut self.i64,
            MirIntegerType::U64 => &mut self.u64,
            MirIntegerType::U8 => &mut self.u8,
        };
        *count = count.saturating_add(1);
        self.propagated_source_folds = self
            .propagated_source_folds
            .saturating_add(usize::from(candidate.has_propagated_source()));
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline::optimizations) enum CheckedF64ToIntegerFoldPlanError {
    Rewrite(MirRewriteError),
    Analysis(LocalConstantAnalysisError),
    ConflictingCandidates { callable: CallableId },
}

impl fmt::Display for CheckedF64ToIntegerFoldPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rewrite(error) => error.fmt(formatter),
            Self::Analysis(error) => error.fmt(formatter),
            Self::ConflictingCandidates { callable } => write!(
                formatter,
                "checked floating-to-integer fold plan for {callable} contains conflicting edits"
            ),
        }
    }
}

impl From<MirRewriteError> for CheckedF64ToIntegerFoldPlanError {
    fn from(value: MirRewriteError) -> Self {
        Self::Rewrite(value)
    }
}

impl From<LocalConstantAnalysisError> for CheckedF64ToIntegerFoldPlanError {
    fn from(value: LocalConstantAnalysisError) -> Self {
        Self::Analysis(value)
    }
}

/// One source snapshot and every checked floating-cast edit derived from it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckedF64ToIntegerCallableFoldPlan {
    snapshot: MirCallableEditSnapshot,
    pub(super) candidates: Vec<CheckedF64ToIntegerProtocolCandidate>,
}

/// All floating-cast replacements derived from one immutable verified program.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::passes::pipeline::optimizations) struct CheckedF64ToIntegerFoldPlan {
    pub(super) callables: BTreeMap<CallableId, CheckedF64ToIntegerCallableFoldPlan>,
    processed_callables: usize,
    counts: CheckedF64ToIntegerFoldCounts,
}

impl CheckedF64ToIntegerFoldPlan {
    pub(in crate::passes::pipeline::optimizations) fn prepare_with_context(
        context: &mut MirProofPassContext<'_>,
    ) -> Result<Self, CheckedF64ToIntegerFoldPlanError> {
        let mut plan = Self::default();
        for callable in context.executable_callables() {
            plan.processed_callables = plan.processed_callables.saturating_add(1);
            let solution = context.local_constants(callable)?;
            plan.prepare_definition(context.executable_definition(callable), &solution)?;
        }
        Ok(plan)
    }

    #[cfg(test)]
    pub(in crate::passes::pipeline::optimizations) fn prepare(
        program: &MirProgram,
    ) -> Result<Self, CheckedF64ToIntegerFoldPlanError> {
        let mut plan = Self::default();
        for definition in program.executable_definitions() {
            plan.processed_callables = plan.processed_callables.saturating_add(1);
            let solution = solve_local_constants(definition)?;
            plan.prepare_definition(definition, &solution)?;
        }
        Ok(plan)
    }

    fn prepare_definition(
        &mut self,
        definition: MirDefinitionRef<'_>,
        solution: &crate::passes::pipeline::optimizations::local_constant::LocalConstantSolution,
    ) -> Result<(), CheckedF64ToIntegerFoldPlanError> {
        let evidence = checked_f64_to_integer_carrier_plan_evidence(definition)?
            .into_iter()
            .map(|evidence| ((evidence.check_block(), evidence.role()), evidence))
            .collect::<BTreeMap<_, _>>();
        let failures = solution
            .retained_checked_failures()
            .iter()
            .map(|failure| ((failure.check_block(), failure.result()), failure.reason()))
            .collect::<BTreeMap<_, _>>();
        let mut candidates = Vec::new();

        for observation in observe_checked_f64_to_integer_topologies(definition)? {
            let CheckedF64ToIntegerTopologyObservation::Protocol(topology) = observation else {
                continue;
            };
            if topology.protected {
                continue;
            }
            if matches!(
                failures.get(&(topology.check_block, topology.result_assignment.value)),
                Some(MirTerminationReason::PrimitiveCastOutOfRange)
            ) {
                self.counts.retained_static_failures =
                    self.counts.retained_static_failures.saturating_add(1);
                continue;
            }

            let carrier = |role| evidence.get(&(topology.check_block, role)).cloned();
            let (Some(source), Some(result)) = (
                carrier(CheckedCarrierPlanRole::Source),
                carrier(CheckedCarrierPlanRole::Result),
            ) else {
                continue;
            };
            let carriers: [CheckedCarrierPlanEvidence; 2] = [source, result];
            let Some(source_fact) = solution.fact(carriers[0].source())? else {
                continue;
            };
            if solution.constant(topology.source_load.value)? != Some(source_fact.constant()) {
                continue;
            }
            let Some(constant) = solution.constant(topology.result_assignment.value)? else {
                continue;
            };
            let Some(candidate) = CheckedF64ToIntegerProtocolCandidate::from_solution(
                *topology,
                carriers,
                source_fact,
                constant,
            ) else {
                continue;
            };
            self.counts.record_candidate(&candidate);
            candidates.push(candidate);
        }

        if candidates.is_empty() {
            return Ok(());
        }
        if !candidates_are_non_conflicting(&candidates) {
            return Err(CheckedF64ToIntegerFoldPlanError::ConflictingCandidates {
                callable: definition.callable(),
            });
        }
        self.callables.insert(
            definition.callable(),
            CheckedF64ToIntegerCallableFoldPlan {
                snapshot: MirCallableEditSnapshot::capture(definition),
                candidates,
            },
        );
        Ok(())
    }

    pub(super) fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }

    #[cfg(test)]
    pub(super) fn candidate_count(&self) -> usize {
        self.callables
            .values()
            .map(|callable| callable.candidates.len())
            .sum()
    }

    #[cfg(test)]
    pub(in crate::passes::pipeline::optimizations) fn candidates(
        &self,
    ) -> impl Iterator<Item = &CheckedF64ToIntegerProtocolCandidate> {
        self.callables
            .values()
            .flat_map(|callable| &callable.candidates)
    }

    pub(super) fn changed_callable_count(&self) -> usize {
        self.callables.len()
    }

    pub(super) const fn processed_callables(&self) -> usize {
        self.processed_callables
    }

    pub(super) const fn counts(&self) -> CheckedF64ToIntegerFoldCounts {
        self.counts
    }

    /// Validates the complete source plan before applying its first edit.
    pub(super) fn rewrite_callable(
        &self,
        callable: CallableId,
        edit: &mut MirCallableEdit,
    ) -> Result<usize, MirRewriteError> {
        let Some(plan) = self.callables.get(&callable) else {
            return Ok(0);
        };
        plan.snapshot
            .validate(edit, "checked floating-to-integer fold plan")?;
        if !candidates_are_non_conflicting(&plan.candidates) {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable,
                subject: "checked floating-to-integer fold plan conflicts",
            });
        }
        for candidate in &plan.candidates {
            validate_checked_f64_to_integer_protocol(edit, candidate)?;
        }

        let mut removed_protocol_values = 0usize;
        for candidate in &plan.candidates {
            let rewrite = apply_checked_f64_to_integer_protocol(edit, candidate)?;
            removed_protocol_values =
                removed_protocol_values.saturating_add(rewrite.removed_protocol_values);
        }
        Ok(removed_protocol_values)
    }
}

fn candidates_are_non_conflicting(candidates: &[CheckedF64ToIntegerProtocolCandidate]) -> bool {
    let mut edited_blocks = BTreeSet::new();
    let mut removed_values = BTreeSet::new();
    let mut preserved_values = BTreeSet::new();
    for candidate in candidates {
        if !edited_blocks.insert(candidate.check_block)
            || !edited_blocks.insert(candidate.success_block)
            || !removed_values.insert(candidate.source_load.value)
        {
            return false;
        }
        preserved_values.extend([
            candidate.source.source_value,
            candidate.result_assignment.value,
            candidate.result_reload.value,
        ]);
    }
    removed_values.is_disjoint(&preserved_values)
}
