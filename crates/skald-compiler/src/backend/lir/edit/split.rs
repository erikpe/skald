use super::{super::*, LoweredEditor};
use crate::backend::graph::EditError;
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> LoweredEditor<'p> {
    /// Move the suffix and original terminal, updating guard and trace-site associations.
    pub(in crate::backend) fn split_block(
        &mut self,
        block: BlockHandle<'p>,
        at: usize,
    ) -> Result<BlockHandle<'p>, EditError> {
        let old = self.draft.blocks.get(block)?;
        if at > old.instructions.len() || old.terminator.is_none() {
            return Err(EditError::InvalidLocation);
        }
        let mut suffix = old.clone();
        suffix.parameters = Some(vec![]);
        suffix.instructions = old.instructions[at..].to_vec();
        let guard = matches!(old.terminator, Some(Terminator::ScalarCheck { .. }));
        let next = self.draft.blocks.push(suffix)?;
        let old = self.draft.blocks.get_mut(block)?;
        old.instructions.truncate(at);
        old.terminator = Some(Terminator::Jump(Edge {
            target: next.id(),
            arguments: vec![],
        }));
        old.terminal_effects = Some(crate::backend::effects::Effects::default());
        for handle in self.draft.blocks.handles().collect::<Vec<_>>() {
            let record = self.draft.blocks.get_mut(handle)?;
            for instruction in &mut record.instructions {
                if guard {
                    let evidence = match &mut instruction.operation {
                        Operation::Divide { evidence, .. } | Operation::Shift { evidence, .. } => {
                            Some(evidence)
                        }
                        Operation::Convert { evidence, .. } => evidence.as_mut(),
                        _ => None,
                    };
                    if let Some(ScalarDomainEvidence::SuccessCheck(id)) = evidence {
                        if *id == block.id() {
                            *id = next.id();
                        }
                    }
                }
                if let Operation::Trace(TraceAction::ReplaceLocation { site, .. }) =
                    &mut instruction.operation
                {
                    match site {
                        TraceSite::Instruction { block: id, ordinal }
                            if *id == block.id() && *ordinal >= at =>
                        {
                            *id = next.id();
                            *ordinal -= at;
                        }
                        TraceSite::Terminator(id) if *id == block.id() => *id = next.id(),
                        _ => {}
                    }
                }
            }
        }
        Ok(next)
    }
}
