//! Narrow compound edit authority for normalized final-storage cleanup.
//!
//! Final optimization implementations can prepare and submit a certified
//! cleanup plan, but never receive raw storage, value, or instruction edits.
//! The existing CFG-only capability remains a separate concrete surface.

mod plan;

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    mir::{
        rewrite::{MirCallableEdit, MirRewriteError},
        BlockId, MirInstruction, MirPlace, MirRvalueKind, MirStorageKind, MirType, StorageId,
        ValueId,
    },
    passes::redundancy::{DeadPathActivationCandidate, DeadPathActivationInstructionKind},
};

pub(in crate::passes::pipeline) use plan::MirFinalStorageCleanupPlan;

/// Exact entity counts authorized by one cleanup plan.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::passes::pipeline) struct MirFinalStorageCleanupSummary {
    storages: usize,
    values: usize,
    instructions: usize,
}

impl MirFinalStorageCleanupSummary {
    pub(in crate::passes::pipeline) const fn storages(self) -> usize {
        self.storages
    }

    pub(in crate::passes::pipeline) const fn values(self) -> usize {
        self.values
    }

    pub(in crate::passes::pipeline) const fn instructions(self) -> usize {
        self.instructions
    }

    fn accumulate_candidate(&mut self, candidate: &DeadPathActivationCandidate) {
        self.storages = self.storages.saturating_add(1);
        self.values = self.values.saturating_add(candidate.load_results().len());
        self.instructions = self
            .instructions
            .saturating_add(candidate.instructions().len());
    }
}

/// Private compound edit surface for one certified callable plan.
struct MirFinalStorageCleanupEdit<'edit> {
    edit: &'edit mut MirCallableEdit,
}

impl<'edit> MirFinalStorageCleanupEdit<'edit> {
    fn new(edit: &'edit mut MirCallableEdit) -> Self {
        Self { edit }
    }

    fn remove_dead_path_activations(
        &mut self,
        candidates: &[DeadPathActivationCandidate],
    ) -> Result<MirFinalStorageCleanupSummary, MirRewriteError> {
        let removals = validate_candidates(self.edit, candidates)?;

        for (block, sites) in &removals.instructions {
            self.edit
                .rewrite_block_instructions(*block, |instructions| {
                    instructions
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| !sites.contains(index))
                        .map(|(_, instruction)| instruction.clone())
                        .collect()
                })?;
        }
        for value in &removals.values {
            self.edit.remove_value(*value)?;
        }
        for storage in &removals.storages {
            self.edit.remove_storage(*storage)?;
        }

        Ok(removals.summary)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CertifiedRemovals {
    storages: BTreeSet<StorageId>,
    values: BTreeSet<ValueId>,
    instructions: BTreeMap<BlockId, BTreeSet<usize>>,
    summary: MirFinalStorageCleanupSummary,
}

fn validate_candidates(
    edit: &MirCallableEdit,
    candidates: &[DeadPathActivationCandidate],
) -> Result<CertifiedRemovals, MirRewriteError> {
    let mut removals = CertifiedRemovals::default();
    let mut loaded_values = BTreeSet::new();

    for candidate in candidates {
        let storage = candidate.storage();
        if !removals.storages.insert(storage) {
            return Err(stale(edit, "duplicate final storage-cleanup carrier"));
        }
        let declaration = edit.storage(storage)?;
        if declaration.kind != MirStorageKind::NormalizedPathActivation {
            return Err(MirRewriteError::StorageKindMismatch {
                storage,
                expected: MirStorageKind::NormalizedPathActivation,
                actual: declaration.kind,
            });
        }
        if declaration != candidate.declaration()
            || candidate.declaration_index() != storage.index()
            || declaration.source.is_some()
            || declaration.ty != MirType::Bool
        {
            return Err(stale(edit, "normalized activation declaration"));
        }

        let candidate_values = candidate
            .load_results()
            .iter()
            .map(|value| value.id)
            .collect::<BTreeSet<_>>();
        if candidate_values.len() != candidate.load_results().len() {
            return Err(stale(edit, "duplicate activation load result"));
        }
        for expected in candidate.load_results() {
            if !removals.values.insert(expected.id)
                || expected.ty != MirType::Bool
                || edit.value(expected.id)? != expected
            {
                return Err(stale(edit, "normalized activation load result"));
            }
        }

        for site in candidate.instructions() {
            let instruction = edit
                .block(site.block())?
                .instructions
                .get(site.instruction())
                .ok_or_else(|| stale(edit, "normalized activation instruction position"))?;
            if instruction != site.expected()
                || !removals
                    .instructions
                    .entry(site.block())
                    .or_default()
                    .insert(site.instruction())
            {
                return Err(stale(edit, "normalized activation instruction"));
            }
            validate_instruction(
                edit,
                storage,
                &candidate_values,
                site.kind(),
                instruction,
                &mut loaded_values,
            )?;
        }
        removals.summary.accumulate_candidate(candidate);
    }

    if loaded_values != removals.values {
        return Err(stale(edit, "complete normalized activation load protocol"));
    }
    Ok(removals)
}

fn validate_instruction(
    edit: &MirCallableEdit,
    storage: StorageId,
    candidate_values: &BTreeSet<ValueId>,
    kind: DeadPathActivationInstructionKind,
    instruction: &MirInstruction,
    loaded_values: &mut BTreeSet<ValueId>,
) -> Result<(), MirRewriteError> {
    let valid = match (kind, instruction) {
        (DeadPathActivationInstructionKind::Load, MirInstruction::Assign(assignment)) => {
            let is_load = matches!(
                &assignment.rvalue.kind,
                MirRvalueKind::Load(place) if place == &MirPlace::base(storage)
            );
            is_load
                && candidate_values.contains(&assignment.result)
                && loaded_values.insert(assignment.result)
        }
        (DeadPathActivationInstructionKind::Store, MirInstruction::Store(store)) => {
            store.destination == MirPlace::base(storage)
                && store.authorization.is_none()
                && store.final_authorization.is_none()
        }
        (DeadPathActivationInstructionKind::LifetimeLive, MirInstruction::StorageLive(event)) => {
            event.storage == storage
        }
        (DeadPathActivationInstructionKind::LifetimeDead, MirInstruction::StorageDead(event)) => {
            event.storage == storage
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(stale(edit, "normalized activation instruction role"))
    }
}

/// Expected complete post-edit state used as a fail-closed capability guard.
struct MirFinalStorageCleanupInvariant {
    expected: MirCallableEdit,
}

impl MirFinalStorageCleanupInvariant {
    fn capture(
        edit: &MirCallableEdit,
        candidates: &[DeadPathActivationCandidate],
    ) -> Result<Self, MirRewriteError> {
        let mut expected = edit.clone();
        MirFinalStorageCleanupEdit::new(&mut expected).remove_dead_path_activations(candidates)?;
        Ok(Self { expected })
    }

    fn verify(self, edit: &MirCallableEdit) -> Result<(), MirRewriteError> {
        if self.expected == *edit {
            Ok(())
        } else {
            Err(MirRewriteError::UnsupportedFinalStorageCleanupMutation {
                callable: edit.callable(),
            })
        }
    }
}

fn stale(edit: &MirCallableEdit, subject: &'static str) -> MirRewriteError {
    MirRewriteError::StaleCallableSnapshot {
        callable: edit.callable(),
        subject,
    }
}

#[cfg(test)]
mod tests;
