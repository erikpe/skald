//! Class copy, initialization, and finalization dependencies.

use crate::{
    identity::{ClassId, CopyAssignmentId, CopyConstructorId, InitializerId},
    mir::{
        MirArrayLifecycleOperation, MirClassLifecycleOperation, MirCopyCapability,
        MirDestructionStep, MirExecutionNode, MirSelectedCopyOperation, MirSynthesizedCopy,
        MirSynthesizedFieldCopy, MirType,
    },
    source::Span,
};

use super::super::{
    extract::MirDependencyExtractor, MirDependencyEdgeKind, MirDependencyExtractionError,
    MirDependencyRegion, MirDependencyTarget,
};

pub(super) fn class_finalizer_target(
    program: &crate::mir::MirProgram,
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

impl MirDependencyExtractor<'_> {
    pub(super) fn extract_class_lifecycle(&mut self) -> Result<(), MirDependencyExtractionError> {
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
        Ok(())
    }

    pub(in crate::passes::reachability) fn add_copy_constructor(
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

    pub(super) fn add_copy_constructor_for_class(
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

    pub(in crate::passes::reachability) fn add_copy_assignment(
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

    pub(in crate::passes::reachability) fn add_complete_finalizer(
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

    pub(in crate::passes::reachability) fn add_initializer(
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
}
