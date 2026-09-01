//! Recursive optional cleanup and shared-owner finalizer dependencies.

use std::collections::BTreeSet;

use crate::{
    identity::OptionalTypeId,
    mir::{
        MirArrayLifecycleOperation, MirClassLifecycleOperation, MirExecutionNode,
        MirOptionalAssignmentPlan, MirOptionalCleanupPlan, MirOptionalCopyPlan, MirProgram,
        MirSharedTarget, MirType, PreliminaryMirSharedLifecycleTarget,
    },
    source::Span,
};

use super::{
    super::{
        extract::MirDependencyExtractor, MirDependencyEdgeKind, MirDependencyExtractionError,
        MirDependencyRegion, MirDependencyTarget, MirRuntimeEntity,
    },
    class::class_finalizer_target,
    MirLifecycleDependency,
};

pub(super) fn resolve_shared_finalizer_dependencies(
    program: &MirProgram,
    target: MirSharedTarget,
    kind: MirDependencyEdgeKind,
) -> Result<Vec<MirLifecycleDependency>, MirDependencyExtractionError> {
    let mut dependencies = Vec::new();
    for target in program.shared_lifecycle_targets(target) {
        match target {
            PreliminaryMirSharedLifecycleTarget::Class(class) => {
                if program.class(class).is_none() {
                    return Err(MirDependencyExtractionError::UnknownClass(class));
                }
                dependencies.push(MirLifecycleDependency {
                    target: MirDependencyTarget::Execution(MirExecutionNode::class(
                        class,
                        MirClassLifecycleOperation::CompleteFinalizer,
                    )),
                    kind,
                });
            }
            PreliminaryMirSharedLifecycleTarget::Array(array) => {
                if program.array_type(array).is_none() {
                    return Err(MirDependencyExtractionError::UnknownArrayType(array));
                }
                dependencies.push(MirLifecycleDependency {
                    target: MirDependencyTarget::Execution(MirExecutionNode::array(
                        array,
                        MirArrayLifecycleOperation::Destruction,
                    )),
                    kind,
                });
                dependencies.push(MirLifecycleDependency {
                    target: MirDependencyTarget::RuntimeEntity(MirRuntimeEntity::ArrayLifecycle(
                        array,
                    )),
                    kind: MirDependencyEdgeKind::RuntimeEntityReference,
                });
            }
            PreliminaryMirSharedLifecycleTarget::OptionalBox(target) => {
                let metadata = program
                    .optional_box_type(target)
                    .ok_or(MirDependencyExtractionError::UnknownOptionalBoxType(target))?;
                dependencies.push(MirLifecycleDependency {
                    target: MirDependencyTarget::RuntimeEntity(
                        MirRuntimeEntity::OptionalBoxLayout(target),
                    ),
                    kind: MirDependencyEdgeKind::RuntimeEntityReference,
                });
                if let Some(optional) = metadata.exact_optional {
                    dependencies.extend(resolve_optional_cleanup_dependencies(program, optional)?);
                }
            }
        }
    }
    Ok(dependencies)
}

pub(super) fn resolve_optional_cleanup_dependencies(
    program: &MirProgram,
    optional: OptionalTypeId,
) -> Result<Vec<MirLifecycleDependency>, MirDependencyExtractionError> {
    let mut dependencies = Vec::new();
    let mut pending = vec![optional];
    let mut visited = BTreeSet::new();
    while let Some(optional) = pending.pop() {
        if !visited.insert(optional) {
            return Err(MirDependencyExtractionError::CyclicOptionalLifecycle(
                optional,
            ));
        }
        dependencies.push(MirLifecycleDependency {
            target: MirDependencyTarget::RuntimeEntity(MirRuntimeEntity::OptionalLifecycle(
                optional,
            )),
            kind: MirDependencyEdgeKind::RuntimeEntityReference,
        });
        let cleanup = program
            .optional_type(optional)
            .ok_or(MirDependencyExtractionError::UnknownOptionalType(optional))?
            .lifecycle
            .cleanup;
        match cleanup {
            MirOptionalCleanupPlan::Class(class) => {
                if program.class(class).is_none() {
                    return Err(MirDependencyExtractionError::UnknownClass(class));
                }
                dependencies.push(MirLifecycleDependency {
                    target: MirDependencyTarget::Execution(MirExecutionNode::class(
                        class,
                        MirClassLifecycleOperation::CompleteFinalizer,
                    )),
                    kind: MirDependencyEdgeKind::OptionalLifecycle,
                });
            }
            MirOptionalCleanupPlan::Optional(nested) => pending.push(nested),
            MirOptionalCleanupPlan::Array(array) => {
                if program.array_type(array).is_none() {
                    return Err(MirDependencyExtractionError::UnknownArrayType(array));
                }
                dependencies.push(MirLifecycleDependency {
                    target: MirDependencyTarget::Execution(MirExecutionNode::array(
                        array,
                        MirArrayLifecycleOperation::Destruction,
                    )),
                    kind: MirDependencyEdgeKind::ArrayDestruction,
                });
                dependencies.push(MirLifecycleDependency {
                    target: MirDependencyTarget::RuntimeEntity(MirRuntimeEntity::ArrayLifecycle(
                        array,
                    )),
                    kind: MirDependencyEdgeKind::RuntimeEntityReference,
                });
            }
            MirOptionalCleanupPlan::Shared(target) => {
                dependencies.extend(resolve_shared_finalizer_dependencies(
                    program,
                    target,
                    MirDependencyEdgeKind::SharedFinalizer,
                )?)
            }
            MirOptionalCleanupPlan::Trivial => {}
        }
    }
    Ok(dependencies)
}

impl MirDependencyExtractor<'_> {
    pub(in crate::passes::reachability) fn add_shared_type_finalizers(
        &mut self,
        source: MirExecutionNode,
        ty: MirType,
        region: MirDependencyRegion,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        match ty {
            MirType::Shared(target) => self.add_shared_finalizers(
                source,
                target,
                MirDependencyEdgeKind::SharedFinalizer,
                region,
                span,
            ),
            MirType::Optional(optional) => {
                let metadata = self
                    .program()
                    .optional_type(optional)
                    .ok_or(MirDependencyExtractionError::UnknownOptionalType(optional))?;
                if let Some(target) = metadata.shared_owner() {
                    self.add_shared_finalizers(
                        source,
                        target,
                        MirDependencyEdgeKind::SharedFinalizer,
                        region,
                        span,
                    )?;
                }
                Ok(())
            }
            MirType::I64
            | MirType::U64
            | MirType::U8
            | MirType::F64
            | MirType::Bool
            | MirType::Class(_)
            | MirType::Array(_)
            | MirType::Function(_)
            | MirType::Interface(_)
            | MirType::Obj
            | MirType::Unit => Ok(()),
        }
    }

    pub(in crate::passes::reachability) fn add_shared_finalizers(
        &mut self,
        source: MirExecutionNode,
        target: MirSharedTarget,
        kind: MirDependencyEdgeKind,
        region: MirDependencyRegion,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        for dependency in resolve_shared_finalizer_dependencies(self.program(), target, kind)? {
            self.add_dependency(source, dependency.target, dependency.kind, region, span);
        }
        Ok(())
    }

    pub(in crate::passes::reachability) fn add_optional_copy(
        &mut self,
        source: MirExecutionNode,
        optional: OptionalTypeId,
        region: MirDependencyRegion,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        let plan = self
            .program()
            .optional_type(optional)
            .ok_or(MirDependencyExtractionError::UnknownOptionalType(optional))?
            .lifecycle
            .copy;
        self.add_runtime_entity(
            source,
            MirRuntimeEntity::OptionalLifecycle(optional),
            region,
            span,
        );
        match plan {
            Some(MirOptionalCopyPlan::Class { operation, .. }) => self.add_copy_constructor(
                source,
                operation,
                MirDependencyEdgeKind::ArrayCopy,
                region,
                span,
            )?,
            Some(MirOptionalCopyPlan::Optional(nested)) => {
                self.add_optional_copy(source, nested, region, span)?
            }
            Some(MirOptionalCopyPlan::Array(array)) => self.add_array_lifecycle(
                source,
                array,
                MirArrayLifecycleOperation::Copy,
                MirDependencyEdgeKind::ArrayCopy,
                region,
                span,
            )?,
            Some(MirOptionalCopyPlan::Trivial | MirOptionalCopyPlan::Shared(_)) | None => {}
        }
        Ok(())
    }

    pub(in crate::passes::reachability) fn add_optional_assignment(
        &mut self,
        source: MirExecutionNode,
        optional: OptionalTypeId,
        region: MirDependencyRegion,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        let plan = self
            .program()
            .optional_type(optional)
            .ok_or(MirDependencyExtractionError::UnknownOptionalType(optional))?
            .lifecycle
            .assignment;
        self.add_runtime_entity(
            source,
            MirRuntimeEntity::OptionalLifecycle(optional),
            region,
            span,
        );
        match plan {
            Some(MirOptionalAssignmentPlan::Class {
                class,
                copy_constructor,
                copy_assignment,
            }) => {
                self.add_copy_constructor(
                    source,
                    copy_constructor,
                    MirDependencyEdgeKind::ArrayAssignment,
                    region,
                    span,
                )?;
                self.add_copy_assignment(
                    source,
                    copy_assignment,
                    MirDependencyEdgeKind::ArrayAssignment,
                    region,
                    span,
                )?;
                self.add_complete_finalizer(
                    source,
                    class,
                    MirDependencyEdgeKind::OptionalLifecycle,
                    region,
                    span,
                )?;
            }
            Some(MirOptionalAssignmentPlan::Optional(nested)) => {
                self.add_optional_assignment(source, nested, region, span)?
            }
            Some(MirOptionalAssignmentPlan::Array(array)) => self.add_array_lifecycle(
                source,
                array,
                MirArrayLifecycleOperation::Assignment,
                MirDependencyEdgeKind::ArrayAssignment,
                region,
                span,
            )?,
            Some(MirOptionalAssignmentPlan::Shared(target)) => self.add_shared_finalizers(
                source,
                target,
                MirDependencyEdgeKind::SharedFinalizer,
                region,
                span,
            )?,
            Some(MirOptionalAssignmentPlan::Trivial) | None => {}
        }
        Ok(())
    }

    pub(in crate::passes::reachability) fn add_optional_cleanup(
        &mut self,
        source: MirExecutionNode,
        optional: OptionalTypeId,
        region: MirDependencyRegion,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        for dependency in resolve_optional_cleanup_dependencies(self.program(), optional)? {
            self.add_dependency(source, dependency.target, dependency.kind, region, span);
        }
        Ok(())
    }
}
