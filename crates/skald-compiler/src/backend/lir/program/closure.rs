//! Exact typed closure over verified receipts and data relocations.

use super::{inventory::WorkEntry, DataDefinition, DataInitializer, ProgramError};
use crate::backend::plan::{ArtifactId, ArtifactPolicy, DataKey, LirCallableId, PlanView};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn check(
    parent: PlanView<'_>,
    callables: &BTreeMap<LirCallableId, WorkEntry<'_>>,
    data: &BTreeMap<DataKey, DataDefinition>,
) -> Result<(), ProgramError> {
    // Small schema fixtures predate resource ownership. Production plans always
    // carry typed roots; absence therefore means there is no closure contract.
    if parent.resources().complete_roots.is_empty() && parent.resources().reachable_roots.is_empty()
    {
        return Ok(());
    }

    let roots = match parent.artifact_policy() {
        ArtifactPolicy::Complete => &parent.resources().complete_roots,
        ArtifactPolicy::Reachable => &parent.resources().reachable_roots,
    };
    let mut pending = roots
        .iter()
        .map(|root| root.artifact)
        .chain(
            parent
                .resources()
                .data
                .iter()
                .map(|fact| ArtifactId::Data(fact.key)),
        )
        .collect::<Vec<_>>();
    let mut reached = BTreeSet::new();
    while let Some(artifact) = pending.pop() {
        if !reached.insert(artifact) {
            continue;
        }
        super::data::parent_artifact(parent, artifact, artifact.category())?;
        match artifact {
            ArtifactId::Callable(key) => {
                let Some(WorkEntry::Verified(receipt)) = callables.get(&key) else {
                    return Err(ProgramError::MissingDefinition(artifact));
                };
                pending.extend(receipt.references().iter().copied());
            }
            ArtifactId::Data(key) => {
                let definition = data
                    .get(&key)
                    .ok_or(ProgramError::MissingDefinition(artifact))?;
                pending.extend(definition.initializers.iter().filter_map(|initializer| {
                    match initializer {
                        DataInitializer::Address { target, .. } => Some(*target),
                        DataInitializer::Bytes(_) | DataInitializer::Zero(_) => None,
                    }
                }));
            }
            ArtifactId::Runtime(_) | ArtifactId::External(_) | ArtifactId::TraceTls => {}
        }
    }

    let materialized = callables
        .iter()
        .filter_map(|(key, state)| {
            matches!(state, WorkEntry::Verified(_)).then_some(ArtifactId::Callable(*key))
        })
        .chain(data.keys().map(|key| ArtifactId::Data(*key)));
    for artifact in materialized {
        if !reached.contains(&artifact) {
            return Err(ProgramError::UnexpectedDefinition(artifact));
        }
    }
    Ok(())
}
