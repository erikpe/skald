use super::{super::*, references};
use crate::backend::graph::{EditError, LoweredBlockId, LoweredValueId};

#[derive(Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum LoweredEditFailure {
    Edit(EditError),
    Verify(Vec<VerificationFailure>),
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct LoweredEditor<'p> {
    pub(super) draft: CallableDraft<'p>,
    pub(super) source: CompletionReceipt<'p>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> LoweredEditor<'p> {
    pub(in crate::backend::lir) fn new(
        draft: CallableDraft<'p>,
        source: CompletionReceipt<'p>,
    ) -> Self {
        Self { draft, source }
    }
    pub(in crate::backend) fn draft(&self) -> &CallableDraft<'p> {
        &self.draft
    }
    pub(in crate::backend) fn values(&self) -> impl ExactSizeIterator<Item = ValueHandle<'p>> + '_ {
        self.draft.values.handles()
    }
    pub(in crate::backend) fn blocks(&self) -> impl ExactSizeIterator<Item = BlockHandle<'p>> + '_ {
        self.draft.blocks.handles()
    }
    pub(in crate::backend) fn objects(
        &self,
    ) -> impl ExactSizeIterator<Item = ObjectHandle<'p>> + '_ {
        self.draft.objects.handles()
    }
    pub(in crate::backend) fn value(
        &self,
        id: LoweredValueId,
    ) -> Result<ValueHandle<'p>, EditError> {
        Ok(self.draft.values.handle_id(id)?)
    }
    pub(in crate::backend) fn block(
        &self,
        id: LoweredBlockId,
    ) -> Result<BlockHandle<'p>, EditError> {
        Ok(self.draft.blocks.handle_id(id)?)
    }
    /// Replace one opcode-derived use. Evidence is deliberately retained and rechecked.
    pub(in crate::backend) fn replace_operand(
        &mut self,
        at: InstructionLocation,
        slot: usize,
        to: ValueHandle<'p>,
    ) -> Result<(), EditError> {
        self.draft.values.get(to)?;
        let handle = self.draft.blocks.handle_id(at.block)?;
        let mut op = self
            .draft
            .blocks
            .get(handle)?
            .instructions
            .get(at.ordinal)
            .ok_or(EditError::InvalidLocation)?
            .operation
            .clone();
        let mut ordinal = 0;
        let mut found = false;
        references::operation(
            &mut op,
            &mut |id| {
                let result = if ordinal == slot {
                    found = true;
                    if self.draft.values.get_id(id)?.ty != self.draft.values.get(to)?.ty {
                        return Err(EditError::TypeMismatch);
                    }
                    to.id()
                } else {
                    id
                };
                ordinal += 1;
                Ok(result)
            },
            &mut Ok,
            &mut Ok,
            false,
        )?;
        if !found {
            return Err(EditError::InvalidLocation);
        }
        self.draft.blocks.get_mut(handle)?.instructions[at.ordinal].operation = op;
        Ok(())
    }
    pub(in crate::backend) fn replace_terminal_operand(
        &mut self,
        block: BlockHandle<'p>,
        slot: usize,
        to: ValueHandle<'p>,
    ) -> Result<(), EditError> {
        self.draft.values.get(to)?;
        let mut terminal = self
            .draft
            .blocks
            .get(block)?
            .terminator
            .clone()
            .ok_or(EditError::InvalidLocation)?;
        let mut ordinal = 0;
        let mut found = false;
        references::terminal(
            &mut terminal,
            &mut |id| {
                let id = if ordinal == slot {
                    found = true;
                    if self.draft.values.get_id(id)?.ty != self.draft.values.get(to)?.ty {
                        return Err(EditError::TypeMismatch);
                    }
                    to.id()
                } else {
                    id
                };
                ordinal += 1;
                Ok(id)
            },
            &mut Ok,
            false,
        )?;
        if !found {
            return Err(EditError::InvalidLocation);
        }
        self.draft.blocks.get_mut(block)?.terminator = Some(terminal);
        Ok(())
    }
    pub(in crate::backend) fn redirect_edge(
        &mut self,
        at: EdgeOccurrence,
        target: BlockHandle<'p>,
        args: &[ValueHandle<'p>],
    ) -> Result<(), EditError> {
        self.draft.blocks.get(target)?;
        for id in args {
            self.draft.values.get(*id)?;
        }
        let handle = self.draft.blocks.handle_id(at.predecessor)?;
        let t = self
            .draft
            .blocks
            .get_mut(handle)?
            .terminator
            .as_mut()
            .ok_or(EditError::InvalidLocation)?;
        let edge = match (t, at.slot) {
            (Terminator::Jump(e), 0) => e,
            (Terminator::Branch { true_edge, .. }, 0) => true_edge,
            (Terminator::Branch { false_edge, .. }, 1) => false_edge,
            (Terminator::ScalarCheck { success, .. }, 0) => success,
            (Terminator::ScalarCheck { failure, .. }, 1) => failure,
            _ => return Err(EditError::InvalidLocation),
        };
        *edge = Edge {
            target: target.id(),
            arguments: args.iter().map(|v| v.id()).collect(),
        };
        Ok(())
    }
    pub(in crate::backend) fn finish(mut self) -> Result<VerifiedCallable<'p>, LoweredEditFailure> {
        self.refresh().map_err(LoweredEditFailure::Edit)?;
        verify_callable(self.draft).map_err(LoweredEditFailure::Verify)
    }
    pub(super) fn refresh(&mut self) -> Result<(), EditError> {
        for handle in self.draft.values.handles().collect::<Vec<_>>() {
            let value = self.draft.values.get_mut(handle)?;
            value.definition = None;
            // Never carry address claims through mutation; publication recomputes them.
            value.provenance = AddressProvenance::Unknown;
        }
        for (component, id) in self.draft.inputs.clone().into_iter().enumerate() {
            self.define(id, Definition::EntryInput { component })?;
        }
        for handle in self.draft.blocks.handles().collect::<Vec<_>>() {
            let mut block = self.draft.blocks.get(handle)?.clone();
            for (ordinal, id) in block
                .parameters
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .copied()
                .enumerate()
            {
                self.define(
                    id,
                    Definition::BlockParameter {
                        block: handle.id(),
                        ordinal,
                    },
                )?;
            }
            for (ordinal, instruction) in block.instructions.iter_mut().enumerate() {
                for (result, id) in instruction.results.iter().copied().enumerate() {
                    self.define(
                        id,
                        Definition::InstructionResult {
                            instruction: InstructionLocation {
                                block: handle.id(),
                                ordinal,
                            },
                            ordinal: result,
                        },
                    )?;
                }
                let required = DraftChecks { draft: &self.draft }
                    .operation_effects(&instruction.operation)
                    .map_err(|_| EditError::TypeMismatch)?;
                instruction.effects = crate::backend::effects::Effects::new(
                    instruction.effects.iter().chain(required.iter()).copied(),
                );
            }
            *self.draft.blocks.get_mut(handle)? = block;
        }
        Ok(())
    }
    fn define(&mut self, id: LoweredValueId, definition: Definition) -> Result<(), EditError> {
        let handle = self.draft.values.handle_id(id)?;
        let value = self.draft.values.get_mut(handle)?;
        if value.definition.replace(definition).is_some() {
            return Err(EditError::DefinitionConflict);
        }
        Ok(())
    }
}
