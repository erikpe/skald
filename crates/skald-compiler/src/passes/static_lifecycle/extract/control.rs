use super::*;

impl Extractor<'_> {
    pub(super) fn extract_bodies(&mut self) {
        for definition in self
            .program
            .definitions
            .iter()
            .map(MirDefinitionRef::Function)
            .chain(
                self.program
                    .member_definitions
                    .iter()
                    .map(MirDefinitionRef::Member),
            )
            .chain(self.initializers.iter().map(MirDefinitionRef::from))
        {
            let source = StaticEffectNode::Callable(definition.callable());
            let after_publication = after_publication_blocks(definition);
            for block in &definition.body().blocks {
                let phase = body_phase(definition, &after_publication, block.id);
                for instruction in &block.instructions {
                    self.extract_instruction(source, definition, phase, instruction);
                }
                if let Some(terminator) = &block.terminator {
                    self.extract_terminator(source, definition, phase, terminator);
                }
            }
        }
    }

    pub(super) fn extract_terminator(
        &mut self,
        source: StaticEffectNode,
        definition: MirDefinitionRef<'_>,
        phase: StaticEffectPhase,
        terminator: &MirTerminator,
    ) {
        let span = terminator.span();
        match terminator {
            MirTerminator::Return { .. }
            | MirTerminator::ReturnShared { .. }
            | MirTerminator::ReturnOptionalShared { .. }
            | MirTerminator::Goto { .. }
            | MirTerminator::Branch { .. }
            | MirTerminator::ShiftCountCheck { .. }
            | MirTerminator::IntegerDivisorCheck { .. }
            | MirTerminator::PrimitiveCastRangeCheck { .. }
            | MirTerminator::ArrayPositionCheck { .. }
            | MirTerminator::ArrayOperationCheck { .. }
            | MirTerminator::ArrayLoop { .. }
            | MirTerminator::Terminate { .. } => {}
            MirTerminator::Panic { message, .. } => self.add_place(
                source,
                definition,
                phase,
                message,
                StaticAccessKind::Borrow,
                span,
            ),
            MirTerminator::CheckedCast { binding, .. } => {
                self.add_view(source, definition, phase, &binding.view, span)
            }
            MirTerminator::SharedCast { cast, .. } => {
                self.add_shared_cast_source(source, definition, phase, &cast.source, span)
            }
            MirTerminator::OptionalUnwrap { source: place, .. }
            | MirTerminator::CheckOptionalMutation { source: place, .. } => self.add_place(
                source,
                definition,
                phase,
                place,
                StaticAccessKind::Borrow,
                span,
            ),
            MirTerminator::OptionalSharedUnwrap { unwrap, .. } => self.add_place(
                source,
                definition,
                phase,
                &unwrap.source,
                StaticAccessKind::Borrow,
                span,
            ),
            MirTerminator::BeginOptionalView { begin, .. } => self.add_place(
                source,
                definition,
                phase,
                &begin.source,
                StaticAccessKind::Borrow,
                span,
            ),
        }
    }

    pub(super) fn extract_array_instruction(
        &mut self,
        source: StaticEffectNode,
        definition: MirDefinitionRef<'_>,
        phase: StaticEffectPhase,
        instruction: &MirArrayInstruction,
    ) {
        let span = instruction.span();
        match instruction {
            MirArrayInstruction::Allocate { .. }
            | MirArrayInstruction::AllocateElements { .. }
            | MirArrayInstruction::InitializeElement { .. }
            | MirArrayInstruction::CompleteElement { .. }
            | MirArrayInstruction::Publish { .. }
            | MirArrayInstruction::PublishShared { .. }
            | MirArrayInstruction::AnchorEnd { .. }
            | MirArrayInstruction::SliceBoundsCheck { .. } => {}
            MirArrayInstruction::InitializeNext { operation, .. } => {
                self.add_array_default(source, *operation, span, phase)
            }
            MirArrayInstruction::CopyNext {
                source: place,
                operation,
                ..
            } => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    place,
                    StaticAccessKind::Read,
                    span,
                );
                self.add_array_copy(source, *operation, span, phase);
            }
            MirArrayInstruction::Adopt { destination, .. } => self.add_place(
                source,
                definition,
                phase,
                destination,
                StaticAccessKind::Initialize,
                span,
            ),
            MirArrayInstruction::Replace {
                destination, array, ..
            } => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    destination,
                    StaticAccessKind::Replace,
                    span,
                );
                self.add_edge(
                    source,
                    StaticEffectNode::array(*array, StaticArrayLifecycleOperation::Destruction),
                    StaticEffectEdgeKind::ArrayDestruction,
                    phase,
                    span,
                );
            }
            MirArrayInstruction::ElementAssign {
                destination,
                source: source_place,
                operation,
                ..
            }
            | MirArrayInstruction::SliceAssignNext {
                destination,
                source: source_place,
                operation,
                ..
            } => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    destination,
                    StaticAccessKind::Replace,
                    span,
                );
                self.add_place(
                    source,
                    definition,
                    phase,
                    source_place,
                    StaticAccessKind::Read,
                    span,
                );
                self.add_array_assignment(source, *operation, span, phase);
            }
            MirArrayInstruction::DestroyNext {
                owner, operation, ..
            } => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    owner,
                    StaticAccessKind::Destroy,
                    span,
                );
                self.add_array_destruction(source, *operation, span, phase);
            }
            MirArrayInstruction::Release { owner, array, .. } => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    owner,
                    StaticAccessKind::Destroy,
                    span,
                );
                self.add_edge(
                    source,
                    StaticEffectNode::array(*array, StaticArrayLifecycleOperation::Destruction),
                    StaticEffectEdgeKind::ArrayDestruction,
                    phase,
                    span,
                );
            }
            MirArrayInstruction::AnchorBegin { owner, .. }
            | MirArrayInstruction::Normalize { owner, .. }
            | MirArrayInstruction::Offset { owner, .. }
            | MirArrayInstruction::Boundary { owner, .. } => self.add_place(
                source,
                definition,
                phase,
                owner,
                StaticAccessKind::Borrow,
                span,
            ),
            MirArrayInstruction::AliasBind { source: place, .. } => self.add_place(
                source,
                definition,
                phase,
                place,
                StaticAccessKind::Borrow,
                span,
            ),
            MirArrayInstruction::SliceCopy {
                source: place,
                operation,
                ..
            } => {
                self.add_place(
                    source,
                    definition,
                    phase,
                    place,
                    StaticAccessKind::Read,
                    span,
                );
                self.add_array_copy(source, *operation, span, phase);
            }
            MirArrayInstruction::SliceLengthCheck { source: place, .. } => self.add_place(
                source,
                definition,
                phase,
                place,
                StaticAccessKind::Borrow,
                span,
            ),
        }
    }
}

fn after_publication_blocks(definition: MirDefinitionRef<'_>) -> BTreeSet<crate::mir::BlockId> {
    let MirDefinitionRef::StaticInitializer(initializer) = definition else {
        return BTreeSet::new();
    };
    let mut found = BTreeSet::new();
    let mut pending = vec![initializer.publication.cleanup_entry];
    while let Some(block) = pending.pop() {
        if !found.insert(block) {
            continue;
        }
        if let Some(terminator) = initializer
            .block(block)
            .and_then(|block| block.terminator.as_ref())
        {
            pending.extend(terminator.successors());
        }
    }
    found
}

fn body_phase(
    definition: MirDefinitionRef<'_>,
    after_publication: &BTreeSet<crate::mir::BlockId>,
    block: crate::mir::BlockId,
) -> StaticEffectPhase {
    match definition {
        MirDefinitionRef::StaticInitializer(_) if after_publication.contains(&block) => {
            StaticEffectPhase::InitializerAfterPublication
        }
        MirDefinitionRef::StaticInitializer(_) => StaticEffectPhase::InitializerBeforePublication,
        MirDefinitionRef::Function(_) | MirDefinitionRef::Member(_) => StaticEffectPhase::Ordinary,
    }
}

pub(super) fn alias_access(access: MirAliasAccess) -> StaticAccessKind {
    match access {
        MirAliasAccess::ReadOnly | MirAliasAccess::Mutable => StaticAccessKind::Borrow,
    }
}
