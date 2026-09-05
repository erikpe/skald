//! Immutable whole-callable plans for checked-integer protocol folding.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::{
    identity::CallableId,
    mir::{
        rewrite::{MirCallableEdit, MirCallableEditSnapshot, MirRewriteError},
        MirDefinitionRef, MirIntegerDivisionKind, MirProgram, MirTerminationReason,
    },
};

use super::super::{
    checked_integer_rewrite::{
        apply_checked_integer_protocol, validate_checked_integer_protocol,
        CheckedIntegerProtocolCandidate,
    },
    checked_integer_topology::{
        observe_checked_integer_topologies, CheckedIntegerProtocolCheck,
        CheckedIntegerTopologyObservation,
    },
    local_constant::{
        checked_carrier_plan_evidence, solve_local_constants, CheckedCarrierPlanEvidence,
        CheckedCarrierPlanRole, LocalConstantAnalysisError, LocalConstantFact,
    },
};

/// Checked-operation families selected while preparing one fold plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline::optimizations) enum CheckedIntegerFoldSelection {
    #[cfg(test)]
    DivisionAndRemainder,
    #[cfg(test)]
    Shift,
    All,
}

impl CheckedIntegerFoldSelection {
    const fn contains(self, _check: CheckedIntegerProtocolCheck) -> bool {
        match self {
            #[cfg(test)]
            Self::DivisionAndRemainder => {
                matches!(_check, CheckedIntegerProtocolCheck::Division(_))
            }
            #[cfg(test)]
            Self::Shift => matches!(_check, CheckedIntegerProtocolCheck::Shift(_)),
            Self::All => true,
        }
    }

    const fn contains_failure(self, reason: MirTerminationReason) -> bool {
        match self {
            #[cfg(test)]
            Self::DivisionAndRemainder => matches!(
                reason,
                MirTerminationReason::IntegerDivisionByZero
                    | MirTerminationReason::IntegerRemainderByZero
            ),
            #[cfg(test)]
            Self::Shift => matches!(reason, MirTerminationReason::ShiftCountOutOfRange),
            Self::All => matches!(
                reason,
                MirTerminationReason::IntegerDivisionByZero
                    | MirTerminationReason::IntegerRemainderByZero
                    | MirTerminationReason::ShiftCountOutOfRange
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CheckedIntegerFoldCounts {
    pub(super) quotients: usize,
    pub(super) remainders: usize,
    pub(super) shifts: usize,
    pub(super) propagated_operand_folds: usize,
    pub(super) retained_static_failures: usize,
}

impl CheckedIntegerFoldCounts {
    fn record_candidate(&mut self, candidate: &CheckedIntegerProtocolCandidate) {
        let count = match candidate.check {
            CheckedIntegerProtocolCheck::Division(check) => match check.operation.kind {
                MirIntegerDivisionKind::Quotient => &mut self.quotients,
                MirIntegerDivisionKind::Remainder => &mut self.remainders,
            },
            CheckedIntegerProtocolCheck::Shift(_) => &mut self.shifts,
        };
        *count = count.saturating_add(1);
        self.propagated_operand_folds = self
            .propagated_operand_folds
            .saturating_add(usize::from(candidate.has_propagated_operand()));
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline::optimizations) enum CheckedIntegerFoldPlanError {
    Rewrite(MirRewriteError),
    Analysis(LocalConstantAnalysisError),
    ConflictingCandidates { callable: CallableId },
}

impl fmt::Display for CheckedIntegerFoldPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rewrite(error) => error.fmt(formatter),
            Self::Analysis(error) => error.fmt(formatter),
            Self::ConflictingCandidates { callable } => write!(
                formatter,
                "checked-integer fold plan for {callable} contains conflicting edits"
            ),
        }
    }
}

impl From<MirRewriteError> for CheckedIntegerFoldPlanError {
    fn from(value: MirRewriteError) -> Self {
        Self::Rewrite(value)
    }
}

impl From<LocalConstantAnalysisError> for CheckedIntegerFoldPlanError {
    fn from(value: LocalConstantAnalysisError) -> Self {
        Self::Analysis(value)
    }
}

/// One source snapshot and every checked edit derived from it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckedIntegerCallableFoldPlan {
    snapshot: MirCallableEditSnapshot,
    pub(super) candidates: Vec<CheckedIntegerProtocolCandidate>,
}

/// All checked replacements derived from one immutable verified program.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::passes::pipeline::optimizations) struct CheckedIntegerFoldPlan {
    pub(super) callables: BTreeMap<CallableId, CheckedIntegerCallableFoldPlan>,
    processed_callables: usize,
    counts: CheckedIntegerFoldCounts,
}

impl CheckedIntegerFoldPlan {
    pub(in crate::passes::pipeline::optimizations) fn prepare(
        program: &MirProgram,
        selection: CheckedIntegerFoldSelection,
    ) -> Result<Self, CheckedIntegerFoldPlanError> {
        let mut plan = Self::default();
        for definition in program.executable_definitions() {
            plan.processed_callables = plan.processed_callables.saturating_add(1);
            plan.prepare_definition(definition, selection)?;
        }
        Ok(plan)
    }

    fn prepare_definition(
        &mut self,
        definition: MirDefinitionRef<'_>,
        selection: CheckedIntegerFoldSelection,
    ) -> Result<(), CheckedIntegerFoldPlanError> {
        let solution = solve_local_constants(definition)?;
        let evidence = checked_carrier_plan_evidence(definition)?
            .into_iter()
            .map(|evidence| ((evidence.check_block(), evidence.role()), evidence))
            .collect::<BTreeMap<_, _>>();
        let failures = solution
            .retained_checked_failures()
            .iter()
            .map(|failure| ((failure.check_block(), failure.result()), failure.reason()))
            .collect::<BTreeMap<_, _>>();
        let mut candidates = Vec::new();

        for observation in observe_checked_integer_topologies(definition)? {
            let CheckedIntegerTopologyObservation::Protocol(topology) = observation else {
                continue;
            };
            if topology.protected || !selection.contains(topology.check) {
                continue;
            }
            if let Some(reason) =
                failures.get(&(topology.check_block, topology.result_assignment.value))
            {
                if selection.contains_failure(*reason) {
                    self.counts.retained_static_failures =
                        self.counts.retained_static_failures.saturating_add(1);
                }
                continue;
            }

            let carrier = |role| evidence.get(&(topology.check_block, role)).cloned();
            let (Some(first), Some(second), Some(result)) = (
                carrier(CheckedCarrierPlanRole::FirstOperand),
                carrier(CheckedCarrierPlanRole::SecondOperand),
                carrier(CheckedCarrierPlanRole::Result),
            ) else {
                continue;
            };
            let carriers: [CheckedCarrierPlanEvidence; 3] = [first, second, result];
            let (Some(first), Some(second)) = (
                solution.fact(carriers[0].source())?,
                solution.fact(carriers[1].source())?,
            ) else {
                continue;
            };
            let operand_facts: [LocalConstantFact; 2] = [first, second];
            let mut loads_match_sources = true;
            for (load, source) in topology.operand_loads.iter().zip(operand_facts.iter()) {
                if solution.constant(load.value)? != Some(source.constant()) {
                    loads_match_sources = false;
                    break;
                }
            }
            if !loads_match_sources {
                continue;
            }
            let Some(constant) = solution.constant(topology.result_assignment.value)? else {
                continue;
            };
            let Some(candidate) = CheckedIntegerProtocolCandidate::from_solution(
                *topology,
                carriers,
                operand_facts,
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
            return Err(CheckedIntegerFoldPlanError::ConflictingCandidates {
                callable: definition.callable(),
            });
        }
        self.callables.insert(
            definition.callable(),
            CheckedIntegerCallableFoldPlan {
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
    ) -> impl Iterator<Item = &CheckedIntegerProtocolCandidate> {
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

    pub(super) const fn counts(&self) -> CheckedIntegerFoldCounts {
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
        plan.snapshot.validate(edit, "checked-integer fold plan")?;
        if !candidates_are_non_conflicting(&plan.candidates) {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable,
                subject: "checked-integer fold plan conflicts",
            });
        }
        for candidate in &plan.candidates {
            validate_checked_integer_protocol(edit, candidate)?;
        }

        let mut removed_operand_loads = 0usize;
        for candidate in &plan.candidates {
            let rewrite = apply_checked_integer_protocol(edit, candidate)?;
            removed_operand_loads =
                removed_operand_loads.saturating_add(rewrite.removed_operand_loads);
        }
        Ok(removed_operand_loads)
    }
}

fn candidates_are_non_conflicting(candidates: &[CheckedIntegerProtocolCandidate]) -> bool {
    let mut edited_blocks = BTreeSet::new();
    let mut removed_values = BTreeSet::new();
    let mut preserved_values = BTreeSet::new();
    for candidate in candidates {
        if !edited_blocks.insert(candidate.check_block)
            || !edited_blocks.insert(candidate.success_block)
        {
            return false;
        }
        preserved_values.extend([
            candidate.operands[0].source_value,
            candidate.operands[1].source_value,
            candidate.result_assignment.value,
            candidate.result_reload.value,
        ]);
        for load in candidate.operand_loads {
            if !removed_values.insert(load.value) {
                return false;
            }
        }
    }
    removed_values.is_disjoint(&preserved_values)
}
