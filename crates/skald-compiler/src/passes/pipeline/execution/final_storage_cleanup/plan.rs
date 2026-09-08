//! Immutable exact plans for normalized path-activation carrier deletion.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    identity::CallableId,
    mir::{
        rewrite::{MirCallableEdit, MirCallableEditSnapshot, MirRewriteError},
        MirProgram,
    },
    passes::{
        redundancy::{analyze_dead_normalized_path_activations, DeadPathActivationCandidate},
        VerifiedFinalMirProgram,
    },
};

use super::{
    MirFinalStorageCleanupEdit, MirFinalStorageCleanupInvariant, MirFinalStorageCleanupSummary,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallablePlan {
    snapshot: MirCallableEditSnapshot,
    candidates: Vec<DeadPathActivationCandidate>,
}

/// Complete carrier deletions certified against one verified final-MIR seal.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::passes::pipeline) struct MirFinalStorageCleanupPlan {
    callables: BTreeMap<CallableId, CallablePlan>,
    processed_callables: usize,
    summary: MirFinalStorageCleanupSummary,
    inspected_carriers: u64,
    removable_carriers: u64,
    protected_carriers: u64,
    maximum_protocol_size: u64,
}

impl MirFinalStorageCleanupPlan {
    pub(in crate::passes::pipeline) fn prepare(
        verified: &VerifiedFinalMirProgram,
    ) -> Result<Self, MirRewriteError> {
        let observation = analyze_dead_normalized_path_activations(verified);
        let counts = observation.counts();
        let selected = observation
            .callables()
            .iter()
            .filter(|callable| !callable.candidates().is_empty())
            .map(|callable| (callable.callable(), callable.candidates()))
            .collect::<BTreeMap<_, _>>();
        let mut plan = Self {
            inspected_carriers: counts.inspected(),
            removable_carriers: counts.proven(),
            protected_carriers: counts.blocked(),
            maximum_protocol_size: counts.maximum_protocol_size(),
            ..Self::default()
        };

        for definition in verified.program().executable_definitions() {
            plan.processed_callables = plan.processed_callables.saturating_add(1);
            let Some(candidates) = selected.get(&definition.callable()) else {
                continue;
            };
            let candidates = candidates.to_vec();
            for candidate in &candidates {
                plan.summary.accumulate_candidate(candidate);
            }
            plan.callables.insert(
                definition.callable(),
                CallablePlan {
                    snapshot: MirCallableEditSnapshot::capture(definition),
                    candidates,
                },
            );
        }
        if let Some(missing) = selected
            .keys()
            .find(|callable| !plan.callables.contains_key(callable))
        {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: *missing,
                subject: "final storage-cleanup callable",
            });
        }
        Ok(plan)
    }

    pub(in crate::passes::pipeline) fn validate_program(
        &self,
        program: &MirProgram,
    ) -> Result<(), MirRewriteError> {
        let mut validated = BTreeSet::new();
        for definition in program.executable_definitions() {
            if let Some(plan) = self.callables.get(&definition.callable()) {
                plan.snapshot
                    .validate_definition(definition, "final storage-cleanup plan")?;
                validated.insert(definition.callable());
            }
        }
        if let Some(missing) = self
            .callables
            .keys()
            .find(|callable| !validated.contains(callable))
        {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: *missing,
                subject: "final storage-cleanup callable",
            });
        }
        Ok(())
    }

    /// Validates the complete callable plan before applying its first edit.
    pub(in crate::passes::pipeline) fn rewrite_callable(
        &self,
        callable: CallableId,
        edit: &mut MirCallableEdit,
    ) -> Result<MirFinalStorageCleanupSummary, MirRewriteError> {
        let Some(plan) = self.callables.get(&callable) else {
            return Ok(MirFinalStorageCleanupSummary::default());
        };
        plan.snapshot.validate(edit, "final storage-cleanup plan")?;

        let invariant = MirFinalStorageCleanupInvariant::capture(edit, &plan.candidates)?;
        let summary =
            MirFinalStorageCleanupEdit::new(edit).remove_dead_path_activations(&plan.candidates)?;
        invariant.verify(edit)?;
        Ok(summary)
    }

    pub(in crate::passes::pipeline) const fn processed_callables(&self) -> usize {
        self.processed_callables
    }

    pub(in crate::passes::pipeline) fn changed_callables(&self) -> usize {
        self.callables.len()
    }

    pub(in crate::passes::pipeline) const fn summary(&self) -> MirFinalStorageCleanupSummary {
        self.summary
    }

    pub(in crate::passes::pipeline) const fn inspected_carriers(&self) -> u64 {
        self.inspected_carriers
    }

    pub(in crate::passes::pipeline) const fn removable_carriers(&self) -> u64 {
        self.removable_carriers
    }

    pub(in crate::passes::pipeline) const fn protected_carriers(&self) -> u64 {
        self.protected_carriers
    }

    pub(in crate::passes::pipeline) const fn maximum_protocol_size(&self) -> u64 {
        self.maximum_protocol_size
    }

    pub(in crate::passes::pipeline) fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }
}
