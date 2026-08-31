//! Static-place extraction from terminators and array operations.

use super::*;

impl MirDependencyExtractor<'_> {
    pub(in crate::passes::reachability) fn extract_static_terminator(
        &mut self,
        source: crate::mir::MirExecutionNode,
        definition: MirDefinitionRef<'_>,
        region: MirDependencyRegion,
        terminator: &MirTerminator,
    ) -> Result<(), MirDependencyExtractionError> {
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
            MirTerminator::Panic { message, .. } => self.add_static_place(
                source,
                definition,
                region,
                message,
                StaticAccessKind::Borrow,
                span,
            )?,
            MirTerminator::CheckedCast { binding, .. } => {
                self.add_static_view(source, definition, region, &binding.view, span)?;
            }
            MirTerminator::SharedCast { cast, .. } => {
                self.add_static_shared_cast_source(source, definition, region, &cast.source, span)?
            }
            MirTerminator::OptionalUnwrap { source: place, .. }
            | MirTerminator::CheckOptionalMutation { source: place, .. } => self.add_static_place(
                source,
                definition,
                region,
                place,
                StaticAccessKind::Borrow,
                span,
            )?,
            MirTerminator::OptionalSharedUnwrap { unwrap, .. } => self.add_static_place(
                source,
                definition,
                region,
                &unwrap.source,
                StaticAccessKind::Borrow,
                span,
            )?,
            MirTerminator::BeginOptionalView { begin, .. } => self.add_static_place(
                source,
                definition,
                region,
                &begin.source,
                StaticAccessKind::Borrow,
                span,
            )?,
            MirTerminator::BeginOptionalBoxView { .. } => {}
        }
        Ok(())
    }

    pub(super) fn extract_static_array_instruction(
        &mut self,
        source: crate::mir::MirExecutionNode,
        definition: MirDefinitionRef<'_>,
        region: MirDependencyRegion,
        instruction: &MirArrayInstruction,
    ) -> Result<(), MirDependencyExtractionError> {
        let span = instruction.span();
        match instruction {
            MirArrayInstruction::Allocate { .. }
            | MirArrayInstruction::AllocateElements { .. }
            | MirArrayInstruction::InitializeElement { .. }
            | MirArrayInstruction::CompleteElement { .. }
            | MirArrayInstruction::Publish { .. }
            | MirArrayInstruction::PublishShared { .. }
            | MirArrayInstruction::AnchorEnd { .. }
            | MirArrayInstruction::SliceBoundsCheck { .. }
            | MirArrayInstruction::InitializeNext { .. } => {}
            MirArrayInstruction::CopyNext { source: place, .. } => self.add_static_place(
                source,
                definition,
                region,
                place,
                StaticAccessKind::Read,
                span,
            )?,
            MirArrayInstruction::Adopt { destination, .. } => self.add_static_place(
                source,
                definition,
                region,
                destination,
                StaticAccessKind::Initialize,
                span,
            )?,
            MirArrayInstruction::Replace { destination, .. } => self.add_static_place(
                source,
                definition,
                region,
                destination,
                StaticAccessKind::Replace,
                span,
            )?,
            MirArrayInstruction::ElementAssign {
                destination,
                source: source_place,
                ..
            }
            | MirArrayInstruction::SliceAssignNext {
                destination,
                source: source_place,
                ..
            } => {
                self.add_static_place(
                    source,
                    definition,
                    region,
                    destination,
                    StaticAccessKind::Replace,
                    span,
                )?;
                self.add_static_place(
                    source,
                    definition,
                    region,
                    source_place,
                    StaticAccessKind::Read,
                    span,
                )?;
            }
            MirArrayInstruction::DestroyNext { owner, .. }
            | MirArrayInstruction::Release { owner, .. } => self.add_static_place(
                source,
                definition,
                region,
                owner,
                StaticAccessKind::Destroy,
                span,
            )?,
            MirArrayInstruction::AnchorBegin { owner, .. }
            | MirArrayInstruction::Normalize { owner, .. }
            | MirArrayInstruction::Offset { owner, .. }
            | MirArrayInstruction::Boundary { owner, .. } => self.add_static_place(
                source,
                definition,
                region,
                owner,
                StaticAccessKind::Borrow,
                span,
            )?,
            MirArrayInstruction::AliasBind { source: place, .. } => self.add_static_place(
                source,
                definition,
                region,
                place,
                StaticAccessKind::Borrow,
                span,
            )?,
            MirArrayInstruction::SliceCopy { source: place, .. } => self.add_static_place(
                source,
                definition,
                region,
                place,
                StaticAccessKind::Read,
                span,
            )?,
            MirArrayInstruction::SliceLengthCheck { source: place, .. } => self.add_static_place(
                source,
                definition,
                region,
                place,
                StaticAccessKind::Borrow,
                span,
            )?,
        }
        Ok(())
    }
}

pub(super) const fn alias_access(access: MirAliasAccess) -> StaticAccessKind {
    match access {
        MirAliasAccess::ReadOnly | MirAliasAccess::Mutable => StaticAccessKind::Borrow,
    }
}
