//! Atomic ownership transfer and rewriting across every executable definition.

use crate::identity::CallableId;

use super::super::{MirFunctionDefinitionTable, MirMemberDefinitionTable, MirProgram};
use super::{
    callable::{MirCallablePackage, MirCommittedDefinition},
    commit::{MirCommitMaps, MirRewriteChangeSummary},
    edit::MirCallableEdit,
    error::MirRewriteError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MirCallableRewriteResult {
    pub(super) callable: CallableId,
    pub(super) maps: MirCommitMaps,
    pub(super) changes: MirRewriteChangeSummary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MirProgramRewriteResult {
    pub(super) program: MirProgram,
    pub(super) callables: Vec<MirCallableRewriteResult>,
}

/// Consumes a program, applies one callback in deterministic executable-body
/// order, and publishes rebuilt containers only after every body commits.
pub(super) fn rewrite_program(
    mut program: MirProgram,
    mut rewrite: impl FnMut(CallableId, &mut MirCallableEdit) -> Result<(), MirRewriteError>,
) -> Result<MirProgramRewriteResult, MirRewriteError> {
    let function_slots = std::mem::take(&mut program.definitions).into_rewrite_slots();
    let members = std::mem::take(&mut program.member_definitions).into_rewrite_entries();
    let initializers = program
        .static_lifecycle
        .as_mut()
        .map_or_else(Vec::new, |coordinator| {
            coordinator.take_initializers_for_rewrite()
        });

    let mut reports = Vec::new();
    let mut rewritten_functions = Vec::with_capacity(function_slots.len());
    for definition in function_slots {
        let definition = match definition {
            Some(definition) => {
                let committed = rewrite_package(
                    MirCallablePackage::from_function(definition)?,
                    &mut rewrite,
                    &mut reports,
                )?;
                let MirCommittedDefinition::Function(definition) = committed else {
                    unreachable!("function package changed executable-definition kind")
                };
                Some(definition)
            }
            None => None,
        };
        rewritten_functions.push(definition);
    }

    let mut rewritten_members = Vec::with_capacity(members.len());
    for definition in members {
        let committed = rewrite_package(
            MirCallablePackage::from_member(definition)?,
            &mut rewrite,
            &mut reports,
        )?;
        let MirCommittedDefinition::Member(definition) = committed else {
            unreachable!("member package changed executable-definition kind")
        };
        rewritten_members.push(definition);
    }

    let mut rewritten_initializers = Vec::with_capacity(initializers.len());
    for definition in initializers {
        let committed = rewrite_package(
            MirCallablePackage::from_static_initializer(definition)?,
            &mut rewrite,
            &mut reports,
        )?;
        let MirCommittedDefinition::StaticInitializer(definition) = committed else {
            unreachable!("static-initializer package changed executable-definition kind")
        };
        rewritten_initializers.push(definition);
    }

    program.definitions = MirFunctionDefinitionTable::new(rewritten_functions);
    program.member_definitions = MirMemberDefinitionTable::new(rewritten_members);
    if let Some(coordinator) = &mut program.static_lifecycle {
        coordinator.restore_initializers_after_rewrite(rewritten_initializers);
    } else {
        debug_assert!(rewritten_initializers.is_empty());
    }
    Ok(MirProgramRewriteResult {
        program,
        callables: reports,
    })
}

fn rewrite_package(
    mut package: MirCallablePackage,
    rewrite: &mut impl FnMut(CallableId, &mut MirCallableEdit) -> Result<(), MirRewriteError>,
    reports: &mut Vec<MirCallableRewriteResult>,
) -> Result<MirCommittedDefinition, MirRewriteError> {
    let callable = package.callable();
    rewrite(callable, package.edit_mut())?;
    let committed = package.commit()?;
    reports.push(MirCallableRewriteResult {
        callable,
        maps: committed.maps,
        changes: committed.changes,
    });
    Ok(committed.definition)
}

#[cfg(test)]
mod tests;
