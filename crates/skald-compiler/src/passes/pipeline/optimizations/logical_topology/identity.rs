//! Reference validation for logical topology records.

use crate::mir::{
    rewrite::{MirLocalIdentity, MirLocalIdentitySite, MirReferenceFailure, MirRewriteError},
    MirDefinitionRef, MirLogicalExpression, MirPathCondition,
};

pub(super) fn validate_references(
    definition: MirDefinitionRef<'_>,
    logical: &MirLogicalExpression,
    condition: &MirPathCondition,
    record_index: usize,
) -> Result<(), MirRewriteError> {
    for block in [
        logical.split,
        logical.selection,
        logical.right_entry,
        logical.right_exit,
        logical.short,
        logical.join,
        condition.active_predecessor,
        condition.inactive_predecessor,
        condition.merge,
    ] {
        if definition.block(block).is_none() {
            return Err(invalid_reference(
                definition,
                MirLocalIdentity::Block(block),
                record_index,
            ));
        }
    }
    for storage in [logical.result, condition.activation] {
        if definition.storage(storage).is_none() {
            return Err(invalid_reference(
                definition,
                MirLocalIdentity::Storage(storage),
                record_index,
            ));
        }
    }
    for value in [
        logical.left_result,
        logical.right_result,
        logical.selected_result,
    ] {
        if definition.value(value).is_none() {
            return Err(invalid_reference(
                definition,
                MirLocalIdentity::Value(value),
                record_index,
            ));
        }
    }
    if let Some(parent) = condition.parent {
        if definition.path_condition(parent).is_none() {
            return Err(invalid_reference(
                definition,
                MirLocalIdentity::PathCondition(parent),
                record_index,
            ));
        }
    }
    Ok(())
}

pub(super) fn invalid_reference(
    definition: MirDefinitionRef<'_>,
    identity: MirLocalIdentity,
    record_index: usize,
) -> MirRewriteError {
    let failure = if identity.callable() == definition.callable() {
        MirReferenceFailure::Unknown
    } else {
        MirReferenceFailure::Foreign
    };
    MirRewriteError::InvalidReference {
        expected: definition.callable(),
        identity,
        site: MirLocalIdentitySite::LogicalExpression(record_index),
        failure,
    }
}
