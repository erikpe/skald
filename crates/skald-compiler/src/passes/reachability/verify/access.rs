//! Reachable static-place validation against certified activation authority.

use crate::{
    identity::CallableId,
    mir::{MirExecutionNode, MirProgram, MirVerificationError, MirVerificationErrors},
};

use super::super::MirReachabilityAnalysis;

pub(in crate::passes) fn verify_reachable_static_accesses(
    program: &MirProgram,
    analysis: &MirReachabilityAnalysis,
) -> Result<(), MirVerificationErrors> {
    let activation = program
        .static_lifecycle
        .as_ref()
        .map(|coordinator| coordinator.lifecycle().proof().activation());
    let errors = analysis
        .static_accesses()
        .iter()
        .filter(|access| {
            !activation.is_some_and(|authority| authority.contains(access.target()))
        })
        .map(|access| {
            let explanation = analysis.static_access_explanation(access);
            let selection = explanation.map_or_else(
                || "without a canonical reachability explanation".to_owned(),
                |explanation| {
                    explanation.dependencies().last().map_or_else(
                        || format!("as {:?}", explanation.root().reason()),
                        |dependency| {
                            format!(
                                "from {:?} through {:?}",
                                explanation.root().reason(),
                                dependency.kind()
                            )
                        },
                    )
                },
            );
            MirVerificationError {
                callable: callable(access.source()),
                block: None,
                message: format!(
                    "reachable {:?} static access from {:?} targets inactive field {}; source selected {selection}",
                    access.kind(),
                    access.region(),
                    access.target(),
                ),
            }
        })
        .collect::<Vec<_>>();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(MirVerificationErrors::new(errors))
    }
}

const fn callable(node: MirExecutionNode) -> Option<CallableId> {
    match node {
        MirExecutionNode::Callable(callable) => Some(callable),
        MirExecutionNode::ClassLifecycle { .. } | MirExecutionNode::ArrayLifecycle { .. } => None,
    }
}
