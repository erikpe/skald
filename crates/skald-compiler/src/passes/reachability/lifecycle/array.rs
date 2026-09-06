//! Array default, copy, assignment, and destruction dependencies.

use crate::{
    identity::ArrayTypeId,
    mir::{
        MirArrayAssignElement, MirArrayCopyElement, MirArrayDefaultElement, MirArrayDestroyElement,
        MirArrayInstruction, MirArrayLifecycleOperation, MirExecutionNode, MirProgram,
        MirSharedTarget,
    },
    source::Span,
};

use super::{
    super::{
        extract::MirDependencyExtractor, MirDependencyEdgeKind, MirDependencyExtractionError,
        MirDependencyRegion, MirDependencyTarget, MirRuntimeEntity,
    },
    MirLifecycleDependency,
};

pub(super) fn resolve_array_destruction_dependencies(
    program: &MirProgram,
    array: ArrayTypeId,
) -> Result<Vec<MirLifecycleDependency>, MirDependencyExtractionError> {
    if program.array_type(array).is_none() {
        return Err(MirDependencyExtractionError::UnknownArrayType(array));
    }
    Ok(vec![
        MirLifecycleDependency {
            target: MirDependencyTarget::Execution(MirExecutionNode::array(
                array,
                MirArrayLifecycleOperation::Destruction,
            )),
            kind: MirDependencyEdgeKind::ArrayDestruction,
        },
        MirLifecycleDependency {
            target: MirDependencyTarget::RuntimeEntity(MirRuntimeEntity::ArrayLifecycle(array)),
            kind: MirDependencyEdgeKind::RuntimeEntityReference,
        },
    ])
}

impl MirDependencyExtractor<'_> {
    pub(super) fn extract_array_lifecycle(&mut self) -> Result<(), MirDependencyExtractionError> {
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

    pub(super) fn add_array_lifecycle(
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

    pub(in crate::passes::reachability) fn extract_array_instruction(
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
            | MirArrayInstruction::BeginIndexed { .. }
            | MirArrayInstruction::BindIndexed { .. }
            | MirArrayInstruction::InitializeIndexedElement { .. }
            | MirArrayInstruction::AdvanceIndexedElement { .. }
            | MirArrayInstruction::EndIndexedElement { .. }
            | MirArrayInstruction::CompleteIndexed { .. }
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
