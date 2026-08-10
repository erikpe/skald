use super::*;

impl Extractor<'_> {
    pub(super) fn add_copy_constructor_edge(
        &mut self,
        source: StaticEffectNode,
        operation: MirSelectedCopyOperation<CopyConstructorId>,
        kind: StaticEffectEdgeKind,
        phase: StaticEffectPhase,
        span: Span,
    ) {
        let class = match operation {
            MirSelectedCopyOperation::User(id) => id.class(),
            MirSelectedCopyOperation::Synthesized(class) => class,
        };
        self.add_copy_constructor_for_class(source, class, kind, phase, span);
    }

    pub(super) fn add_copy_constructor_for_class(
        &mut self,
        source: StaticEffectNode,
        class: crate::identity::ClassId,
        kind: StaticEffectEdgeKind,
        phase: StaticEffectPhase,
        span: Span,
    ) {
        self.add_edge(
            source,
            StaticEffectNode::class(class, StaticClassLifecycleOperation::CopyConstructor),
            kind,
            phase,
            span,
        );
    }

    pub(super) fn add_copy_assignment_edge(
        &mut self,
        source: StaticEffectNode,
        operation: MirSelectedCopyOperation<CopyAssignmentId>,
        kind: StaticEffectEdgeKind,
        phase: StaticEffectPhase,
        span: Span,
    ) {
        let class = match operation {
            MirSelectedCopyOperation::User(id) => id.class(),
            MirSelectedCopyOperation::Synthesized(class) => class,
        };
        self.add_edge(
            source,
            StaticEffectNode::class(class, StaticClassLifecycleOperation::CopyAssignment),
            kind,
            phase,
            span,
        );
    }

    pub(super) fn add_complete_finalizer(
        &mut self,
        source: StaticEffectNode,
        class: crate::identity::ClassId,
        kind: StaticEffectEdgeKind,
        phase: StaticEffectPhase,
        span: Span,
    ) {
        self.add_edge(
            source,
            StaticEffectNode::class(class, StaticClassLifecycleOperation::CompleteFinalizer),
            kind,
            phase,
            span,
        );
    }

    pub(super) fn add_shared_finalizers(
        &mut self,
        source: StaticEffectNode,
        target: MirSharedTarget,
        kind: StaticEffectEdgeKind,
        phase: StaticEffectPhase,
        span: Span,
    ) {
        for target in self.program.shared_lifecycle_targets(target) {
            let node = match target {
                PreliminaryMirSharedLifecycleTarget::Class(class) => {
                    StaticEffectNode::class(class, StaticClassLifecycleOperation::CompleteFinalizer)
                }
                PreliminaryMirSharedLifecycleTarget::Array(array) => {
                    StaticEffectNode::array(array, StaticArrayLifecycleOperation::Destruction)
                }
            };
            self.add_edge(source, node, kind, phase, span);
        }
    }

    pub(super) fn add_array_default(
        &mut self,
        source: StaticEffectNode,
        operation: MirArrayDefaultElement,
        span: Span,
        phase: StaticEffectPhase,
    ) {
        match operation {
            MirArrayDefaultElement::Class { initializer, .. }
            | MirArrayDefaultElement::SharedClass { initializer, .. } => self.add_edge(
                source,
                StaticEffectNode::Callable(initializer.into()),
                StaticEffectEdgeKind::ArrayDefault,
                phase,
                span,
            ),
            MirArrayDefaultElement::ArrayEmpty(array)
            | MirArrayDefaultElement::SharedArrayEmpty(array) => self.add_edge(
                source,
                StaticEffectNode::array(array, StaticArrayLifecycleOperation::Default),
                StaticEffectEdgeKind::ArrayDefault,
                phase,
                span,
            ),
            MirArrayDefaultElement::Primitive | MirArrayDefaultElement::OptionalAbsent => {}
        }
    }

    pub(super) fn add_array_copy(
        &mut self,
        source: StaticEffectNode,
        operation: MirArrayCopyElement,
        span: Span,
        phase: StaticEffectPhase,
    ) {
        match operation {
            MirArrayCopyElement::Class { operation, .. }
            | MirArrayCopyElement::OptionalClass { operation, .. } => self
                .add_copy_constructor_edge(
                    source,
                    operation,
                    StaticEffectEdgeKind::ArrayCopy,
                    phase,
                    span,
                ),
            MirArrayCopyElement::Array(array) => self.add_edge(
                source,
                StaticEffectNode::array(array, StaticArrayLifecycleOperation::Copy),
                StaticEffectEdgeKind::ArrayCopy,
                phase,
                span,
            ),
            MirArrayCopyElement::Primitive
            | MirArrayCopyElement::OptionalPrimitive
            | MirArrayCopyElement::Shared(_)
            | MirArrayCopyElement::OptionalShared(_) => {}
            MirArrayCopyElement::Optional(optional) => {
                self.add_optional_copy_edges(source, optional, phase, span)
            }
        }
    }

    pub(super) fn add_array_assignment(
        &mut self,
        source: StaticEffectNode,
        operation: MirArrayAssignElement,
        span: Span,
        phase: StaticEffectPhase,
    ) {
        match operation {
            MirArrayAssignElement::Class { operation, .. } => self.add_copy_assignment_edge(
                source,
                operation,
                StaticEffectEdgeKind::ArrayAssignment,
                phase,
                span,
            ),
            MirArrayAssignElement::OptionalClass {
                class,
                copy_constructor,
                copy_assignment,
                ..
            } => {
                self.add_copy_constructor_edge(
                    source,
                    copy_constructor,
                    StaticEffectEdgeKind::ArrayAssignment,
                    phase,
                    span,
                );
                self.add_copy_assignment_edge(
                    source,
                    copy_assignment,
                    StaticEffectEdgeKind::ArrayAssignment,
                    phase,
                    span,
                );
                self.add_complete_finalizer(
                    source,
                    class,
                    StaticEffectEdgeKind::OptionalCleanup,
                    phase,
                    span,
                );
            }
            MirArrayAssignElement::Array(array) => self.add_edge(
                source,
                StaticEffectNode::array(array, StaticArrayLifecycleOperation::Assignment),
                StaticEffectEdgeKind::ArrayAssignment,
                phase,
                span,
            ),
            MirArrayAssignElement::Shared(target)
            | MirArrayAssignElement::OptionalShared(target) => self.add_shared_finalizers(
                source,
                target,
                StaticEffectEdgeKind::SharedFinalizer,
                phase,
                span,
            ),
            MirArrayAssignElement::Primitive | MirArrayAssignElement::OptionalPrimitive => {}
            MirArrayAssignElement::Optional(optional) => {
                self.add_optional_assignment_edges(source, optional, phase, span)
            }
        }
    }

    pub(super) fn add_array_destruction(
        &mut self,
        source: StaticEffectNode,
        operation: MirArrayDestroyElement,
        span: Span,
        phase: StaticEffectPhase,
    ) {
        match operation {
            MirArrayDestroyElement::Class(class) | MirArrayDestroyElement::OptionalClass(class) => {
                self.add_complete_finalizer(
                    source,
                    class,
                    StaticEffectEdgeKind::ArrayDestruction,
                    phase,
                    span,
                )
            }
            MirArrayDestroyElement::Array(array) => self.add_edge(
                source,
                StaticEffectNode::array(array, StaticArrayLifecycleOperation::Destruction),
                StaticEffectEdgeKind::ArrayDestruction,
                phase,
                span,
            ),
            MirArrayDestroyElement::Shared(target)
            | MirArrayDestroyElement::OptionalShared(target) => self.add_shared_finalizers(
                source,
                target,
                StaticEffectEdgeKind::SharedFinalizer,
                phase,
                span,
            ),
            MirArrayDestroyElement::Trivial => {}
            MirArrayDestroyElement::Optional(optional) => {
                self.add_optional_cleanup_edges(source, optional, phase, span)
            }
        }
    }

    pub(super) fn add_optional_copy_edges(
        &mut self,
        source: StaticEffectNode,
        optional: crate::identity::OptionalTypeId,
        phase: StaticEffectPhase,
        span: Span,
    ) {
        let Some(plan) = self
            .program
            .optional_type(optional)
            .and_then(|ty| ty.lifecycle.copy)
        else {
            return;
        };
        match plan {
            crate::mir::MirOptionalCopyPlan::Class { operation, .. } => self
                .add_copy_constructor_edge(
                    source,
                    operation,
                    StaticEffectEdgeKind::ArrayCopy,
                    phase,
                    span,
                ),
            crate::mir::MirOptionalCopyPlan::Optional(nested) => {
                self.add_optional_copy_edges(source, nested, phase, span)
            }
            crate::mir::MirOptionalCopyPlan::Array(array) => self.add_edge(
                source,
                StaticEffectNode::array(array, StaticArrayLifecycleOperation::Copy),
                StaticEffectEdgeKind::ArrayCopy,
                phase,
                span,
            ),
            crate::mir::MirOptionalCopyPlan::Trivial
            | crate::mir::MirOptionalCopyPlan::Shared(_) => {}
        }
    }

    pub(super) fn add_optional_assignment_edges(
        &mut self,
        source: StaticEffectNode,
        optional: crate::identity::OptionalTypeId,
        phase: StaticEffectPhase,
        span: Span,
    ) {
        let Some(plan) = self
            .program
            .optional_type(optional)
            .and_then(|ty| ty.lifecycle.assignment)
        else {
            return;
        };
        match plan {
            crate::mir::MirOptionalAssignmentPlan::Class {
                class,
                copy_constructor,
                copy_assignment,
            } => {
                self.add_copy_constructor_edge(
                    source,
                    copy_constructor,
                    StaticEffectEdgeKind::ArrayAssignment,
                    phase,
                    span,
                );
                self.add_copy_assignment_edge(
                    source,
                    copy_assignment,
                    StaticEffectEdgeKind::ArrayAssignment,
                    phase,
                    span,
                );
                self.add_complete_finalizer(
                    source,
                    class,
                    StaticEffectEdgeKind::OptionalCleanup,
                    phase,
                    span,
                );
            }
            crate::mir::MirOptionalAssignmentPlan::Optional(nested) => {
                self.add_optional_assignment_edges(source, nested, phase, span)
            }
            crate::mir::MirOptionalAssignmentPlan::Array(array) => self.add_edge(
                source,
                StaticEffectNode::array(array, StaticArrayLifecycleOperation::Assignment),
                StaticEffectEdgeKind::ArrayAssignment,
                phase,
                span,
            ),
            crate::mir::MirOptionalAssignmentPlan::Shared(target) => self.add_shared_finalizers(
                source,
                target,
                StaticEffectEdgeKind::SharedFinalizer,
                phase,
                span,
            ),
            crate::mir::MirOptionalAssignmentPlan::Trivial => {}
        }
    }

    pub(super) fn add_optional_cleanup_edges(
        &mut self,
        source: StaticEffectNode,
        optional: crate::identity::OptionalTypeId,
        phase: StaticEffectPhase,
        span: Span,
    ) {
        let Some(metadata) = self.program.optional_type(optional) else {
            return;
        };
        match metadata.lifecycle.cleanup {
            crate::mir::MirOptionalCleanupPlan::Class(class) => self.add_complete_finalizer(
                source,
                class,
                StaticEffectEdgeKind::OptionalCleanup,
                phase,
                span,
            ),
            crate::mir::MirOptionalCleanupPlan::Optional(nested) => {
                self.add_optional_cleanup_edges(source, nested, phase, span)
            }
            crate::mir::MirOptionalCleanupPlan::Array(array) => self.add_edge(
                source,
                StaticEffectNode::array(array, StaticArrayLifecycleOperation::Destruction),
                StaticEffectEdgeKind::ArrayDestruction,
                phase,
                span,
            ),
            crate::mir::MirOptionalCleanupPlan::Shared(target) => self.add_shared_finalizers(
                source,
                target,
                StaticEffectEdgeKind::SharedFinalizer,
                phase,
                span,
            ),
            crate::mir::MirOptionalCleanupPlan::Trivial => {}
        }
    }
}
