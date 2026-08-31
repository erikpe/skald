//! Target-independent explicit and implicit lifecycle dependencies.

use std::collections::BTreeSet;

use crate::{
    identity::{
        ArrayTypeId, ClassId, CopyAssignmentId, CopyConstructorId, InitializerId, OptionalTypeId,
    },
    mir::{
        MirArrayAssignElement, MirArrayCopyElement, MirArrayDefaultElement, MirArrayDestroyElement,
        MirArrayInstruction, MirArrayLifecycleOperation, MirClassLifecycleOperation,
        MirCopyCapability, MirDestructionStep, MirExecutionNode, MirOptionalAssignmentPlan,
        MirOptionalCleanupPlan, MirOptionalCopyPlan, MirProgram, MirSelectedCopyOperation,
        MirSharedTarget, MirStaticValueCleanup, MirSynthesizedCopy, MirSynthesizedFieldCopy,
        MirType, PreliminaryMirProgram, PreliminaryMirSharedLifecycleTarget,
    },
    source::Span,
};

use super::{
    extract::MirDependencyExtractor, MirDependencyEdgeKind, MirDependencyExtractionError,
    MirDependencyRegion, MirDependencyTarget, MirRuntimeEntity,
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
    field: crate::identity::StaticFieldId,
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
    field: crate::identity::StaticFieldId,
    cleanup: &MirStaticValueCleanup,
) -> Result<Vec<MirLifecycleDependency>, MirDependencyExtractionError> {
    let dependencies = match cleanup {
        MirStaticValueCleanup::None => Vec::new(),
        MirStaticValueCleanup::CompleteObject(cleanup) => vec![MirLifecycleDependency {
            target: class_finalizer_target(program, cleanup.target)?,
            kind: MirDependencyEdgeKind::CompleteFinalizer,
        }],
        MirStaticValueCleanup::OptionalClass(cleanup) => vec![MirLifecycleDependency {
            target: class_finalizer_target(program, cleanup.class)?,
            kind: MirDependencyEdgeKind::OptionalLifecycle,
        }],
        MirStaticValueCleanup::Shared(cleanup) => resolve_shared_finalizer_dependencies(
            program,
            cleanup.target,
            MirDependencyEdgeKind::SharedFinalizer,
        )?,
        MirStaticValueCleanup::OptionalShared(cleanup) => resolve_shared_finalizer_dependencies(
            program,
            cleanup.target,
            MirDependencyEdgeKind::SharedFinalizer,
        )?,
        MirStaticValueCleanup::AggregateOptional(cleanup) => {
            resolve_optional_cleanup_dependencies(program, cleanup.optional)?
        }
        MirStaticValueCleanup::Array(MirArrayInstruction::Release { array, .. }) => {
            if program.array_type(*array).is_none() {
                return Err(MirDependencyExtractionError::UnknownArrayType(*array));
            }
            vec![
                MirLifecycleDependency {
                    target: MirDependencyTarget::Execution(MirExecutionNode::array(
                        *array,
                        MirArrayLifecycleOperation::Destruction,
                    )),
                    kind: MirDependencyEdgeKind::ArrayDestruction,
                },
                MirLifecycleDependency {
                    target: MirDependencyTarget::RuntimeEntity(MirRuntimeEntity::ArrayLifecycle(
                        *array,
                    )),
                    kind: MirDependencyEdgeKind::RuntimeEntityReference,
                },
            ]
        }
        MirStaticValueCleanup::Array(_) => {
            return Err(MirDependencyExtractionError::InvalidStaticCleanup(field));
        }
    };
    Ok(dependencies)
}

fn class_finalizer_target(
    program: &MirProgram,
    class: ClassId,
) -> Result<MirDependencyTarget, MirDependencyExtractionError> {
    if program.class(class).is_none() {
        return Err(MirDependencyExtractionError::UnknownClass(class));
    }
    Ok(MirDependencyTarget::Execution(MirExecutionNode::class(
        class,
        MirClassLifecycleOperation::CompleteFinalizer,
    )))
}

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
    pub(super) fn extract_implicit_lifecycle(
        &mut self,
    ) -> Result<(), MirDependencyExtractionError> {
        let classes: Vec<_> = self.program().classes.iter().cloned().collect();
        for class in classes {
            let id = class.id;
            let span = class.span;
            let copy_constructor = class.copy_constructor;
            let copy_assignment = class.copy_assignment;
            let destruction = class.destruction;
            let direct_base = class.direct_base;

            self.extract_copy_constructor(id, &copy_constructor, span)?;
            self.extract_copy_assignment(id, &copy_assignment, span)?;
            let source = MirExecutionNode::class(id, MirClassLifecycleOperation::CompleteFinalizer);
            for step in destruction.steps {
                match step {
                    MirDestructionStep::UserBody(destructor) => {
                        self.program()
                            .destructor(destructor)
                            .ok_or(MirDependencyExtractionError::UnknownDestructor(destructor))?;
                        self.add_execution_dependency(
                            source,
                            MirExecutionNode::callable(destructor.into()),
                            MirDependencyEdgeKind::UserDestructor,
                            MirDependencyRegion::Destruction,
                            destruction
                                .destructor
                                .as_ref()
                                .map_or(span, |declaration| declaration.span),
                        );
                    }
                    MirDestructionStep::Field(field)
                    | MirDestructionStep::OptionalClassField(field) => {
                        let declaration = self
                            .program()
                            .field(field)
                            .ok_or(MirDependencyExtractionError::UnknownField(field))?;
                        let target = match declaration.ty {
                            MirType::Class(class) => class,
                            MirType::Optional(optional) => self
                                .program()
                                .optional_type(optional)
                                .ok_or(MirDependencyExtractionError::UnknownOptionalType(optional))?
                                .inline_class()
                                .ok_or(MirDependencyExtractionError::InvalidLifecycleFieldType(
                                    field,
                                ))?,
                            _ => {
                                return Err(
                                    MirDependencyExtractionError::InvalidLifecycleFieldType(field),
                                );
                            }
                        };
                        self.add_complete_finalizer(
                            source,
                            target,
                            MirDependencyEdgeKind::FieldFinalizer,
                            MirDependencyRegion::Destruction,
                            declaration.span,
                        )?;
                    }
                    MirDestructionStep::SharedField(field)
                    | MirDestructionStep::OptionalSharedField(field) => {
                        let declaration = self
                            .program()
                            .field(field)
                            .ok_or(MirDependencyExtractionError::UnknownField(field))?;
                        let target = match declaration.ty {
                            MirType::Shared(target) => target,
                            MirType::Optional(optional) => self
                                .program()
                                .optional_type(optional)
                                .ok_or(MirDependencyExtractionError::UnknownOptionalType(optional))?
                                .shared_owner()
                                .ok_or(MirDependencyExtractionError::InvalidLifecycleFieldType(
                                    field,
                                ))?,
                            _ => {
                                return Err(
                                    MirDependencyExtractionError::InvalidLifecycleFieldType(field),
                                );
                            }
                        };
                        self.add_shared_finalizers(
                            source,
                            target,
                            MirDependencyEdgeKind::SharedFinalizer,
                            MirDependencyRegion::Destruction,
                            declaration.span,
                        )?;
                    }
                    MirDestructionStep::ArrayField(field) => {
                        let declaration = self
                            .program()
                            .field(field)
                            .ok_or(MirDependencyExtractionError::UnknownField(field))?;
                        let MirType::Array(array) = declaration.ty else {
                            return Err(MirDependencyExtractionError::InvalidLifecycleFieldType(
                                field,
                            ));
                        };
                        self.add_array_lifecycle(
                            source,
                            array,
                            MirArrayLifecycleOperation::Destruction,
                            MirDependencyEdgeKind::ArrayDestruction,
                            MirDependencyRegion::Destruction,
                            declaration.span,
                        )?;
                    }
                    MirDestructionStep::OptionalField { field, optional } => {
                        let span = self
                            .program()
                            .field(field)
                            .ok_or(MirDependencyExtractionError::UnknownField(field))?
                            .span;
                        self.add_optional_cleanup(
                            source,
                            optional,
                            MirDependencyRegion::Destruction,
                            span,
                        )?;
                    }
                    MirDestructionStep::Base(base) => self.add_complete_finalizer(
                        source,
                        base,
                        MirDependencyEdgeKind::BaseFinalizer,
                        MirDependencyRegion::Destruction,
                        direct_base.map_or(span, |declaration| declaration.span),
                    )?,
                }
            }
        }

        let arrays: Vec<_> = self.program().array_types.iter().cloned().collect();
        for array in arrays {
            let span = self.program().span;
            if let Some(operation) = array.lifecycle.default {
                let source = MirExecutionNode::array(array.id, MirArrayLifecycleOperation::Default);
                self.add_array_default(
                    source,
                    operation,
                    span,
                    MirDependencyRegion::ArrayLifecycle,
                )?;
            }
            if let Some(operation) = array.lifecycle.copy {
                let source = MirExecutionNode::array(array.id, MirArrayLifecycleOperation::Copy);
                self.add_array_copy(source, operation, span, MirDependencyRegion::ArrayLifecycle)?;
            }
            if let Some(operation) = array.lifecycle.assignment {
                let source =
                    MirExecutionNode::array(array.id, MirArrayLifecycleOperation::Assignment);
                self.add_array_assignment(
                    source,
                    operation,
                    span,
                    MirDependencyRegion::ArrayLifecycle,
                )?;
            }
            let source = MirExecutionNode::array(array.id, MirArrayLifecycleOperation::Destruction);
            self.add_array_destruction(
                source,
                array.lifecycle.destruction,
                span,
                MirDependencyRegion::ArrayLifecycle,
            )?;
        }
        Ok(())
    }

    pub(super) fn add_copy_constructor(
        &mut self,
        source: MirExecutionNode,
        operation: MirSelectedCopyOperation<CopyConstructorId>,
        kind: MirDependencyEdgeKind,
        region: MirDependencyRegion,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        let class = match operation {
            MirSelectedCopyOperation::User(id) => {
                self.program()
                    .copy_constructor(id)
                    .ok_or(MirDependencyExtractionError::UnknownCopyConstructor(id))?;
                id.class()
            }
            MirSelectedCopyOperation::Synthesized(class) => class,
        };
        self.add_class_lifecycle(
            source,
            class,
            MirClassLifecycleOperation::CopyConstructor,
            kind,
            region,
            span,
        )
    }

    fn add_copy_constructor_for_class(
        &mut self,
        source: MirExecutionNode,
        class: ClassId,
        kind: MirDependencyEdgeKind,
        region: MirDependencyRegion,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        self.add_class_lifecycle(
            source,
            class,
            MirClassLifecycleOperation::CopyConstructor,
            kind,
            region,
            span,
        )
    }

    pub(super) fn add_copy_assignment(
        &mut self,
        source: MirExecutionNode,
        operation: MirSelectedCopyOperation<CopyAssignmentId>,
        kind: MirDependencyEdgeKind,
        region: MirDependencyRegion,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        let class = match operation {
            MirSelectedCopyOperation::User(id) => {
                self.program()
                    .copy_assignment(id)
                    .ok_or(MirDependencyExtractionError::UnknownCopyAssignment(id))?;
                id.class()
            }
            MirSelectedCopyOperation::Synthesized(class) => class,
        };
        self.add_class_lifecycle(
            source,
            class,
            MirClassLifecycleOperation::CopyAssignment,
            kind,
            region,
            span,
        )
    }

    pub(super) fn add_complete_finalizer(
        &mut self,
        source: MirExecutionNode,
        class: ClassId,
        kind: MirDependencyEdgeKind,
        region: MirDependencyRegion,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        self.add_class_lifecycle(
            source,
            class,
            MirClassLifecycleOperation::CompleteFinalizer,
            kind,
            region,
            span,
        )
    }

    pub(super) fn add_initializer(
        &mut self,
        source: MirExecutionNode,
        initializer: InitializerId,
        kind: MirDependencyEdgeKind,
        region: MirDependencyRegion,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        self.program().initializer(initializer).ok_or(
            MirDependencyExtractionError::UnknownInitializer(initializer),
        )?;
        self.add_execution_dependency(
            source,
            MirExecutionNode::callable(initializer.into()),
            kind,
            region,
            span,
        );
        Ok(())
    }

    fn add_class_lifecycle(
        &mut self,
        source: MirExecutionNode,
        class: ClassId,
        operation: MirClassLifecycleOperation,
        kind: MirDependencyEdgeKind,
        region: MirDependencyRegion,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        if self.program().class(class).is_none() {
            return Err(MirDependencyExtractionError::UnknownClass(class));
        }
        self.add_execution_dependency(
            source,
            MirExecutionNode::class(class, operation),
            kind,
            region,
            span,
        );
        Ok(())
    }

    fn add_array_lifecycle(
        &mut self,
        source: MirExecutionNode,
        array: ArrayTypeId,
        operation: MirArrayLifecycleOperation,
        kind: MirDependencyEdgeKind,
        region: MirDependencyRegion,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        if self.program().array_type(array).is_none() {
            return Err(MirDependencyExtractionError::UnknownArrayType(array));
        }
        self.add_execution_dependency(
            source,
            MirExecutionNode::array(array, operation),
            kind,
            region,
            span,
        );
        self.add_runtime_entity(
            source,
            MirRuntimeEntity::ArrayLifecycle(array),
            region,
            span,
        );
        Ok(())
    }

    pub(super) fn add_shared_type_finalizers(
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

    pub(super) fn add_shared_finalizers(
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

    fn extract_copy_constructor(
        &mut self,
        class: ClassId,
        capability: &MirCopyCapability<CopyConstructorId>,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        let source = MirExecutionNode::class(class, MirClassLifecycleOperation::CopyConstructor);
        match capability {
            MirCopyCapability::User(copy) => {
                if let Some(base) = copy.base {
                    self.add_copy_constructor(
                        source,
                        base.operation,
                        MirDependencyEdgeKind::BaseCopy,
                        MirDependencyRegion::Copy,
                        span,
                    )?;
                }
                self.add_execution_dependency(
                    source,
                    MirExecutionNode::callable(copy.operation.into()),
                    MirDependencyEdgeKind::UserCopyBody,
                    MirDependencyRegion::Copy,
                    span,
                );
            }
            MirCopyCapability::Synthesized(copy) => {
                self.extract_synthesized_constructor(source, copy, span)?
            }
            MirCopyCapability::Unavailable => {}
        }
        Ok(())
    }

    fn extract_synthesized_constructor(
        &mut self,
        source: MirExecutionNode,
        copy: &MirSynthesizedCopy<CopyConstructorId>,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        if let Some(base) = copy.base {
            self.add_copy_constructor(
                source,
                base.operation,
                MirDependencyEdgeKind::BaseCopy,
                MirDependencyRegion::Copy,
                span,
            )?;
        }
        for field in &copy.fields {
            let field_span = self
                .program()
                .field(field.field())
                .ok_or(MirDependencyExtractionError::UnknownField(field.field()))?
                .span;
            match *field {
                MirSynthesizedFieldCopy::Class { operation, .. }
                | MirSynthesizedFieldCopy::OptionalClass { operation, .. } => self
                    .add_copy_constructor(
                        source,
                        operation,
                        MirDependencyEdgeKind::FieldCopy,
                        MirDependencyRegion::Copy,
                        field_span,
                    )?,
                MirSynthesizedFieldCopy::Array { array, .. } => self.add_array_lifecycle(
                    source,
                    array,
                    MirArrayLifecycleOperation::Copy,
                    MirDependencyEdgeKind::ArrayCopy,
                    MirDependencyRegion::Copy,
                    field_span,
                )?,
                MirSynthesizedFieldCopy::Optional { optional, .. } => {
                    self.add_optional_copy(source, optional, MirDependencyRegion::Copy, field_span)?
                }
                MirSynthesizedFieldCopy::Primitive { .. }
                | MirSynthesizedFieldCopy::OptionalPrimitive { .. }
                | MirSynthesizedFieldCopy::Shared { .. }
                | MirSynthesizedFieldCopy::OptionalShared { .. } => {}
            }
        }
        Ok(())
    }

    fn extract_copy_assignment(
        &mut self,
        class: ClassId,
        capability: &MirCopyCapability<CopyAssignmentId>,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        let source = MirExecutionNode::class(class, MirClassLifecycleOperation::CopyAssignment);
        match capability {
            MirCopyCapability::User(copy) => {
                if let Some(base) = copy.base {
                    self.add_copy_assignment(
                        source,
                        base.operation,
                        MirDependencyEdgeKind::BaseCopy,
                        MirDependencyRegion::Copy,
                        span,
                    )?;
                }
                self.add_execution_dependency(
                    source,
                    MirExecutionNode::callable(copy.operation.into()),
                    MirDependencyEdgeKind::UserCopyBody,
                    MirDependencyRegion::Copy,
                    span,
                );
            }
            MirCopyCapability::Synthesized(copy) => {
                self.extract_synthesized_assignment(source, copy, span)?
            }
            MirCopyCapability::Unavailable => {}
        }
        Ok(())
    }

    fn extract_synthesized_assignment(
        &mut self,
        source: MirExecutionNode,
        copy: &MirSynthesizedCopy<CopyAssignmentId>,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        if let Some(base) = copy.base {
            self.add_copy_assignment(
                source,
                base.operation,
                MirDependencyEdgeKind::BaseCopy,
                MirDependencyRegion::Copy,
                span,
            )?;
        }
        for field in &copy.fields {
            let field_span = self
                .program()
                .field(field.field())
                .ok_or(MirDependencyExtractionError::UnknownField(field.field()))?
                .span;
            match *field {
                MirSynthesizedFieldCopy::Class { operation, .. } => self.add_copy_assignment(
                    source,
                    operation,
                    MirDependencyEdgeKind::FieldCopy,
                    MirDependencyRegion::Copy,
                    field_span,
                )?,
                MirSynthesizedFieldCopy::OptionalClass {
                    class, operation, ..
                } => {
                    self.add_copy_constructor_for_class(
                        source,
                        class,
                        MirDependencyEdgeKind::FieldCopy,
                        MirDependencyRegion::Copy,
                        field_span,
                    )?;
                    self.add_copy_assignment(
                        source,
                        operation,
                        MirDependencyEdgeKind::FieldCopy,
                        MirDependencyRegion::Copy,
                        field_span,
                    )?;
                    self.add_complete_finalizer(
                        source,
                        class,
                        MirDependencyEdgeKind::OptionalLifecycle,
                        MirDependencyRegion::Copy,
                        field_span,
                    )?;
                }
                MirSynthesizedFieldCopy::Array { array, .. } => self.add_array_lifecycle(
                    source,
                    array,
                    MirArrayLifecycleOperation::Assignment,
                    MirDependencyEdgeKind::ArrayAssignment,
                    MirDependencyRegion::Copy,
                    field_span,
                )?,
                MirSynthesizedFieldCopy::Optional { optional, .. } => self
                    .add_optional_assignment(
                        source,
                        optional,
                        MirDependencyRegion::Copy,
                        field_span,
                    )?,
                MirSynthesizedFieldCopy::Primitive { .. }
                | MirSynthesizedFieldCopy::OptionalPrimitive { .. }
                | MirSynthesizedFieldCopy::Shared { .. }
                | MirSynthesizedFieldCopy::OptionalShared { .. } => {}
            }
        }
        Ok(())
    }

    pub(super) fn add_optional_copy(
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

    pub(super) fn add_optional_assignment(
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

    pub(super) fn add_optional_cleanup(
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

    fn add_array_default(
        &mut self,
        source: MirExecutionNode,
        operation: MirArrayDefaultElement,
        span: Span,
        region: MirDependencyRegion,
    ) -> Result<(), MirDependencyExtractionError> {
        match operation {
            MirArrayDefaultElement::Class { initializer, .. }
            | MirArrayDefaultElement::SharedClass { initializer, .. } => self.add_initializer(
                source,
                initializer,
                MirDependencyEdgeKind::ArrayDefault,
                region,
                span,
            )?,
            MirArrayDefaultElement::ArrayEmpty(array)
            | MirArrayDefaultElement::SharedArrayEmpty(array) => self.add_array_lifecycle(
                source,
                array,
                MirArrayLifecycleOperation::Default,
                MirDependencyEdgeKind::ArrayDefault,
                region,
                span,
            )?,
            MirArrayDefaultElement::SharedOptionalBoxAbsent(target) => self.add_shared_finalizers(
                source,
                MirSharedTarget::OptionalBox(target),
                MirDependencyEdgeKind::SharedFinalizer,
                region,
                span,
            )?,
            MirArrayDefaultElement::Primitive | MirArrayDefaultElement::OptionalAbsent => {}
        }
        Ok(())
    }

    fn add_array_copy(
        &mut self,
        source: MirExecutionNode,
        operation: MirArrayCopyElement,
        span: Span,
        region: MirDependencyRegion,
    ) -> Result<(), MirDependencyExtractionError> {
        match operation {
            MirArrayCopyElement::Class { operation, .. }
            | MirArrayCopyElement::OptionalClass { operation, .. } => self.add_copy_constructor(
                source,
                operation,
                MirDependencyEdgeKind::ArrayCopy,
                region,
                span,
            )?,
            MirArrayCopyElement::Array(array) => self.add_array_lifecycle(
                source,
                array,
                MirArrayLifecycleOperation::Copy,
                MirDependencyEdgeKind::ArrayCopy,
                region,
                span,
            )?,
            MirArrayCopyElement::Optional(optional) => {
                self.add_optional_copy(source, optional, region, span)?
            }
            MirArrayCopyElement::Primitive
            | MirArrayCopyElement::OptionalPrimitive
            | MirArrayCopyElement::Shared(_)
            | MirArrayCopyElement::OptionalShared(_) => {}
        }
        Ok(())
    }

    fn add_array_assignment(
        &mut self,
        source: MirExecutionNode,
        operation: MirArrayAssignElement,
        span: Span,
        region: MirDependencyRegion,
    ) -> Result<(), MirDependencyExtractionError> {
        match operation {
            MirArrayAssignElement::Class { operation, .. } => self.add_copy_assignment(
                source,
                operation,
                MirDependencyEdgeKind::ArrayAssignment,
                region,
                span,
            )?,
            MirArrayAssignElement::OptionalClass {
                class,
                copy_constructor,
                copy_assignment,
            } => {
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
            MirArrayAssignElement::Array(array) => self.add_array_lifecycle(
                source,
                array,
                MirArrayLifecycleOperation::Assignment,
                MirDependencyEdgeKind::ArrayAssignment,
                region,
                span,
            )?,
            MirArrayAssignElement::Shared(target)
            | MirArrayAssignElement::OptionalShared(target) => self.add_shared_finalizers(
                source,
                target,
                MirDependencyEdgeKind::SharedFinalizer,
                region,
                span,
            )?,
            MirArrayAssignElement::Optional(optional) => {
                self.add_optional_assignment(source, optional, region, span)?
            }
            MirArrayAssignElement::Primitive | MirArrayAssignElement::OptionalPrimitive => {}
        }
        Ok(())
    }

    fn add_array_destruction(
        &mut self,
        source: MirExecutionNode,
        operation: MirArrayDestroyElement,
        span: Span,
        region: MirDependencyRegion,
    ) -> Result<(), MirDependencyExtractionError> {
        match operation {
            MirArrayDestroyElement::Class(class) | MirArrayDestroyElement::OptionalClass(class) => {
                self.add_complete_finalizer(
                    source,
                    class,
                    MirDependencyEdgeKind::ArrayDestruction,
                    region,
                    span,
                )?
            }
            MirArrayDestroyElement::Array(array) => self.add_array_lifecycle(
                source,
                array,
                MirArrayLifecycleOperation::Destruction,
                MirDependencyEdgeKind::ArrayDestruction,
                region,
                span,
            )?,
            MirArrayDestroyElement::Shared(target)
            | MirArrayDestroyElement::OptionalShared(target) => self.add_shared_finalizers(
                source,
                target,
                MirDependencyEdgeKind::SharedFinalizer,
                region,
                span,
            )?,
            MirArrayDestroyElement::Optional(optional) => {
                self.add_optional_cleanup(source, optional, region, span)?
            }
            MirArrayDestroyElement::Trivial => {}
        }
        Ok(())
    }

    pub(super) fn extract_array_instruction(
        &mut self,
        source: MirExecutionNode,
        region: MirDependencyRegion,
        instruction: &MirArrayInstruction,
    ) -> Result<(), MirDependencyExtractionError> {
        let span = instruction.span();
        match instruction {
            MirArrayInstruction::InitializeNext { operation, .. } => {
                self.add_array_default(source, *operation, span, region)?
            }
            MirArrayInstruction::CopyNext { operation, .. }
            | MirArrayInstruction::SliceCopy { operation, .. } => {
                self.add_array_copy(source, *operation, span, region)?
            }
            MirArrayInstruction::Replace { array, .. }
            | MirArrayInstruction::Release { array, .. } => self.add_array_lifecycle(
                source,
                *array,
                MirArrayLifecycleOperation::Destruction,
                MirDependencyEdgeKind::ArrayDestruction,
                region,
                span,
            )?,
            MirArrayInstruction::ElementAssign { operation, .. }
            | MirArrayInstruction::SliceAssignNext { operation, .. } => {
                self.add_array_assignment(source, *operation, span, region)?
            }
            MirArrayInstruction::DestroyNext { operation, .. } => {
                self.add_array_destruction(source, *operation, span, region)?
            }
            MirArrayInstruction::Allocate { array, .. }
            | MirArrayInstruction::AllocateElements { array, .. }
            | MirArrayInstruction::PublishShared { array, .. }
            | MirArrayInstruction::Adopt { array, .. }
            | MirArrayInstruction::AnchorBegin { array, .. }
            | MirArrayInstruction::Normalize { array, .. }
            | MirArrayInstruction::Offset { array, .. }
            | MirArrayInstruction::Boundary { array, .. } => self.add_runtime_entity(
                source,
                MirRuntimeEntity::ArrayLifecycle(*array),
                region,
                span,
            ),
            MirArrayInstruction::InitializeElement { .. }
            | MirArrayInstruction::CompleteElement { .. }
            | MirArrayInstruction::Publish { .. }
            | MirArrayInstruction::AnchorEnd { .. }
            | MirArrayInstruction::AliasBind { .. }
            | MirArrayInstruction::SliceBoundsCheck { .. }
            | MirArrayInstruction::SliceLengthCheck { .. } => {}
        }
        Ok(())
    }
}
