//! Exact source snapshots for validating multi-edit callable plans.

use crate::mir::{MirBody, MirDefinitionRef, MirStorage, MirValue};

use super::{LogicalRecordIndex, MirCallableEdit, MirRewriteError};

/// Dense editable callable state captured before a sparse transaction starts.
///
/// A multi-edit plan validates this complete snapshot once before its first
/// mutation. Individual edits may then depend on other candidates from the
/// same source snapshot without treating earlier unpublished edits as stale.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirCallableEditSnapshot {
    callable: crate::identity::CallableId,
    storage: Vec<MirStorage>,
    values: Vec<MirValue>,
    body: MirBody,
}

impl MirCallableEditSnapshot {
    pub(crate) fn capture(definition: MirDefinitionRef<'_>) -> Self {
        Self {
            callable: definition.callable(),
            storage: definition.storage_entries().to_vec(),
            values: definition.values().to_vec(),
            body: definition.body().clone(),
        }
    }

    /// Rejects any mutation, deletion, insertion, or reordering since capture.
    pub(crate) fn validate(
        &self,
        edit: &MirCallableEdit,
        subject: &'static str,
    ) -> Result<(), MirRewriteError> {
        if !self.matches(edit) {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: edit.callable(),
                subject,
            });
        }
        Ok(())
    }

    fn matches(&self, edit: &MirCallableEdit) -> bool {
        self.callable == edit.callable()
            && self.body.entry == edit.entry()
            && edit
                .storage_ids()
                .eq(self.storage.iter().map(|storage| storage.id))
            && self
                .storage
                .iter()
                .all(|storage| edit.storage(storage.id) == Ok(storage))
            && edit
                .value_ids()
                .eq(self.values.iter().map(|value| value.id))
            && self
                .values
                .iter()
                .all(|value| edit.value(value.id) == Ok(value))
            && edit.block_order()
                == self
                    .body
                    .blocks
                    .iter()
                    .map(|block| block.id)
                    .collect::<Vec<_>>()
            && self
                .body
                .blocks
                .iter()
                .all(|block| edit.block(block.id) == Ok(block))
            && edit.path_condition_ids().eq(self
                .body
                .path_conditions
                .iter()
                .map(|condition| condition.id))
            && self
                .body
                .path_conditions
                .iter()
                .all(|condition| edit.path_condition(condition.id) == Ok(condition))
            && edit.logical_order().len() == self.body.logical_expressions.len()
            && self
                .body
                .logical_expressions
                .iter()
                .enumerate()
                .all(|(index, expression)| {
                    edit.logical_order().get(index) == Some(&LogicalRecordIndex::new(index))
                        && edit.logical_record(LogicalRecordIndex::new(index)) == Ok(expression)
                })
    }
}
