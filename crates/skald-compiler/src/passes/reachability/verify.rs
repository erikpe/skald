//! Reachability-owned completeness verification for sparse final MIR.

use std::fmt;

use crate::mir::{MirExecutionNode, MirProgram, MirVerificationError, MirVerificationErrors};

use super::{MirDependencyEdgeKind, MirReachabilityAnalysis, MirReachabilityRootReason};

/// Requires a retained body only when independently computed reachability
/// proves that the callable can execute in this exact final-MIR product.
pub(in crate::passes) fn verify_reachable_definitions(
    program: &MirProgram,
    analysis: &MirReachabilityAnalysis,
) -> Result<(), MirVerificationErrors> {
    let errors = analysis
        .reachable_callables()
        .iter()
        .copied()
        .filter(|callable| !program.has_executable_definition(*callable))
        .map(|callable| {
            let explanation = analysis
                .explanation(MirExecutionNode::callable(callable))
                .expect("every reachable callable has a canonical explanation");
            let category = explanation.dependencies().last().map_or_else(
                || MirReachableDefinitionCategory::Root(explanation.root().reason()),
                |dependency| MirReachableDefinitionCategory::Dependency(dependency.kind()),
            );
            MirVerificationError {
                callable: Some(callable),
                block: None,
                message: format!(
                    "reachable callable has no retained definition; selected by dependency category `{category}`"
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MirReachableDefinitionCategory {
    Root(MirReachabilityRootReason),
    Dependency(MirDependencyEdgeKind),
}

impl fmt::Display for MirReachableDefinitionCategory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Root(MirReachabilityRootReason::Entry) => "entry-root",
            Self::Root(MirReachabilityRootReason::StaticActivation(_)) => "static-activation-root",
            Self::Root(MirReachabilityRootReason::StaticShutdown(_)) => "static-shutdown-root",
            Self::Dependency(MirDependencyEdgeKind::DirectCall) => "direct-call",
            Self::Dependency(MirDependencyEdgeKind::StaticCall) => "static-call",
            Self::Dependency(MirDependencyEdgeKind::DirectMethodCall) => "direct-method-call",
            Self::Dependency(MirDependencyEdgeKind::VirtualDispatch) => "virtual-dispatch",
            Self::Dependency(MirDependencyEdgeKind::InterfaceDispatch) => "interface-dispatch",
            Self::Dependency(MirDependencyEdgeKind::CallableAddressRetention) => {
                "callable-address-retention"
            }
            Self::Dependency(MirDependencyEdgeKind::IndirectCall) => "indirect-call",
            Self::Dependency(MirDependencyEdgeKind::Initializer) => "initializer",
            Self::Dependency(MirDependencyEdgeKind::CopyConstructor) => "copy-constructor",
            Self::Dependency(MirDependencyEdgeKind::CopyAssignment) => "copy-assignment",
            Self::Dependency(MirDependencyEdgeKind::UserCopyBody) => "user-copy-body",
            Self::Dependency(MirDependencyEdgeKind::BaseCopy) => "base-copy",
            Self::Dependency(MirDependencyEdgeKind::FieldCopy) => "field-copy",
            Self::Dependency(MirDependencyEdgeKind::CompleteFinalizer) => "complete-finalizer",
            Self::Dependency(MirDependencyEdgeKind::UserDestructor) => "user-destructor",
            Self::Dependency(MirDependencyEdgeKind::FieldFinalizer) => "field-finalizer",
            Self::Dependency(MirDependencyEdgeKind::BaseFinalizer) => "base-finalizer",
            Self::Dependency(MirDependencyEdgeKind::SharedFinalizer) => "shared-finalizer",
            Self::Dependency(MirDependencyEdgeKind::TemporaryCleanup) => "temporary-cleanup",
            Self::Dependency(MirDependencyEdgeKind::OptionalLifecycle) => "optional-lifecycle",
            Self::Dependency(MirDependencyEdgeKind::ArrayDefault) => "array-default",
            Self::Dependency(MirDependencyEdgeKind::ArrayCopy) => "array-copy",
            Self::Dependency(MirDependencyEdgeKind::ArrayAssignment) => "array-assignment",
            Self::Dependency(MirDependencyEdgeKind::ArrayDestruction) => "array-destruction",
            Self::Dependency(MirDependencyEdgeKind::RuntimeEntityReference) => {
                "runtime-entity-reference"
            }
        })
    }
}

#[cfg(test)]
mod tests;
