//! Explicit whole-program root policy, separate from dependency extraction.

use crate::{
    identity::StaticFieldId,
    mir::{
        MirArrayInstruction, MirArrayLifecycleOperation, MirClassLifecycleOperation,
        MirExecutionNode, MirFunctionLinkage, MirProgram, MirStaticActivationWork,
        MirStaticValueCleanup,
    },
    source::Span,
};

use super::{
    lifecycle::{
        resolve_optional_cleanup_dependencies, resolve_shared_finalizer_dependencies,
        MirLifecycleDependency,
    },
    mir_reachability_root_reason_key, mir_span_key, MirDependencyEdgeKind,
    MirDependencyExtractionError, MirDependencyTarget, MirReachabilityRoot,
    MirReachabilityRootReason, MirReachabilityRootTarget, MirRuntimeEntity,
};

pub(super) struct MirReachabilityRoots {
    pub(super) roots: Vec<MirReachabilityRoot>,
    pub(super) runtime_entities: Vec<MirRuntimeEntity>,
}

pub(super) fn collect_reachability_roots(
    program: &MirProgram,
) -> Result<MirReachabilityRoots, MirDependencyExtractionError> {
    let entry = program.declarations.get(program.entry_function).ok_or(
        MirDependencyExtractionError::UnknownFunction(program.entry_function),
    )?;
    if entry.linkage != MirFunctionLinkage::Internal {
        return Err(MirDependencyExtractionError::NonInternalEntry(
            program.entry_function,
        ));
    }
    let mut collector = RootCollector {
        program,
        roots: vec![MirReachabilityRoot::new(
            MirReachabilityRootTarget::Execution(MirExecutionNode::callable(
                program.entry_function.into(),
            )),
            MirReachabilityRootReason::Entry,
            entry.span,
        )],
        runtime_entities: Vec::new(),
    };

    if let Some(coordinator) = &program.static_lifecycle {
        for activation in coordinator.activation() {
            let span = activation
                .transitions
                .first()
                .map_or(program.span, |transition| transition.span);
            collector
                .runtime_entities
                .push(MirRuntimeEntity::StaticStorage(activation.field));
            match activation.work {
                MirStaticActivationWork::ZeroDefault => collector.add_root(
                    MirReachabilityRootTarget::RuntimeEntity(MirRuntimeEntity::StaticStorage(
                        activation.field,
                    )),
                    MirReachabilityRootReason::StaticActivation(activation.field),
                    span,
                ),
                MirStaticActivationWork::Explicit(initializer) => {
                    collector.add_root(
                        MirReachabilityRootTarget::Execution(MirExecutionNode::callable(
                            initializer.into(),
                        )),
                        MirReachabilityRootReason::StaticActivation(activation.field),
                        span,
                    );
                }
            }
        }
        for destruction in coordinator.shutdown() {
            collector
                .runtime_entities
                .push(MirRuntimeEntity::StaticStorage(destruction.field));
            collector.add_shutdown_roots(
                destruction.field,
                &destruction.cleanup,
                destruction.begin.span,
            )?;
        }
    }

    collector.roots.sort_by_key(root_key);
    collector.roots.dedup();
    collector.runtime_entities.sort_unstable();
    collector.runtime_entities.dedup();
    Ok(MirReachabilityRoots {
        roots: collector.roots,
        runtime_entities: collector.runtime_entities,
    })
}

struct RootCollector<'mir> {
    program: &'mir MirProgram,
    roots: Vec<MirReachabilityRoot>,
    runtime_entities: Vec<MirRuntimeEntity>,
}

impl RootCollector<'_> {
    fn add_root(
        &mut self,
        target: MirReachabilityRootTarget,
        reason: MirReachabilityRootReason,
        span: Span,
    ) {
        self.roots
            .push(MirReachabilityRoot::new(target, reason, span));
    }

    fn add_shutdown_roots(
        &mut self,
        field: StaticFieldId,
        cleanup: &MirStaticValueCleanup,
        fallback_span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        let reason = MirReachabilityRootReason::StaticShutdown(field);
        match cleanup {
            MirStaticValueCleanup::None => self.add_root(
                MirReachabilityRootTarget::RuntimeEntity(MirRuntimeEntity::StaticStorage(field)),
                reason,
                fallback_span,
            ),
            MirStaticValueCleanup::CompleteObject(cleanup) => {
                self.add_class_finalizer_root(cleanup.target, reason, cleanup.span)?
            }
            MirStaticValueCleanup::OptionalClass(cleanup) => {
                self.add_class_finalizer_root(cleanup.class, reason, cleanup.span)?
            }
            MirStaticValueCleanup::Shared(cleanup) => {
                let dependencies = resolve_shared_finalizer_dependencies(
                    self.program,
                    cleanup.target,
                    MirDependencyEdgeKind::SharedFinalizer,
                )?;
                self.add_lifecycle_roots(field, dependencies, reason, cleanup.span)?
            }
            MirStaticValueCleanup::OptionalShared(cleanup) => {
                let dependencies = resolve_shared_finalizer_dependencies(
                    self.program,
                    cleanup.target,
                    MirDependencyEdgeKind::SharedFinalizer,
                )?;
                self.add_lifecycle_roots(field, dependencies, reason, cleanup.span)?
            }
            MirStaticValueCleanup::AggregateOptional(cleanup) => {
                let dependencies =
                    resolve_optional_cleanup_dependencies(self.program, cleanup.optional)?;
                self.add_lifecycle_roots(field, dependencies, reason, cleanup.span)?
            }
            MirStaticValueCleanup::Array(instruction) => match instruction {
                MirArrayInstruction::Release { array, span, .. } => {
                    self.add_array_destruction_root(*array, reason, *span)?
                }
                MirArrayInstruction::Allocate { .. }
                | MirArrayInstruction::AllocateElements { .. }
                | MirArrayInstruction::InitializeElement { .. }
                | MirArrayInstruction::CompleteElement { .. }
                | MirArrayInstruction::InitializeNext { .. }
                | MirArrayInstruction::CopyNext { .. }
                | MirArrayInstruction::Publish { .. }
                | MirArrayInstruction::PublishShared { .. }
                | MirArrayInstruction::Adopt { .. }
                | MirArrayInstruction::Replace { .. }
                | MirArrayInstruction::ElementAssign { .. }
                | MirArrayInstruction::DestroyNext { .. }
                | MirArrayInstruction::AnchorBegin { .. }
                | MirArrayInstruction::AnchorEnd { .. }
                | MirArrayInstruction::AliasBind { .. }
                | MirArrayInstruction::Normalize { .. }
                | MirArrayInstruction::Offset { .. }
                | MirArrayInstruction::Boundary { .. }
                | MirArrayInstruction::SliceCopy { .. }
                | MirArrayInstruction::SliceAssignNext { .. }
                | MirArrayInstruction::SliceBoundsCheck { .. }
                | MirArrayInstruction::SliceLengthCheck { .. } => {
                    return Err(MirDependencyExtractionError::InvalidStaticCleanup(field));
                }
            },
        }
        Ok(())
    }

    fn add_class_finalizer_root(
        &mut self,
        class: crate::identity::ClassId,
        reason: MirReachabilityRootReason,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        if self.program.class(class).is_none() {
            return Err(MirDependencyExtractionError::UnknownClass(class));
        }
        self.add_root(
            MirReachabilityRootTarget::Execution(MirExecutionNode::class(
                class,
                MirClassLifecycleOperation::CompleteFinalizer,
            )),
            reason,
            span,
        );
        Ok(())
    }

    fn add_array_destruction_root(
        &mut self,
        array: crate::identity::ArrayTypeId,
        reason: MirReachabilityRootReason,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        if self.program.array_type(array).is_none() {
            return Err(MirDependencyExtractionError::UnknownArrayType(array));
        }
        self.runtime_entities
            .push(MirRuntimeEntity::ArrayLifecycle(array));
        self.add_root(
            MirReachabilityRootTarget::Execution(MirExecutionNode::array(
                array,
                MirArrayLifecycleOperation::Destruction,
            )),
            reason,
            span,
        );
        Ok(())
    }

    fn add_lifecycle_roots(
        &mut self,
        field: StaticFieldId,
        dependencies: Vec<MirLifecycleDependency>,
        reason: MirReachabilityRootReason,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        for dependency in dependencies {
            match dependency.target {
                MirDependencyTarget::Execution(node) => {
                    self.add_root(MirReachabilityRootTarget::Execution(node), reason, span)
                }
                MirDependencyTarget::RuntimeEntity(entity) => {
                    self.runtime_entities.push(entity);
                    self.add_root(
                        MirReachabilityRootTarget::RuntimeEntity(entity),
                        reason,
                        span,
                    );
                }
                MirDependencyTarget::External(_) | MirDependencyTarget::Intrinsic(_) => {
                    return Err(MirDependencyExtractionError::InvalidStaticCleanup(field));
                }
            }
        }
        Ok(())
    }
}

fn root_key(
    root: &MirReachabilityRoot,
) -> (
    (u8, usize, usize),
    MirReachabilityRootTarget,
    (usize, usize, usize),
) {
    (
        mir_reachability_root_reason_key(root.reason()),
        root.target(),
        mir_span_key(root.span()),
    )
}
