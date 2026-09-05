use std::collections::{BTreeMap, BTreeSet};

use crate::{
    identity::CallableId,
    mir::{
        classify_local_identity_site,
        rewrite::{local_cfg_facts_for_definition, MirCallableEdit, MirRewriteError},
        BlockId, MirDefinitionRef, MirIdentitySiteRole, MirInstruction, MirPlace, MirProgram,
        MirRvalueKind, MirStorageKind, MirType, PathConditionId, StorageId,
    },
};

use super::{
    MirProofNormalizationError, MirProofNormalizationErrorKind, MirProofNormalizationStatistics,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct PathReadRewrite {
    block: BlockId,
    instruction: usize,
    expected: MirInstruction,
    replacement: MirInstruction,
}

/// Complete immutable plan for one callable. No MIR is changed while plans
/// are collected, so a malformed later callable cannot expose earlier edits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CallableNormalizationPlan {
    callable: CallableId,
    path_conditions: Vec<PathConditionId>,
    activation_storage: Vec<StorageId>,
    path_reads: Vec<PathReadRewrite>,
    logical_records: usize,
    released_proof_blocks: usize,
}

impl CallableNormalizationPlan {
    pub(super) const fn callable(&self) -> CallableId {
        self.callable
    }

    pub(super) fn statistics(&self) -> MirProofNormalizationStatistics {
        MirProofNormalizationStatistics::new(
            self.path_conditions.len(),
            self.logical_records,
            self.path_reads.len(),
            self.activation_storage.len(),
            usize::from(self.has_changes()),
            self.released_proof_blocks,
        )
    }

    pub(super) fn apply(self, edit: &mut MirCallableEdit) -> Result<(), MirRewriteError> {
        if edit.callable() != self.callable {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: edit.callable(),
                subject: "proof-normalization callable",
            });
        }

        let current_paths = edit.path_condition_ids().collect::<Vec<_>>();
        if current_paths != self.path_conditions {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: self.callable,
                subject: "path-condition inventory",
            });
        }
        if edit.logical_order().len() != self.logical_records {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: self.callable,
                subject: "logical-expression inventory",
            });
        }

        for storage in self.activation_storage {
            edit.replace_storage_kind(
                storage,
                MirStorageKind::PathCondition,
                MirStorageKind::NormalizedPathActivation,
            )?;
        }
        for rewrite in self.path_reads {
            edit.replace_instruction(
                rewrite.block,
                rewrite.instruction,
                &rewrite.expected,
                rewrite.replacement,
            )?;
        }

        let logical_records = edit.logical_order().to_vec();
        for record in logical_records {
            edit.remove_logical_record(record)?;
        }
        for condition in self.path_conditions {
            edit.remove_path_condition(condition)?;
        }
        Ok(())
    }

    fn has_changes(&self) -> bool {
        !self.path_conditions.is_empty()
            || !self.activation_storage.is_empty()
            || !self.path_reads.is_empty()
            || self.logical_records != 0
    }
}

pub(super) fn inventory_program(
    program: &MirProgram,
) -> Result<Vec<CallableNormalizationPlan>, MirProofNormalizationError> {
    program
        .executable_definitions()
        .map(inventory_definition)
        .collect()
}

fn inventory_definition(
    definition: MirDefinitionRef<'_>,
) -> Result<CallableNormalizationPlan, MirProofNormalizationError> {
    let callable = definition.callable();
    let mut conditions = BTreeMap::new();
    let mut activation_storage = BTreeSet::new();

    if let Some(storage) = definition
        .storage_entries()
        .iter()
        .find(|storage| storage.kind.is_normalized_path_activation())
    {
        return Err(
            MirProofNormalizationErrorKind::UnexpectedNormalizedActivationStorage {
                callable,
                storage: storage.id,
            }
            .into(),
        );
    }

    for (index, condition) in definition.path_conditions().iter().enumerate() {
        let expected = PathConditionId::new(callable, index);
        if condition.id != expected {
            return Err(
                MirProofNormalizationErrorKind::InvalidPathConditionIdentity {
                    callable,
                    index,
                    actual: condition.id,
                }
                .into(),
            );
        }
        if condition.activation.callable() != callable {
            return Err(MirProofNormalizationErrorKind::ForeignActivationStorage {
                callable,
                condition: condition.id,
                storage: condition.activation,
            }
            .into());
        }
        let Some(storage) = definition.storage(condition.activation) else {
            return Err(MirProofNormalizationErrorKind::UnknownActivationStorage {
                callable,
                condition: condition.id,
                storage: condition.activation,
            }
            .into());
        };
        if storage.kind != MirStorageKind::PathCondition || storage.ty != MirType::Bool {
            return Err(MirProofNormalizationErrorKind::InvalidActivationStorage {
                callable,
                condition: condition.id,
                storage: condition.activation,
                kind: storage.kind,
                ty: storage.ty,
            }
            .into());
        }
        if !activation_storage.insert(condition.activation) {
            return Err(MirProofNormalizationErrorKind::DuplicateActivationStorage {
                callable,
                storage: condition.activation,
            }
            .into());
        }
        conditions.insert(condition.id, condition.activation);
    }

    for storage in definition
        .storage_entries()
        .iter()
        .filter(|storage| storage.kind == MirStorageKind::PathCondition)
    {
        if !activation_storage.contains(&storage.id) {
            return Err(MirProofNormalizationErrorKind::OrphanPathConditionStorage {
                callable,
                storage: storage.id,
            }
            .into());
        }
    }

    let mut path_reads = Vec::new();
    for block in &definition.body().blocks {
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            let MirInstruction::Assign(assignment) = instruction else {
                continue;
            };
            let MirRvalueKind::PathCondition(read) = assignment.rvalue.kind else {
                continue;
            };
            if read.condition.callable() != callable {
                return Err(MirProofNormalizationErrorKind::ForeignPathReadCondition {
                    callable,
                    block: block.id,
                    instruction: instruction_index,
                    condition: read.condition,
                }
                .into());
            }
            let Some(expected_activation) = conditions.get(&read.condition).copied() else {
                return Err(MirProofNormalizationErrorKind::UnknownPathReadCondition {
                    callable,
                    block: block.id,
                    instruction: instruction_index,
                    condition: read.condition,
                }
                .into());
            };
            if read.activation != expected_activation {
                return Err(MirProofNormalizationErrorKind::PathReadActivationMismatch {
                    callable,
                    block: block.id,
                    instruction: instruction_index,
                    condition: read.condition,
                    expected: expected_activation,
                    actual: read.activation,
                }
                .into());
            }

            let mut replacement = instruction.clone();
            let MirInstruction::Assign(replacement_assignment) = &mut replacement else {
                unreachable!("the cloned instruction preserves its assignment variant")
            };
            replacement_assignment.rvalue.kind =
                MirRvalueKind::Load(MirPlace::base(read.activation));
            path_reads.push(PathReadRewrite {
                block: block.id,
                instruction: instruction_index,
                expected: instruction.clone(),
                replacement,
            });
        }
    }

    Ok(CallableNormalizationPlan {
        callable,
        path_conditions: conditions.keys().copied().collect(),
        activation_storage: activation_storage.into_iter().collect(),
        path_reads,
        logical_records: definition.logical_expressions().len(),
        released_proof_blocks: released_proof_blocks(definition)?,
    })
}

fn released_proof_blocks(definition: MirDefinitionRef<'_>) -> Result<usize, MirRewriteError> {
    let cfg = local_cfg_facts_for_definition(definition)?;
    let mut permanent = BTreeSet::from([cfg.entry()]);
    let mut proof = BTreeSet::new();
    for root in cfg.protected_roots() {
        match classify_local_identity_site(root.site()) {
            MirIdentitySiteRole::BodyEntry => {
                permanent.insert(root.block());
            }
            MirIdentitySiteRole::PermanentAttachment => {
                permanent.insert(root.block());
            }
            MirIdentitySiteRole::ConsumableProof => {
                proof.insert(root.block());
            }
            MirIdentitySiteRole::Ordinary => {}
        }
    }
    Ok(proof.difference(&permanent).count())
}
