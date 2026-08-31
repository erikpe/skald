//! Atomic stable-identity retention of final-MIR executable definitions.
//!
//! This facade prepares retention only from verified whole-world reachability
//! facts. It never accepts a caller-selected identity set or predicate as
//! production authority. Preparation validates every precondition while
//! borrowing MIR; only a successful changed plan may consume the program and
//! publish rebuilt definition containers.

mod apply;
mod error;
mod model;

use crate::{identity::CallableId, passes::reachability::MirReachabilityAnalysis};

use super::{MirExecutionNode, MirProgram};

pub(crate) use error::MirDefinitionRetentionError;
pub(crate) use model::{
    MirDefinitionKindCounts, MirDefinitionRetention, MirDefinitionRetentionChange,
    MirDefinitionRetentionSummary, MirPreparedDefinitionRetention,
};

/// Validates and prepares exact reachable-definition retention without
/// mutating or consuming `program`.
///
/// The analysis is the immutable product bound to the same verified final MIR
/// by the pipeline capability. Callers cannot substitute a retained-ID set.
pub(crate) fn prepare_reachable_definition_retention(
    program: &MirProgram,
    reachability: &MirReachabilityAnalysis,
) -> Result<MirDefinitionRetention, MirDefinitionRetentionError> {
    prepare(program, |callable| {
        reachability.is_reachable(MirExecutionNode::callable(callable))
    })
}

fn prepare(
    program: &MirProgram,
    mut is_reachable: impl FnMut(CallableId) -> bool,
) -> Result<MirDefinitionRetention, MirDefinitionRetentionError> {
    let mut examined = MirDefinitionKindCounts::default();
    let mut retained = MirDefinitionKindCounts::default();
    let mut removed = MirDefinitionKindCounts::default();
    let mut removed_callables = Vec::new();

    for definition in program.definitions.iter() {
        classify_definition(
            definition.callable(),
            &mut is_reachable,
            &mut examined,
            &mut retained,
            &mut removed,
            &mut removed_callables,
        );
    }
    for definition in program.member_definitions.iter() {
        classify_definition(
            definition.callable,
            &mut is_reachable,
            &mut examined,
            &mut retained,
            &mut removed,
            &mut removed_callables,
        );
    }
    if let Some(coordinator) = &program.static_lifecycle {
        for initializer in coordinator.initializers() {
            let callable = initializer.callable();
            examined.record(callable);
            if !is_reachable(callable) {
                return Err(MirDefinitionRetentionError::UnreachableStaticInitializer(
                    initializer.id,
                ));
            }
            retained.record(callable);
        }
    }

    let summary =
        MirDefinitionRetentionSummary::new(examined, retained, removed, removed_callables);
    if summary.removed().total() == 0 {
        Ok(MirDefinitionRetention::Unchanged(summary))
    } else {
        Ok(MirDefinitionRetention::Changed(
            MirPreparedDefinitionRetention::new(summary),
        ))
    }
}

fn classify_definition(
    callable: CallableId,
    is_reachable: &mut impl FnMut(CallableId) -> bool,
    examined: &mut MirDefinitionKindCounts,
    retained: &mut MirDefinitionKindCounts,
    removed: &mut MirDefinitionKindCounts,
    removed_callables: &mut Vec<CallableId>,
) {
    examined.record(callable);
    if is_reachable(callable) {
        retained.record(callable);
    } else {
        removed.record(callable);
        removed_callables.push(callable);
    }
}

#[cfg(test)]
mod tests;
