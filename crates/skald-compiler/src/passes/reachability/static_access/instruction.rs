//! Static-place extraction from MIR instructions.

use super::*;

impl MirDependencyExtractor<'_> {
    pub(in crate::passes::reachability) fn extract_static_instruction(
        &mut self,
        source: crate::mir::MirExecutionNode,
        definition: MirDefinitionRef<'_>,
        region: MirDependencyRegion,
        instruction: &MirInstruction,
    ) -> Result<(), MirDependencyExtractionError> {
        let span = instruction.span();
        match instruction {
            MirInstruction::StorageLive(_) | MirInstruction::StorageDead(_) => {}
            MirInstruction::Assign(assign) => {
                self.extract_static_rvalue(source, definition, region, &assign.rvalue, span)?;
            }
            MirInstruction::Call(call) => {
                if let Some(receiver) = &call.receiver {
                    match receiver {
                        MirCallReceiver::Method(receiver) => {
                            self.add_static_place(
                                source,
                                definition,
                                region,
                                &receiver.place,
                                StaticAccessKind::Borrow,
                                span,
                            )?;
                            self.add_static_origin(
                                source,
                                definition,
                                region,
                                &receiver.origin,
                                span,
                            )?;
                        }
                        MirCallReceiver::Interface(view) => {
                            self.add_static_view(source, definition, region, view, span)?;
                        }
                    }
                }
                for argument in &call.arguments {
                    self.add_static_argument(source, definition, region, argument, span)?;
                }
                if let Some(destination) = &call.destination {
                    self.add_static_place(
                        source,
                        definition,
                        region,
                        destination,
                        StaticAccessKind::Initialize,
                        span,
                    )?;
                }
            }
            MirInstruction::Cleanup(cleanup) => self.add_static_place(
                source,
                definition,
                region,
                &cleanup.destination,
                StaticAccessKind::Destroy,
                cleanup.span,
            )?,
            MirInstruction::Initialize(initialize) => {
                self.add_static_place(
                    source,
                    definition,
                    region,
                    &initialize.destination,
                    StaticAccessKind::Initialize,
                    span,
                )?;
                for argument in &initialize.arguments {
                    self.add_static_argument(source, definition, region, argument, span)?;
                }
            }
            MirInstruction::Store(store) => self.add_static_place(
                source,
                definition,
                region,
                &store.destination,
                StaticAccessKind::Write,
                span,
            )?,
            MirInstruction::CopyConstruct(copy) => {
                self.add_static_place(
                    source,
                    definition,
                    region,
                    &copy.destination,
                    StaticAccessKind::Initialize,
                    span,
                )?;
                self.add_static_place(
                    source,
                    definition,
                    region,
                    &copy.source,
                    StaticAccessKind::Read,
                    span,
                )?;
            }
            MirInstruction::CopyAssign(copy) => {
                self.add_static_place(
                    source,
                    definition,
                    region,
                    &copy.destination,
                    StaticAccessKind::Replace,
                    span,
                )?;
                self.add_static_place(
                    source,
                    definition,
                    region,
                    &copy.source,
                    StaticAccessKind::Read,
                    span,
                )?;
            }
            MirInstruction::EndFullExpression(end) => {
                for cleanup in &end.temporaries {
                    self.add_static_place(
                        source,
                        definition,
                        region,
                        &cleanup.destination,
                        StaticAccessKind::Destroy,
                        cleanup.span,
                    )?;
                }
            }
            MirInstruction::BindCheckedView(binding) => {
                self.add_static_view(source, definition, region, &binding.view, span)?;
            }
            MirInstruction::EndCheckedView(_) => {}
            MirInstruction::SharedAllocate(allocate) => {
                if let crate::mir::MirSharedAllocationMode::Copy { source: place } = &allocate.mode
                {
                    self.add_static_place(
                        source,
                        definition,
                        region,
                        place,
                        StaticAccessKind::Read,
                        span,
                    )?;
                }
            }
            MirInstruction::SharedInitialize(initialize) => {
                for argument in &initialize.arguments {
                    self.add_static_argument(source, definition, region, argument, span)?;
                }
            }
            MirInstruction::SharedPublish(_)
            | MirInstruction::SharedStatic(_)
            | MirInstruction::SharedAdopt(_)
            | MirInstruction::SharedCopy(_)
            | MirInstruction::SharedMove(_) => {}
            MirInstruction::SharedFieldCopy(copy) => self.add_static_place(
                source,
                definition,
                region,
                &copy.source,
                StaticAccessKind::Read,
                span,
            )?,
            MirInstruction::SharedCast(cast) => {
                self.add_static_shared_cast_source(source, definition, region, &cast.source, span)?;
            }
            MirInstruction::SharedRelease(_) => {}
            MirInstruction::SharedFieldInitialize(initialize) => self.add_static_place(
                source,
                definition,
                region,
                &initialize.destination,
                StaticAccessKind::Initialize,
                span,
            )?,
            MirInstruction::SharedFieldReplace(replace) => self.add_static_place(
                source,
                definition,
                region,
                &replace.destination,
                StaticAccessKind::Replace,
                span,
            )?,
            MirInstruction::StringInitialize(initialize) => self.add_static_place(
                source,
                definition,
                region,
                &initialize.destination,
                StaticAccessKind::Initialize,
                span,
            )?,
            MirInstruction::OptionalInitialize(initialize) => {
                self.add_static_place(
                    source,
                    definition,
                    region,
                    &initialize.destination,
                    StaticAccessKind::Initialize,
                    span,
                )?;
                self.add_static_optional_source(
                    source,
                    definition,
                    region,
                    &initialize.source,
                    span,
                )?;
            }
            MirInstruction::OptionalAssign(assign) => {
                self.add_static_place(
                    source,
                    definition,
                    region,
                    &assign.destination,
                    StaticAccessKind::Replace,
                    span,
                )?;
                self.add_static_optional_source(source, definition, region, &assign.source, span)?;
            }
            MirInstruction::AggregateOptionalInitialize(initialize) => {
                self.add_static_place(
                    source,
                    definition,
                    region,
                    &initialize.destination,
                    StaticAccessKind::Initialize,
                    span,
                )?;
                if let crate::mir::MirAggregateOptionalSource::Copy(copy) = &initialize.source {
                    self.add_static_place(
                        source,
                        definition,
                        region,
                        copy,
                        StaticAccessKind::Read,
                        span,
                    )?;
                }
            }
            MirInstruction::AggregateOptionalAssign(assign) => {
                self.add_static_place(
                    source,
                    definition,
                    region,
                    &assign.destination,
                    StaticAccessKind::Replace,
                    span,
                )?;
                if let crate::mir::MirAggregateOptionalSource::Copy(copy) = &assign.source {
                    self.add_static_place(
                        source,
                        definition,
                        region,
                        copy,
                        StaticAccessKind::Read,
                        span,
                    )?;
                }
            }
            MirInstruction::AggregateOptionalPublish(publish) => self.add_static_place(
                source,
                definition,
                region,
                &publish.destination,
                StaticAccessKind::Write,
                span,
            )?,
            MirInstruction::AggregateOptionalCleanup(cleanup) => self.add_static_place(
                source,
                definition,
                region,
                &cleanup.destination,
                StaticAccessKind::Destroy,
                span,
            )?,
            MirInstruction::ClassOptionalInitialize(initialize) => {
                self.add_static_place(
                    source,
                    definition,
                    region,
                    &initialize.destination,
                    StaticAccessKind::Initialize,
                    span,
                )?;
                self.add_static_class_optional_source(
                    source,
                    definition,
                    region,
                    &initialize.source,
                    span,
                )?;
            }
            MirInstruction::ClassOptionalAssign(assign) => {
                self.add_static_place(
                    source,
                    definition,
                    region,
                    &assign.destination,
                    StaticAccessKind::Replace,
                    span,
                )?;
                self.add_static_class_optional_source(
                    source,
                    definition,
                    region,
                    &assign.source,
                    span,
                )?;
            }
            MirInstruction::ClassOptionalPublish(publish) => self.add_static_place(
                source,
                definition,
                region,
                &publish.destination,
                StaticAccessKind::Write,
                span,
            )?,
            MirInstruction::ClassOptionalCleanup(cleanup) => self.add_static_place(
                source,
                definition,
                region,
                &cleanup.destination,
                StaticAccessKind::Destroy,
                span,
            )?,
            MirInstruction::EndOptionalView(end) => self.add_static_place(
                source,
                definition,
                region,
                &end.source,
                StaticAccessKind::Borrow,
                span,
            )?,
            MirInstruction::EndOptionalBoxView(_) => {}
            MirInstruction::OptionalSharedInitialize(initialize) => {
                self.add_static_place(
                    source,
                    definition,
                    region,
                    &initialize.destination,
                    StaticAccessKind::Initialize,
                    span,
                )?;
                self.add_static_optional_shared_source(
                    source,
                    definition,
                    region,
                    &initialize.source,
                    span,
                )?;
            }
            MirInstruction::OptionalSharedAssign(assign) => {
                self.add_static_place(
                    source,
                    definition,
                    region,
                    &assign.destination,
                    StaticAccessKind::Replace,
                    span,
                )?;
                self.add_static_optional_shared_source(
                    source,
                    definition,
                    region,
                    &assign.source,
                    span,
                )?;
            }
            MirInstruction::OptionalSharedCleanup(cleanup) => self.add_static_place(
                source,
                definition,
                region,
                &cleanup.destination,
                StaticAccessKind::Destroy,
                span,
            )?,
            MirInstruction::Array(array) => {
                self.extract_static_array_instruction(source, definition, region, array)?;
            }
            MirInstruction::Io(io) => match &io.operation {
                MirIoOperation::StandardHandle { .. } | MirIoOperation::Close { .. } => {}
                MirIoOperation::Open { path, .. } => self.add_static_place(
                    source,
                    definition,
                    region,
                    &path.place,
                    super::control::alias_access(path.access),
                    span,
                )?,
                MirIoOperation::Read { destination, .. } => self.add_static_place(
                    source,
                    definition,
                    region,
                    &destination.place,
                    StaticAccessKind::Borrow,
                    span,
                )?,
                MirIoOperation::Write { source: buffer, .. } => self.add_static_place(
                    source,
                    definition,
                    region,
                    &buffer.place,
                    StaticAccessKind::Borrow,
                    span,
                )?,
            },
        }
        Ok(())
    }
}
