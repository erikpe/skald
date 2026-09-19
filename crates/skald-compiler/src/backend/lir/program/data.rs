//! Explicit initializer bytes and relocatable addresses; never assembly parsing.
use super::inventory::ProgramError;
use crate::backend::plan::{
    ArtifactCategory, ArtifactId, ArtifactPolicy, DataInitializerFact, DataKey, PlanView,
    StaticStorageDisposition,
};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum DataInitializer {
    Bytes(Vec<u8>),
    Zero(usize),
    Address {
        target: ArtifactId,
        category: ArtifactCategory,
        addend: i64,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::backend) struct DataDefinition {
    pub key: DataKey,
    pub initializers: Vec<DataInitializer>,
}

pub(super) fn check(
    definition: &DataDefinition,
    parent: PlanView<'_>,
    lookup: impl Fn(ArtifactId, ArtifactCategory) -> Result<Option<usize>, ProgramError>,
) -> Result<BTreeSet<ArtifactId>, ProgramError> {
    if let Some(planned) = parent
        .resources()
        .data
        .iter()
        .find(|fact| fact.key == definition.key)
    {
        let exact = definition.initializers.len() == planned.initializers.len()
            && definition
                .initializers
                .iter()
                .zip(&planned.initializers)
                .all(|(actual, expected)| match (actual, expected) {
                    (DataInitializer::Bytes(actual), DataInitializerFact::Bytes(expected)) => {
                        actual == expected
                    }
                    (DataInitializer::Zero(actual), DataInitializerFact::Zero(expected)) => {
                        actual == expected
                    }
                    (
                        DataInitializer::Address {
                            target: actual_target,
                            category: actual_category,
                            addend: actual_addend,
                        },
                        DataInitializerFact::Address {
                            target: expected_target,
                            category: expected_category,
                            addend: expected_addend,
                        },
                    ) => {
                        actual_target == expected_target
                            && actual_category == expected_category
                            && actual_addend == expected_addend
                    }
                    _ => false,
                });
        if !exact {
            return Err(ProgramError::InvalidInitializer);
        }
    }
    let extent = lookup(ArtifactId::Data(definition.key), ArtifactCategory::Data)?
        .ok_or(ProgramError::InvalidInitializer)?;
    let mut bytes = 0usize;
    let mut references = BTreeSet::new();
    for initializer in &definition.initializers {
        let width = match initializer {
            DataInitializer::Bytes(value) => value.len(),
            DataInitializer::Zero(width) => *width,
            DataInitializer::Address {
                target,
                category,
                addend,
            } => {
                let target_extent = lookup(*target, *category)?;
                let valid = match target_extent {
                    Some(size) => usize::try_from(*addend).is_ok_and(|offset| offset <= size),
                    None => *addend == 0,
                };
                if !valid {
                    return Err(ProgramError::InvalidAddend);
                }
                references.insert(*target);
                parent.profile().data_layout.pointer_bytes
            }
        };
        bytes = bytes.checked_add(width).ok_or(ProgramError::SizeOverflow)?;
    }
    if bytes != extent {
        return Err(ProgramError::InvalidInitializer);
    }
    if let DataKey::FailureMessage(reason) = definition.key {
        let expected = reason.bytes();
        if bytes != expected.len() {
            return Err(ProgramError::InvalidInitializer);
        }
        let mut offset = 0;
        for initializer in &definition.initializers {
            let valid = match initializer {
                DataInitializer::Bytes(value) => {
                    let end = offset + value.len();
                    let valid = expected.get(offset..end) == Some(value.as_slice());
                    offset = end;
                    valid
                }
                DataInitializer::Zero(width) => {
                    let end = offset + width;
                    let valid = expected[offset..end].iter().all(|byte| *byte == 0);
                    offset = end;
                    valid
                }
                DataInitializer::Address { .. } => false,
            };
            if !valid {
                return Err(ProgramError::InvalidInitializer);
            }
        }
    }
    Ok(references)
}

pub(super) fn parent_artifact(
    parent: PlanView<'_>,
    key: ArtifactId,
    category: ArtifactCategory,
) -> Result<Option<usize>, ProgramError> {
    let declaration = parent.artifact(parent.artifact_id(key)?, category)?;
    if let ArtifactId::Callable(key) = key {
        parent.callable(key)?;
    }
    if let ArtifactId::Data(DataKey::Static(field)) = key {
        let permitted = match parent.static_storage_disposition(field) {
            Some(StaticStorageDisposition::Active) => true,
            Some(StaticStorageDisposition::RetainedInactive) => {
                parent.artifact_policy() == ArtifactPolicy::Complete
            }
            None => parent.is_active_static(field),
        };
        if !permitted {
            return Err(ProgramError::Plan(
                crate::backend::plan::PlanError::InvalidDomain,
            ));
        }
    }
    declaration
        .layout
        .map(|layout| {
            parent
                .layout(parent.layout_id(layout.index())?)
                .map(|l| l.size)
        })
        .transpose()
        .map_err(ProgramError::from)
}
