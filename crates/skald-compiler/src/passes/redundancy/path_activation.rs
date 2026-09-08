//! Exact read-only analysis of dead normalized path-activation protocols.

use std::collections::BTreeSet;

use crate::{
    mir::{
        rewrite::{
            storage_use_census_for_definition, value_use_site_index_for_definition,
            MirLocalIdentitySite, MirRewriteError, MirStoragePlaceUse, MirStorageUseRole,
            MirStorageWriteAuthorization, MirValueUseSiteIndex,
        },
        BlockId, MirDefinitionRef, MirInstruction, MirPlace, MirRvalueKind, MirStorageKind,
        MirType, StorageId,
    },
    passes::VerifiedFinalMirProgram,
};

use self::accumulator::Accumulator;
use super::path_activation_model::{
    BlockedPathActivation, DeadPathActivationBlocker, DeadPathActivationCallableObservation,
    DeadPathActivationCandidate, DeadPathActivationInstruction, DeadPathActivationInstructionKind,
    DeadPathActivationObservation,
};

mod accumulator;

/// Finds complete dead normalized path-activation protocols without cloning or
/// mutating the verified final-MIR product.
pub fn analyze_dead_normalized_path_activations(
    verified: &VerifiedFinalMirProgram,
) -> DeadPathActivationObservation {
    analyze_program(verified.program())
}

fn analyze_program(program: &crate::mir::MirProgram) -> DeadPathActivationObservation {
    let mut total = Accumulator::default();
    let mut callables = Vec::new();
    for definition in program.executable_definitions() {
        let callable = definition.callable();
        let observed = analyze_definition(definition)
            .expect("verified MIR must have coherent callable-local identities");
        if observed.accumulator.counts.inspected != 0 {
            total.merge(&observed.accumulator);
            let examples = observed.accumulator.examples.clone();
            callables.push(DeadPathActivationCallableObservation::new(
                callable,
                observed.accumulator.finish(1),
                observed.candidates,
                observed.blocked,
                examples,
            ));
        }
    }
    let examples = total.examples.clone();
    DeadPathActivationObservation::new(total.finish(callables.len() as u64), callables, examples)
}

#[cfg(test)]
pub(super) fn analyze_unverified_definition(
    definition: MirDefinitionRef<'_>,
) -> Result<DeadPathActivationCallableObservation, MirRewriteError> {
    let callable = definition.callable();
    let observed = analyze_definition(definition)?;
    let affected = u64::from(observed.accumulator.counts.inspected != 0);
    let examples = observed.accumulator.examples.clone();
    Ok(DeadPathActivationCallableObservation::new(
        callable,
        observed.accumulator.finish(affected),
        observed.candidates,
        observed.blocked,
        examples,
    ))
}

struct DefinitionObservation {
    accumulator: Accumulator,
    candidates: Vec<DeadPathActivationCandidate>,
    blocked: Vec<BlockedPathActivation>,
}

fn analyze_definition(
    definition: MirDefinitionRef<'_>,
) -> Result<DefinitionObservation, MirRewriteError> {
    let storage_uses = storage_use_census_for_definition(definition)?;
    let value_uses = value_use_site_index_for_definition(definition)?;
    let mut accumulator = Accumulator::default();
    let mut candidates = Vec::new();
    let mut blocked = Vec::new();

    for entry in storage_uses
        .iter()
        .filter(|entry| entry.kind() == MirStorageKind::NormalizedPathActivation)
    {
        accumulator.increment_inspected();
        let storage = entry.storage();
        let declaration = &definition.storage_entries()[storage.index()];
        let mut blockers = BTreeSet::new();
        let mut instructions = Vec::new();
        let mut load_results = Vec::new();

        if declaration.source.is_some() {
            blockers.insert(DeadPathActivationBlocker::SourceBinding);
        }
        if declaration.ty != MirType::Bool {
            blockers.insert(DeadPathActivationBlocker::NonBooleanStorage);
        }

        for use_site in entry.uses() {
            classify_use(
                definition,
                &value_uses,
                storage,
                use_site.site(),
                use_site.role(),
                &mut instructions,
                &mut load_results,
                &mut blockers,
            );
        }

        instructions.sort_by_key(|site| (site.block(), site.instruction(), site.kind()));
        instructions.dedup_by_key(|site| (site.block(), site.instruction()));
        load_results.sort_by_key(|value| value.id);
        load_results.dedup_by_key(|value| value.id);

        let location = instructions
            .first()
            .map(|site| (site.block(), site.instruction()))
            .or_else(|| first_instruction_location(definition, entry.uses()));
        if blockers.is_empty() {
            let candidate = DeadPathActivationCandidate::new(
                storage.index(),
                declaration.clone(),
                instructions,
                load_results,
            );
            accumulator.record_candidate(&candidate, location);
            candidates.push(candidate);
        } else {
            let blockers = blockers.into_iter().collect::<Vec<_>>();
            accumulator.record_blocked(storage, location, &blockers);
            blocked.push(BlockedPathActivation::new(storage, blockers));
        }
    }

    Ok(DefinitionObservation {
        accumulator,
        candidates,
        blocked,
    })
}

#[allow(clippy::too_many_arguments)]
fn classify_use(
    definition: MirDefinitionRef<'_>,
    value_uses: &MirValueUseSiteIndex,
    storage: StorageId,
    site: MirLocalIdentitySite,
    role: MirStorageUseRole,
    instructions: &mut Vec<DeadPathActivationInstruction>,
    load_results: &mut Vec<crate::mir::MirValue>,
    blockers: &mut BTreeSet<DeadPathActivationBlocker>,
) {
    match use_disposition(role) {
        UseDisposition::Lifetime(kind) => {
            retain_lifetime(definition, storage, site, kind, instructions, blockers)
        }
        UseDisposition::Load => retain_load(
            definition,
            value_uses,
            storage,
            site,
            instructions,
            load_results,
            blockers,
        ),
        UseDisposition::Store => retain_store(definition, storage, site, instructions, blockers),
        UseDisposition::Blocker(blocker) => {
            blockers.insert(blocker);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UseDisposition {
    Lifetime(DeadPathActivationInstructionKind),
    Load,
    Store,
    Blocker(DeadPathActivationBlocker),
}

const fn use_disposition(role: MirStorageUseRole) -> UseDisposition {
    match role {
        MirStorageUseRole::LifetimeLive => {
            UseDisposition::Lifetime(DeadPathActivationInstructionKind::LifetimeLive)
        }
        MirStorageUseRole::LifetimeDead => {
            UseDisposition::Lifetime(DeadPathActivationInstructionKind::LifetimeDead)
        }
        MirStorageUseRole::OrdinaryRead(MirStoragePlaceUse::ExactBase) => UseDisposition::Load,
        MirStorageUseRole::OrdinaryRead(_) => {
            UseDisposition::Blocker(DeadPathActivationBlocker::NoncanonicalReadPlace)
        }
        MirStorageUseRole::OrdinaryWrite {
            place: MirStoragePlaceUse::ExactBase,
            authorization: MirStorageWriteAuthorization::None,
        } => UseDisposition::Store,
        MirStorageUseRole::OrdinaryWrite {
            place: MirStoragePlaceUse::ExactBase,
            authorization: _,
        } => UseDisposition::Blocker(DeadPathActivationBlocker::AuthorizedWrite),
        MirStorageUseRole::OrdinaryWrite { .. } => {
            UseDisposition::Blocker(DeadPathActivationBlocker::NoncanonicalWritePlace)
        }
        MirStorageUseRole::Declaration => {
            UseDisposition::Blocker(DeadPathActivationBlocker::MalformedInstructionSite)
        }
        MirStorageUseRole::Attachment => {
            UseDisposition::Blocker(DeadPathActivationBlocker::Attachment)
        }
        MirStorageUseRole::CheckedProtocol => {
            UseDisposition::Blocker(DeadPathActivationBlocker::CheckedProtocol)
        }
        MirStorageUseRole::ProofMetadata => {
            UseDisposition::Blocker(DeadPathActivationBlocker::ProofMetadata)
        }
        MirStorageUseRole::Alias => {
            UseDisposition::Blocker(DeadPathActivationBlocker::AliasExposure)
        }
        MirStorageUseRole::Call => UseDisposition::Blocker(DeadPathActivationBlocker::Call),
        MirStorageUseRole::OwnershipOrLifecycle => {
            UseDisposition::Blocker(DeadPathActivationBlocker::OwnershipOrLifecycle)
        }
        MirStorageUseRole::InputOutput => {
            UseDisposition::Blocker(DeadPathActivationBlocker::InputOutput)
        }
        MirStorageUseRole::OtherExecutable => {
            UseDisposition::Blocker(DeadPathActivationBlocker::OtherExecutable)
        }
    }
}

fn retain_lifetime(
    definition: MirDefinitionRef<'_>,
    storage: StorageId,
    site: MirLocalIdentitySite,
    kind: DeadPathActivationInstructionKind,
    instructions: &mut Vec<DeadPathActivationInstruction>,
    blockers: &mut BTreeSet<DeadPathActivationBlocker>,
) {
    let Some((block, instruction_index, instruction)) = instruction_at(definition, site) else {
        blockers.insert(DeadPathActivationBlocker::MalformedInstructionSite);
        return;
    };
    let matches = match (kind, instruction) {
        (DeadPathActivationInstructionKind::LifetimeLive, MirInstruction::StorageLive(event)) => {
            event.storage == storage
        }
        (DeadPathActivationInstructionKind::LifetimeDead, MirInstruction::StorageDead(event)) => {
            event.storage == storage
        }
        _ => false,
    };
    if matches {
        instructions.push(DeadPathActivationInstruction::new(
            block,
            instruction_index,
            kind,
            instruction.clone(),
        ));
    } else {
        blockers.insert(DeadPathActivationBlocker::MalformedInstructionSite);
    }
}

fn retain_store(
    definition: MirDefinitionRef<'_>,
    storage: StorageId,
    site: MirLocalIdentitySite,
    instructions: &mut Vec<DeadPathActivationInstruction>,
    blockers: &mut BTreeSet<DeadPathActivationBlocker>,
) {
    let Some((block, instruction_index, instruction)) = instruction_at(definition, site) else {
        blockers.insert(DeadPathActivationBlocker::MalformedInstructionSite);
        return;
    };
    let matches = matches!(instruction, MirInstruction::Store(store)
        if store.destination == MirPlace::base(storage)
            && store.authorization.is_none()
            && store.final_authorization.is_none());
    if matches {
        instructions.push(DeadPathActivationInstruction::new(
            block,
            instruction_index,
            DeadPathActivationInstructionKind::Store,
            instruction.clone(),
        ));
    } else {
        blockers.insert(DeadPathActivationBlocker::MalformedInstructionSite);
    }
}

fn retain_load(
    definition: MirDefinitionRef<'_>,
    value_uses: &MirValueUseSiteIndex,
    storage: StorageId,
    site: MirLocalIdentitySite,
    instructions: &mut Vec<DeadPathActivationInstruction>,
    load_results: &mut Vec<crate::mir::MirValue>,
    blockers: &mut BTreeSet<DeadPathActivationBlocker>,
) {
    let Some((block, instruction_index, instruction)) = instruction_at(definition, site) else {
        blockers.insert(DeadPathActivationBlocker::MalformedInstructionSite);
        return;
    };
    let MirInstruction::Assign(assignment) = instruction else {
        blockers.insert(DeadPathActivationBlocker::MalformedInstructionSite);
        return;
    };
    if assignment.rvalue.ty != MirType::Bool
        || !matches!(&assignment.rvalue.kind, MirRvalueKind::Load(place) if place == &MirPlace::base(storage))
    {
        blockers.insert(DeadPathActivationBlocker::MalformedLoadResult);
        return;
    }
    let Some(declaration) = definition.values().get(assignment.result.index()) else {
        blockers.insert(DeadPathActivationBlocker::MalformedLoadResult);
        return;
    };
    let Some(uses) = value_uses.get(assignment.result) else {
        blockers.insert(DeadPathActivationBlocker::MalformedLoadResult);
        return;
    };
    if declaration.id != assignment.result
        || declaration.ty != MirType::Bool
        || uses.definition() != site
    {
        blockers.insert(DeadPathActivationBlocker::MalformedLoadResult);
        return;
    }
    if !uses.uses().is_empty() {
        blockers.insert(DeadPathActivationBlocker::MaterialLoadResult);
        return;
    }
    instructions.push(DeadPathActivationInstruction::new(
        block,
        instruction_index,
        DeadPathActivationInstructionKind::Load,
        instruction.clone(),
    ));
    load_results.push(declaration.clone());
}

fn instruction_at(
    definition: MirDefinitionRef<'_>,
    site: MirLocalIdentitySite,
) -> Option<(BlockId, usize, &MirInstruction)> {
    let MirLocalIdentitySite::Instruction {
        block: block_index,
        instruction,
    } = site
    else {
        return None;
    };
    let block = definition.body().blocks.get(block_index)?;
    let expected = BlockId::new(definition.callable(), block_index);
    (block.id == expected)
        .then(|| block.instructions.get(instruction))
        .flatten()
        .map(|value| (block.id, instruction, value))
}

fn first_instruction_location(
    definition: MirDefinitionRef<'_>,
    uses: &[crate::mir::rewrite::MirStorageUseSite],
) -> Option<(BlockId, usize)> {
    uses.iter()
        .find_map(|site| instruction_at(definition, site.site()))
        .map(|(block, instruction, _)| (block, instruction))
}

#[cfg(test)]
#[path = "path_activation/tests.rs"]
mod tests;
