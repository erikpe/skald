//! Prepared deterministic folding of verified checked-integer protocols.
//!
//! Discovery borrows sealed dense MIR. Application later revalidates every
//! candidate against one sparse callable transaction before mutation, so no
//! cached protocol fact is trusted across an intervening edit.

use std::collections::BTreeMap;

use crate::{
    identity::CallableId,
    mir::{
        rewrite::{MirCallableEdit, MirRewriteError},
        MirIntegerDivisionKind, MirProgram, MirTerminationReason,
    },
};

use super::{
    super::{
        execution::{
            MirPassCapability, MirPassData, MirPassFailure, MirPassMeasurement, MirPassOutcome,
        },
        policy::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
        MirPassIdentity,
    },
    checked_integer_protocol::{
        observe_checked_integer_protocols, CheckedIntegerProtocolCandidate,
        CheckedIntegerProtocolCheck, CheckedIntegerProtocolObservation,
        CheckedIntegerProtocolRejectionReason,
    },
    checked_integer_rewrite::rewrite_checked_integer_protocol,
};

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(5);
const NAME: &str = "checked-integer-constant-folding";
const DESCRIPTION: &str = "Folds exact successful checked-integer constant protocols.";
const FOLDED_QUOTIENTS: &str = "folded quotient protocols";
const FOLDED_REMAINDERS: &str = "folded remainder protocols";
const FOLDED_SHIFTS: &str = "folded shift protocols";
const REMOVED_PROTOCOL_LOAD_VALUES: &str = "removed protocol-load values";
const RETAINED_STATIC_FAILURES: &str = "retained statically failing candidates";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, NAME, DESCRIPTION),
    MirPassImplementation::new(IDENTITY, transform),
);

/// Checked-operation families selected while preparing one fold plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CheckedIntegerFoldSelection {
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
}

/// Immutable seal-local candidates grouped in deterministic callable order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CheckedIntegerFoldPlan {
    candidates: BTreeMap<CallableId, Vec<CheckedIntegerProtocolCandidate>>,
    processed_callables: usize,
    counts: CheckedIntegerFoldCounts,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CheckedIntegerFoldCounts {
    quotients: usize,
    remainders: usize,
    shifts: usize,
    retained_static_failures: usize,
}

impl CheckedIntegerFoldCounts {
    fn record_candidate(&mut self, check: CheckedIntegerProtocolCheck) {
        let count = match check {
            CheckedIntegerProtocolCheck::Division(check) => match check.operation.kind {
                MirIntegerDivisionKind::Quotient => &mut self.quotients,
                MirIntegerDivisionKind::Remainder => &mut self.remainders,
            },
            CheckedIntegerProtocolCheck::Shift(_) => &mut self.shifts,
        };
        *count = count.saturating_add(1);
    }
}

impl CheckedIntegerFoldPlan {
    /// Observes one operation family without retaining general MIR facts.
    pub(super) fn prepare(
        program: &MirProgram,
        selection: CheckedIntegerFoldSelection,
    ) -> Result<Self, MirRewriteError> {
        let mut candidates = BTreeMap::<_, Vec<_>>::new();
        let mut processed_callables = 0usize;
        let mut counts = CheckedIntegerFoldCounts::default();
        for definition in program.executable_definitions() {
            processed_callables = processed_callables.saturating_add(1);
            for observation in observe_checked_integer_protocols(definition)? {
                match observation {
                    CheckedIntegerProtocolObservation::Candidate(candidate)
                        if selection.contains(candidate.check) =>
                    {
                        counts.record_candidate(candidate.check);
                        candidates
                            .entry(definition.callable())
                            .or_default()
                            .push(*candidate);
                    }
                    CheckedIntegerProtocolObservation::Rejected {
                        reason: CheckedIntegerProtocolRejectionReason::StaticFailure(reason),
                        ..
                    } if selection.contains_failure(reason) => {
                        counts.retained_static_failures =
                            counts.retained_static_failures.saturating_add(1);
                    }
                    CheckedIntegerProtocolObservation::Candidate(_)
                    | CheckedIntegerProtocolObservation::Rejected { .. } => {}
                }
            }
        }
        Ok(Self {
            candidates,
            processed_callables,
            counts,
        })
    }

    pub(super) fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    #[cfg(test)]
    pub(super) fn candidate_count(&self) -> usize {
        self.candidates.values().map(Vec::len).sum()
    }

    pub(super) fn changed_callable_count(&self) -> usize {
        self.candidates.len()
    }

    /// Applies every prepared candidate for one callable in captured block
    /// order. The surrounding all-program rewrite coordinator commits once.
    pub(super) fn rewrite_callable(
        &self,
        callable: CallableId,
        edit: &mut MirCallableEdit,
    ) -> Result<usize, MirRewriteError> {
        let Some(candidates) = self.candidates.get(&callable) else {
            return Ok(0);
        };
        let mut removed_operand_loads = 0usize;
        for candidate in candidates {
            let rewrite = rewrite_checked_integer_protocol(edit, candidate)?;
            removed_operand_loads =
                removed_operand_loads.saturating_add(rewrite.removed_operand_loads);
        }
        Ok(removed_operand_loads)
    }
}

impl CheckedIntegerFoldSelection {
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

fn transform(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    let plan = CheckedIntegerFoldPlan::prepare(
        capability.verified().program(),
        CheckedIntegerFoldSelection::All,
    )
    .map_err(MirPassFailure::Rewrite)?;
    if plan.is_empty() {
        return capability.unchanged_with(pass_data(&plan, 0, 0));
    }

    let changed_callables = plan.changed_callable_count();
    let mut removed_protocol_load_values = 0usize;
    let rewritten = capability.rewrite(|callable, edit| {
        removed_protocol_load_values =
            removed_protocol_load_values.saturating_add(plan.rewrite_callable(callable, edit)?);
        Ok(())
    })?;
    rewritten.finish(pass_data(
        &plan,
        changed_callables,
        removed_protocol_load_values,
    ))
}

fn pass_data(
    plan: &CheckedIntegerFoldPlan,
    changed_callables: usize,
    removed_protocol_load_values: usize,
) -> MirPassData {
    let data = if changed_callables == 0 {
        MirPassData::processed(plan.processed_callables)
    } else {
        MirPassData::changed(changed_callables)
    };
    data.with_measurement(MirPassMeasurement::count(
        FOLDED_QUOTIENTS,
        count(plan.counts.quotients),
    ))
    .with_measurement(MirPassMeasurement::count(
        FOLDED_REMAINDERS,
        count(plan.counts.remainders),
    ))
    .with_measurement(MirPassMeasurement::count(
        FOLDED_SHIFTS,
        count(plan.counts.shifts),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_PROTOCOL_LOAD_VALUES,
        count(removed_protocol_load_values),
    ))
    .with_measurement(MirPassMeasurement::count(
        RETAINED_STATIC_FAILURES,
        count(plan.counts.retained_static_failures),
    ))
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "checked_integer_folding/tests.rs"]
mod tests;
