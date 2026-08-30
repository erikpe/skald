//! MIR-instruction effect and edge extraction.

use super::*;

impl Extractor<'_> {
    pub(super) fn extract_instruction(
        &mut self,
        source: StaticEffectNode,
        definition: MirDefinitionRef<'_>,
        phase: StaticEffectPhase,
        instruction: &MirInstruction,
    ) {
        let span = instruction.span();
        match instruction {
            MirInstruction::StorageLive(_) | MirInstruction::StorageDead(_) => {}
            MirInstruction::Assign(assign) => {
                self.add_rvalue(source, definition, phase, &assign.rvalue, span)
            }
            MirInstruction::Call(call) => {
                if let Some(receiver) = &call.receiver {
                    match receiver {
                        MirCallReceiver::Method(receiver) => {
                            self.add_place(
                                source,
                                definition,
                                phase,
                                &receiver.place,
                                StaticAccessKind::Borrow,
                                span,
                            );
                            self.add_origin(source, definition, phase, &receiver.origin, span);
                        }
                        MirCallReceiver::Interface(view) => {
                            self.add_view(source, definition, phase, view, span)
                        }
                    }
                }
                for argument in &call.arguments {
                    self.add_argument(source, definition, phase, argument, span);
                }
                if let Some(destination) = &call.destination {
                    self.add_place(
                        source,
                        definition,
                        phase,
                        destination,
                        StaticAccessKind::Initialize,
                        span,
                    );
                }
            }
            MirInstruction::Cleanup(cleanup) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &cleanup.destination,
                    StaticAccessKind::Destroy,
                    cleanup.span,
                );
            }
            MirInstruction::Initialize(initialize) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &initialize.destination,
                    StaticAccessKind::Initialize,
                    span,
                );
                for argument in &initialize.arguments {
                    self.add_argument(source, definition, phase, argument, span);
                }
            }
            MirInstruction::Store(store) => self.add_place(
                source,
                definition,
                phase,
                &store.destination,
                StaticAccessKind::Write,
                span,
            ),
            MirInstruction::CopyConstruct(copy) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &copy.destination,
                    StaticAccessKind::Initialize,
                    span,
                );
                self.add_place(
                    source,
                    definition,
                    phase,
                    &copy.source,
                    StaticAccessKind::Read,
                    span,
                );
            }
            MirInstruction::CopyAssign(copy) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &copy.destination,
                    StaticAccessKind::Replace,
                    span,
                );
                self.add_place(
                    source,
                    definition,
                    phase,
                    &copy.source,
                    StaticAccessKind::Read,
                    span,
                );
            }
            MirInstruction::EndFullExpression(end) => {
                for cleanup in &end.temporaries {
                    self.add_place(
                        source,
                        definition,
                        phase,
                        &cleanup.destination,
                        StaticAccessKind::Destroy,
                        cleanup.span,
                    );
                }
            }
            MirInstruction::BindCheckedView(binding) => {
                self.add_view(source, definition, phase, &binding.view, span)
            }
            MirInstruction::EndCheckedView(_) => {}
            MirInstruction::SharedAllocate(allocate) => {
                if let crate::mir::MirSharedAllocationMode::Copy { source: place } = &allocate.mode
                {
                    self.add_place(
                        source,
                        definition,
                        phase,
                        place,
                        StaticAccessKind::Read,
                        span,
                    );
                }
            }
            MirInstruction::SharedInitialize(initialize) => {
                for argument in &initialize.arguments {
                    self.add_argument(source, definition, phase, argument, span);
                }
            }
            MirInstruction::SharedPublish(_)
            | MirInstruction::SharedStatic(_)
            | MirInstruction::SharedAdopt(_)
            | MirInstruction::SharedCopy(_)
            | MirInstruction::SharedMove(_) => {}
            MirInstruction::SharedFieldCopy(copy) => self.add_place(
                source,
                definition,
                phase,
                &copy.source,
                StaticAccessKind::Read,
                span,
            ),
            MirInstruction::SharedCast(cast) => {
                self.add_shared_cast_source(source, definition, phase, &cast.source, span)
            }
            MirInstruction::SharedRelease(_) => {}
            MirInstruction::SharedFieldInitialize(initialize) => self.add_place(
                source,
                definition,
                phase,
                &initialize.destination,
                StaticAccessKind::Initialize,
                span,
            ),
            MirInstruction::SharedFieldReplace(replace) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &replace.destination,
                    StaticAccessKind::Replace,
                    span,
                );
            }
            MirInstruction::StringInitialize(initialize) => self.add_place(
                source,
                definition,
                phase,
                &initialize.destination,
                StaticAccessKind::Initialize,
                span,
            ),
            MirInstruction::OptionalInitialize(initialize) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &initialize.destination,
                    StaticAccessKind::Initialize,
                    span,
                );
                self.add_optional_source(source, definition, phase, &initialize.source, span);
            }
            MirInstruction::OptionalAssign(assign) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &assign.destination,
                    StaticAccessKind::Replace,
                    span,
                );
                self.add_optional_source(source, definition, phase, &assign.source, span);
            }
            MirInstruction::AggregateOptionalInitialize(initialize) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &initialize.destination,
                    StaticAccessKind::Initialize,
                    span,
                );
                if let crate::mir::MirAggregateOptionalSource::Copy(copy) = &initialize.source {
                    self.add_place(
                        source,
                        definition,
                        phase,
                        copy,
                        StaticAccessKind::Read,
                        span,
                    );
                }
            }
            MirInstruction::AggregateOptionalAssign(assign) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &assign.destination,
                    StaticAccessKind::Replace,
                    span,
                );
                if let crate::mir::MirAggregateOptionalSource::Copy(copy) = &assign.source {
                    self.add_place(
                        source,
                        definition,
                        phase,
                        copy,
                        StaticAccessKind::Read,
                        span,
                    );
                }
            }
            MirInstruction::AggregateOptionalPublish(publish) => self.add_place(
                source,
                definition,
                phase,
                &publish.destination,
                StaticAccessKind::Write,
                span,
            ),
            MirInstruction::AggregateOptionalCleanup(cleanup) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &cleanup.destination,
                    StaticAccessKind::Destroy,
                    span,
                );
            }
            MirInstruction::ClassOptionalInitialize(initialize) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &initialize.destination,
                    StaticAccessKind::Initialize,
                    span,
                );
                self.add_class_optional_source(source, definition, phase, &initialize.source, span);
            }
            MirInstruction::ClassOptionalAssign(assign) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &assign.destination,
                    StaticAccessKind::Replace,
                    span,
                );
                self.add_class_optional_source(source, definition, phase, &assign.source, span);
            }
            MirInstruction::ClassOptionalPublish(publish) => self.add_place(
                source,
                definition,
                phase,
                &publish.destination,
                StaticAccessKind::Write,
                span,
            ),
            MirInstruction::ClassOptionalCleanup(cleanup) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &cleanup.destination,
                    StaticAccessKind::Destroy,
                    span,
                );
            }
            MirInstruction::EndOptionalView(end) => self.add_place(
                source,
                definition,
                phase,
                &end.source,
                StaticAccessKind::Borrow,
                span,
            ),
            MirInstruction::EndOptionalBoxView(_) => {}
            MirInstruction::OptionalSharedInitialize(initialize) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &initialize.destination,
                    StaticAccessKind::Initialize,
                    span,
                );
                self.add_optional_shared_source(
                    source,
                    definition,
                    phase,
                    &initialize.source,
                    span,
                );
            }
            MirInstruction::OptionalSharedAssign(assign) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &assign.destination,
                    StaticAccessKind::Replace,
                    span,
                );
                self.add_optional_shared_source(source, definition, phase, &assign.source, span);
            }
            MirInstruction::OptionalSharedCleanup(cleanup) => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    &cleanup.destination,
                    StaticAccessKind::Destroy,
                    span,
                );
            }
            MirInstruction::Array(array) => {
                self.extract_array_instruction(source, definition, phase, array)
            }
            MirInstruction::Io(io) => match &io.operation {
                MirIoOperation::StandardHandle { .. } | MirIoOperation::Close { .. } => {}
                MirIoOperation::Open { path, .. } => self.add_place(
                    source,
                    definition,
                    phase,
                    &path.place,
                    super::control::alias_access(path.access),
                    span,
                ),
                MirIoOperation::Read { destination, .. } => self.add_place(
                    source,
                    definition,
                    phase,
                    &destination.place,
                    StaticAccessKind::Borrow,
                    span,
                ),
                MirIoOperation::Write { source: buffer, .. } => self.add_place(
                    source,
                    definition,
                    phase,
                    &buffer.place,
                    StaticAccessKind::Borrow,
                    span,
                ),
            },
        }
    }
}
