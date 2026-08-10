use super::*;

impl Extractor<'_> {
    pub(super) fn extract_implicit_lifecycle(&mut self) {
        for class in self.program.classes.iter() {
            self.extract_copy_constructor(class.id, &class.copy_constructor, class.span);
            self.extract_copy_assignment(class.id, &class.copy_assignment, class.span);
            let source =
                StaticEffectNode::class(class.id, StaticClassLifecycleOperation::CompleteFinalizer);
            for step in &class.destruction.steps {
                match *step {
                    MirDestructionStep::UserBody(destructor) => self.add_edge(
                        source,
                        StaticEffectNode::Callable(destructor.into()),
                        StaticEffectEdgeKind::UserDestructor,
                        StaticEffectPhase::Destruction,
                        class
                            .destruction
                            .destructor
                            .as_ref()
                            .map_or(class.span, |declaration| declaration.span),
                    ),
                    MirDestructionStep::Field(field)
                    | MirDestructionStep::OptionalClassField(field) => {
                        let declaration = self
                            .program
                            .field(field)
                            .expect("verified destruction field must exist");
                        let target = match declaration.ty {
                            MirType::Class(class) => class,
                            MirType::Optional(optional) => self
                                .program
                                .optional_type(optional)
                                .and_then(crate::mir::MirOptionalType::inline_class)
                                .expect("verified optional-class destruction step must be typed"),
                            _ => unreachable!("verified class destruction step must be typed"),
                        };
                        self.add_edge(
                            source,
                            StaticEffectNode::class(
                                target,
                                StaticClassLifecycleOperation::CompleteFinalizer,
                            ),
                            StaticEffectEdgeKind::FieldFinalizer,
                            StaticEffectPhase::Destruction,
                            declaration.span,
                        );
                    }
                    MirDestructionStep::SharedField(field)
                    | MirDestructionStep::OptionalSharedField(field) => {
                        let declaration = self
                            .program
                            .field(field)
                            .expect("verified shared destruction field must exist");
                        let target = match declaration.ty {
                            MirType::Shared(target) => target,
                            MirType::Optional(optional) => self
                                .program
                                .optional_type(optional)
                                .and_then(crate::mir::MirOptionalType::shared_owner)
                                .expect("verified optional-owner destruction step must be typed"),
                            _ => unreachable!("verified shared destruction step must be typed"),
                        };
                        self.add_shared_finalizers(
                            source,
                            target,
                            StaticEffectEdgeKind::SharedFinalizer,
                            StaticEffectPhase::Destruction,
                            declaration.span,
                        );
                    }
                    MirDestructionStep::ArrayField(field) => {
                        let declaration = self
                            .program
                            .field(field)
                            .expect("verified array destruction field must exist");
                        let MirType::Array(array) = declaration.ty else {
                            unreachable!("verified array destruction step must be typed")
                        };
                        self.add_edge(
                            source,
                            StaticEffectNode::array(
                                array,
                                StaticArrayLifecycleOperation::Destruction,
                            ),
                            StaticEffectEdgeKind::ArrayDestruction,
                            StaticEffectPhase::Destruction,
                            declaration.span,
                        );
                    }
                    MirDestructionStep::Base(base) => self.add_edge(
                        source,
                        StaticEffectNode::class(
                            base,
                            StaticClassLifecycleOperation::CompleteFinalizer,
                        ),
                        StaticEffectEdgeKind::BaseFinalizer,
                        StaticEffectPhase::Destruction,
                        class
                            .direct_base
                            .map_or(class.span, |declaration| declaration.span),
                    ),
                }
            }
        }

        for array in self.program.array_types.iter() {
            let span = self.program.span;
            if let Some(operation) = array.lifecycle.default {
                let source =
                    StaticEffectNode::array(array.id, StaticArrayLifecycleOperation::Default);
                self.add_array_default(source, operation, span, StaticEffectPhase::ArrayLifecycle);
            }
            if let Some(operation) = array.lifecycle.copy {
                let source = StaticEffectNode::array(array.id, StaticArrayLifecycleOperation::Copy);
                self.add_array_copy(source, operation, span, StaticEffectPhase::ArrayLifecycle);
            }
            if let Some(operation) = array.lifecycle.assignment {
                let source =
                    StaticEffectNode::array(array.id, StaticArrayLifecycleOperation::Assignment);
                self.add_array_assignment(
                    source,
                    operation,
                    span,
                    StaticEffectPhase::ArrayLifecycle,
                );
            }
            let source =
                StaticEffectNode::array(array.id, StaticArrayLifecycleOperation::Destruction);
            self.add_array_destruction(
                source,
                array.lifecycle.destruction,
                span,
                StaticEffectPhase::ArrayLifecycle,
            );
        }
    }

    pub(super) fn extract_copy_constructor(
        &mut self,
        class: crate::identity::ClassId,
        capability: &MirCopyCapability<CopyConstructorId>,
        span: Span,
    ) {
        let source = StaticEffectNode::class(class, StaticClassLifecycleOperation::CopyConstructor);
        match capability {
            MirCopyCapability::User(copy) => {
                if let Some(base) = copy.base {
                    self.add_copy_constructor_edge(
                        source,
                        base.operation,
                        StaticEffectEdgeKind::BaseCopy,
                        StaticEffectPhase::Copy,
                        span,
                    );
                }
                self.add_edge(
                    source,
                    StaticEffectNode::Callable(copy.operation.into()),
                    StaticEffectEdgeKind::UserCopyBody,
                    StaticEffectPhase::Copy,
                    span,
                );
            }
            MirCopyCapability::Synthesized(copy) => {
                self.extract_synthesized_constructor(source, copy, span)
            }
            MirCopyCapability::Unavailable => {}
        }
    }

    pub(super) fn extract_synthesized_constructor(
        &mut self,
        source: StaticEffectNode,
        copy: &MirSynthesizedCopy<CopyConstructorId>,
        span: Span,
    ) {
        if let Some(base) = copy.base {
            self.add_copy_constructor_edge(
                source,
                base.operation,
                StaticEffectEdgeKind::BaseCopy,
                StaticEffectPhase::Copy,
                span,
            );
        }
        for field in &copy.fields {
            let field_span = self
                .program
                .field(field.field())
                .map_or(span, |declaration| declaration.span);
            match *field {
                MirSynthesizedFieldCopy::Class { operation, .. }
                | MirSynthesizedFieldCopy::OptionalClass { operation, .. } => {
                    self.add_copy_constructor_edge(
                        source,
                        operation,
                        StaticEffectEdgeKind::FieldCopy,
                        StaticEffectPhase::Copy,
                        field_span,
                    );
                }
                MirSynthesizedFieldCopy::Array { array, .. } => self.add_edge(
                    source,
                    StaticEffectNode::array(array, StaticArrayLifecycleOperation::Copy),
                    StaticEffectEdgeKind::ArrayCopy,
                    StaticEffectPhase::Copy,
                    field_span,
                ),
                MirSynthesizedFieldCopy::Primitive { .. }
                | MirSynthesizedFieldCopy::OptionalPrimitive { .. }
                | MirSynthesizedFieldCopy::Shared { .. }
                | MirSynthesizedFieldCopy::OptionalShared { .. } => {}
            }
        }
    }

    pub(super) fn extract_copy_assignment(
        &mut self,
        class: crate::identity::ClassId,
        capability: &MirCopyCapability<CopyAssignmentId>,
        span: Span,
    ) {
        let source = StaticEffectNode::class(class, StaticClassLifecycleOperation::CopyAssignment);
        match capability {
            MirCopyCapability::User(copy) => {
                if let Some(base) = copy.base {
                    self.add_copy_assignment_edge(
                        source,
                        base.operation,
                        StaticEffectEdgeKind::BaseCopy,
                        StaticEffectPhase::Copy,
                        span,
                    );
                }
                self.add_edge(
                    source,
                    StaticEffectNode::Callable(copy.operation.into()),
                    StaticEffectEdgeKind::UserCopyBody,
                    StaticEffectPhase::Copy,
                    span,
                );
            }
            MirCopyCapability::Synthesized(copy) => {
                self.extract_synthesized_assignment(source, copy, span)
            }
            MirCopyCapability::Unavailable => {}
        }
    }

    pub(super) fn extract_synthesized_assignment(
        &mut self,
        source: StaticEffectNode,
        copy: &MirSynthesizedCopy<CopyAssignmentId>,
        span: Span,
    ) {
        if let Some(base) = copy.base {
            self.add_copy_assignment_edge(
                source,
                base.operation,
                StaticEffectEdgeKind::BaseCopy,
                StaticEffectPhase::Copy,
                span,
            );
        }
        for field in &copy.fields {
            let field_span = self
                .program
                .field(field.field())
                .map_or(span, |declaration| declaration.span);
            match *field {
                MirSynthesizedFieldCopy::Class { operation, .. } => {
                    self.add_copy_assignment_edge(
                        source,
                        operation,
                        StaticEffectEdgeKind::FieldCopy,
                        StaticEffectPhase::Copy,
                        field_span,
                    );
                }
                MirSynthesizedFieldCopy::OptionalClass {
                    class, operation, ..
                } => {
                    self.add_copy_constructor_for_class(
                        source,
                        class,
                        StaticEffectEdgeKind::FieldCopy,
                        StaticEffectPhase::Copy,
                        field_span,
                    );
                    self.add_copy_assignment_edge(
                        source,
                        operation,
                        StaticEffectEdgeKind::FieldCopy,
                        StaticEffectPhase::Copy,
                        field_span,
                    );
                    self.add_complete_finalizer(
                        source,
                        class,
                        StaticEffectEdgeKind::OptionalCleanup,
                        StaticEffectPhase::Copy,
                        field_span,
                    );
                }
                MirSynthesizedFieldCopy::Array { array, .. } => self.add_edge(
                    source,
                    StaticEffectNode::array(array, StaticArrayLifecycleOperation::Assignment),
                    StaticEffectEdgeKind::ArrayAssignment,
                    StaticEffectPhase::Copy,
                    field_span,
                ),
                MirSynthesizedFieldCopy::Primitive { .. }
                | MirSynthesizedFieldCopy::OptionalPrimitive { .. }
                | MirSynthesizedFieldCopy::Shared { .. }
                | MirSynthesizedFieldCopy::OptionalShared { .. } => {}
            }
        }
    }
}
