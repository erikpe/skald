//! Target-independent explicit and implicit lifecycle dependencies.

mod array;
mod class;
mod optional_shared;

use crate::{
    identity::StaticFieldId,
    mir::{MirArrayInstruction, MirProgram, MirStaticValueCleanup, PreliminaryMirProgram},
};

use self::{
    array::resolve_array_destruction_dependencies,
    class::class_finalizer_target,
    optional_shared::{
        resolve_optional_cleanup_dependencies, resolve_shared_finalizer_dependencies,
    },
};
use super::{
    extract::MirDependencyExtractor, MirDependencyEdgeKind, MirDependencyExtractionError,
    MirDependencyTarget,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MirLifecycleDependency {
    target: MirDependencyTarget,
    kind: MirDependencyEdgeKind,
}

impl MirLifecycleDependency {
    pub(crate) const fn target(self) -> MirDependencyTarget {
        self.target
    }

    pub(crate) const fn kind(self) -> MirDependencyEdgeKind {
        self.kind
    }
}

/// Resolves the target-independent dependencies needed to destroy one
/// preliminary static's eventual value. Static activation and final root
/// collection deliberately share this lifecycle policy.
pub(crate) fn resolve_static_field_destruction_dependencies(
    program: &PreliminaryMirProgram,
    field: StaticFieldId,
) -> Result<Vec<MirLifecycleDependency>, MirDependencyExtractionError> {
    let declaration = program
        .static_fields()
        .find(|candidate| candidate.field == field)
        .ok_or(MirDependencyExtractionError::UnknownStaticField(field))?;
    let cleanup = MirStaticValueCleanup::for_field(
        &program.program().optional_types,
        declaration.ty,
        field,
        declaration.span,
    )
    .ok_or(MirDependencyExtractionError::InvalidStaticCleanup(field))?;
    resolve_static_cleanup_dependencies(program.program(), field, &cleanup)
}

pub(super) fn resolve_static_cleanup_dependencies(
    program: &MirProgram,
    field: StaticFieldId,
    cleanup: &MirStaticValueCleanup,
) -> Result<Vec<MirLifecycleDependency>, MirDependencyExtractionError> {
    match cleanup {
        MirStaticValueCleanup::None => Ok(Vec::new()),
        MirStaticValueCleanup::CompleteObject(cleanup) => Ok(vec![MirLifecycleDependency {
            target: class_finalizer_target(program, cleanup.target)?,
            kind: MirDependencyEdgeKind::CompleteFinalizer,
        }]),
        MirStaticValueCleanup::OptionalClass(cleanup) => Ok(vec![MirLifecycleDependency {
            target: class_finalizer_target(program, cleanup.class)?,
            kind: MirDependencyEdgeKind::OptionalLifecycle,
        }]),
        MirStaticValueCleanup::Shared(cleanup) => resolve_shared_finalizer_dependencies(
            program,
            cleanup.target,
            MirDependencyEdgeKind::SharedFinalizer,
        ),
        MirStaticValueCleanup::OptionalShared(cleanup) => resolve_shared_finalizer_dependencies(
            program,
            cleanup.target,
            MirDependencyEdgeKind::SharedFinalizer,
        ),
        MirStaticValueCleanup::AggregateOptional(cleanup) => {
            resolve_optional_cleanup_dependencies(program, cleanup.optional)
        }
        MirStaticValueCleanup::Array(MirArrayInstruction::Release { array, .. }) => {
            resolve_array_destruction_dependencies(program, *array)
        }
        MirStaticValueCleanup::Array(_) => {
            Err(MirDependencyExtractionError::InvalidStaticCleanup(field))
        }
    }
}

impl MirDependencyExtractor<'_> {
    pub(super) fn extract_implicit_lifecycle(
        &mut self,
    ) -> Result<(), MirDependencyExtractionError> {
        self.extract_class_lifecycle()?;
        self.extract_array_lifecycle()
    }
}
