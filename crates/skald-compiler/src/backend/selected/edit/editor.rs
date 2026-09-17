use super::super::storage::{Block, Terminal};
use super::{super::*, SelectedRemap};
use crate::backend::graph::{
    DefinitionSite, EditError, LocalHandle, SelectedBlockId, SelectedValueId,
};

/// Concrete targets exhaustively rewrite opcode fields, not editable descriptions.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) trait EditablePayload: Payload + Clone {
    fn replace_use(&mut self, slot: usize, value: SelectedValueId) -> Result<(), EditError>;
    fn remap(&mut self, remap: &SelectedRemap<'_>) -> Result<(), EditError>;
    fn relocate_split(
        &mut self,
        from: SelectedBlockId,
        to: SelectedBlockId,
        at: usize,
    ) -> Result<(), EditError>;
}
#[derive(Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum SelectedEditFailure {
    Edit(EditError),
    Verify(Vec<SelectedFailure>),
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct SelectedEditor<'p, P> {
    pub(super) draft: SelectedDraft<'p, P>,
    pub(super) source: SelectedReceipt<'p>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p, P: EditablePayload> SelectedEditor<'p, P> {
    pub(in crate::backend::selected) fn new(
        draft: SelectedDraft<'p, P>,
        source: SelectedReceipt<'p>,
    ) -> Self {
        Self { draft, source }
    }
    pub(in crate::backend) fn draft(&self) -> &SelectedDraft<'p, P> {
        &self.draft
    }
    pub(in crate::backend) fn values(
        &self,
    ) -> impl ExactSizeIterator<Item = LocalHandle<'p, SelectedValueId>> + '_ {
        self.draft.values.handles()
    }
    pub(in crate::backend) fn blocks(
        &self,
    ) -> impl ExactSizeIterator<Item = LocalHandle<'p, SelectedBlockId>> + '_ {
        self.draft.blocks.handles()
    }
    pub(in crate::backend) fn objects(
        &self,
    ) -> impl ExactSizeIterator<Item = LocalHandle<'p, crate::backend::graph::SelectedObjectId>> + '_
    {
        self.draft.objects.handles()
    }
    pub(in crate::backend) fn value(
        &self,
        id: SelectedValueId,
    ) -> Result<LocalHandle<'p, SelectedValueId>, EditError> {
        Ok(self.draft.values.handle_id(id)?)
    }
    pub(in crate::backend) fn block(
        &self,
        id: SelectedBlockId,
    ) -> Result<LocalHandle<'p, SelectedBlockId>, EditError> {
        Ok(self.draft.blocks.handle_id(id)?)
    }
    pub(in crate::backend) fn replace_operand(
        &mut self,
        block: LocalHandle<'p, SelectedBlockId>,
        ordinal: usize,
        slot: usize,
        value: LocalHandle<'p, SelectedValueId>,
    ) -> Result<(), EditError> {
        let ty = self.draft.values.get(value)?.ty;
        let op = self
            .draft
            .blocks
            .get(block)?
            .instructions
            .get(ordinal)
            .ok_or(EditError::InvalidLocation)?;
        let desc = op.describe();
        let operand = desc.operands.get(slot).ok_or(EditError::InvalidLocation)?;
        if operand.role != OperandRole::Use || operand.representation != ty {
            return Err(EditError::TypeMismatch);
        }
        let mut op = op.clone();
        op.replace_use(slot, value.id())?;
        self.draft.blocks.get_mut(block)?.instructions[ordinal] = op;
        Ok(())
    }
    pub(in crate::backend) fn replace_terminal_operand(
        &mut self,
        block: LocalHandle<'p, SelectedBlockId>,
        slot: usize,
        value: LocalHandle<'p, SelectedValueId>,
    ) -> Result<(), EditError> {
        let ty = self.draft.values.get(value)?.ty;
        let mut payload = self
            .draft
            .blocks
            .get(block)?
            .terminal
            .as_ref()
            .ok_or(EditError::InvalidLocation)?
            .payload
            .clone();
        let desc = payload.describe();
        let operand = desc.operands.get(slot).ok_or(EditError::InvalidLocation)?;
        if operand.role != OperandRole::Use || operand.representation != ty {
            return Err(EditError::TypeMismatch);
        }
        payload.replace_use(slot, value.id())?;
        self.draft
            .blocks
            .get_mut(block)?
            .terminal
            .as_mut()
            .ok_or(EditError::InvalidLocation)?
            .payload = payload;
        Ok(())
    }
    pub(in crate::backend) fn redirect_edge(
        &mut self,
        block: LocalHandle<'p, SelectedBlockId>,
        slot: usize,
        target: LocalHandle<'p, SelectedBlockId>,
        args: &[LocalHandle<'p, SelectedValueId>],
    ) -> Result<(), EditError> {
        self.draft.blocks.get(target)?;
        for id in args {
            self.draft.values.get(*id)?;
        }
        let term = self
            .draft
            .blocks
            .get_mut(block)?
            .terminal
            .as_mut()
            .ok_or(EditError::InvalidLocation)?;
        let edge = term.edges.get_mut(slot).ok_or(EditError::InvalidLocation)?;
        *edge = (target.id(), args.iter().map(|id| id.id()).collect());
        Ok(())
    }
    pub(in crate::backend) fn split_block(
        &mut self,
        block: LocalHandle<'p, SelectedBlockId>,
        at: usize,
        transfer: P,
    ) -> Result<LocalHandle<'p, SelectedBlockId>, EditError> {
        let desc = transfer.describe();
        if desc.flow != Flow::Branch
            || desc.successors != 1
            || desc
                .operands
                .iter()
                .any(|op| op.role == OperandRole::Definition)
        {
            return Err(EditError::InvalidLocation);
        }
        let old = self.draft.blocks.get(block)?;
        if at > old.instructions.len() || old.terminal.is_none() {
            return Err(EditError::InvalidLocation);
        }
        let suffix = Block {
            parameters: vec![],
            instructions: old.instructions[at..].to_vec(),
            terminal: old.terminal.clone(),
            origin: old.origin,
        };
        let block_id = block.id();
        let next = self.draft.blocks.push(suffix)?;
        let old = self.draft.blocks.get_mut(block)?;
        old.instructions.truncate(at);
        old.terminal = Some(Terminal {
            payload: transfer,
            edges: vec![(next.id(), vec![])],
        });
        for handle in self.draft.blocks.handles().collect::<Vec<_>>() {
            let block = self.draft.blocks.get_mut(handle)?;
            for op in &mut block.instructions {
                op.relocate_split(block_id, next.id(), at)?;
            }
            if let Some(term) = &mut block.terminal {
                term.payload.relocate_split(block_id, next.id(), at)?;
            }
        }
        Ok(next)
    }
    pub(in crate::backend) fn finish(
        mut self,
        target: &impl TargetVerifier<P>,
    ) -> Result<VerifiedSelectedCallable<'p, P>, SelectedEditFailure> {
        self.refresh().map_err(SelectedEditFailure::Edit)?;
        verify_selected(self.draft, target).map_err(SelectedEditFailure::Verify)
    }
    pub(super) fn refresh(&mut self) -> Result<(), EditError> {
        for handle in self.draft.values.handles().collect::<Vec<_>>() {
            self.draft.values.get_mut(handle)?.definition = None;
        }
        for (ordinal, id) in self.draft.inputs.clone().into_iter().enumerate() {
            self.define(id, DefinitionSite::EntryInput(ordinal))?;
        }
        for handle in self.draft.blocks.handles().collect::<Vec<_>>() {
            let block = self.draft.blocks.get(handle)?.clone();
            for (ordinal, id) in block.parameters.iter().copied().enumerate() {
                self.define(
                    id,
                    DefinitionSite::Parameter {
                        block: handle.id().index(),
                        ordinal,
                    },
                )?;
            }
            for (instruction, op) in block.instructions.iter().enumerate() {
                for (ordinal, operand) in op
                    .describe()
                    .operands
                    .iter()
                    .filter(|op| op.role == OperandRole::Definition)
                    .enumerate()
                {
                    self.define(
                        operand.value,
                        DefinitionSite::Result {
                            block: handle.id().index(),
                            instruction,
                            ordinal,
                        },
                    )?;
                }
            }
        }
        Ok(())
    }
    fn define(&mut self, id: SelectedValueId, definition: DefinitionSite) -> Result<(), EditError> {
        let handle = self.draft.values.handle_id(id)?;
        if self
            .draft
            .values
            .get_mut(handle)?
            .definition
            .replace(definition)
            .is_some()
        {
            return Err(EditError::DefinitionConflict);
        }
        Ok(())
    }
}
