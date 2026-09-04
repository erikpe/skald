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
        MirProgram,
    },
};

use super::{
    checked_integer_protocol::{
        observe_checked_integer_protocols, CheckedIntegerProtocolCandidate,
        CheckedIntegerProtocolCheck, CheckedIntegerProtocolObservation,
    },
    checked_integer_rewrite::rewrite_checked_integer_protocol,
};

/// Checked-operation families selected while preparing one fold plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CheckedIntegerFoldSelection {
    DivisionAndRemainder,
    Shift,
    All,
}

impl CheckedIntegerFoldSelection {
    const fn contains(self, check: CheckedIntegerProtocolCheck) -> bool {
        matches!(
            (self, check),
            (
                Self::DivisionAndRemainder,
                CheckedIntegerProtocolCheck::Division(_)
            ) | (Self::Shift, CheckedIntegerProtocolCheck::Shift(_))
                | (Self::All, _)
        )
    }
}

/// Immutable seal-local candidates grouped in deterministic callable order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CheckedIntegerFoldPlan {
    candidates: BTreeMap<CallableId, Vec<CheckedIntegerProtocolCandidate>>,
}

impl CheckedIntegerFoldPlan {
    /// Observes one operation family without retaining general MIR facts.
    pub(super) fn prepare(
        program: &MirProgram,
        selection: CheckedIntegerFoldSelection,
    ) -> Result<Self, MirRewriteError> {
        let mut candidates = BTreeMap::<_, Vec<_>>::new();
        for definition in program.executable_definitions() {
            for observation in observe_checked_integer_protocols(definition)? {
                let CheckedIntegerProtocolObservation::Candidate(candidate) = observation else {
                    continue;
                };
                if selection.contains(candidate.check) {
                    candidates
                        .entry(definition.callable())
                        .or_default()
                        .push(*candidate);
                }
            }
        }
        Ok(Self { candidates })
    }

    pub(super) fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

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

#[cfg(test)]
#[path = "checked_integer_folding/tests.rs"]
mod tests;
