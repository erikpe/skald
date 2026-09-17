use super::{super::*, references, LoweredEditor};
use crate::backend::graph::{EditError, IdMap, LoweredBlockId, LoweredObjectId, LoweredValueId};
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct LoweredRemap<'p> {
    source: CompletionReceipt<'p>,
    pub values: IdMap<LoweredValueId>,
    pub blocks: IdMap<LoweredBlockId>,
    pub objects: IdMap<LoweredObjectId>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl LoweredRemap<'_> {
    pub(in crate::backend) fn require_source(
        &self,
        receipt: &CompletionReceipt<'_>,
    ) -> Result<(), EditError> {
        if self.source.same_snapshot(receipt) {
            Ok(())
        } else {
            Err(EditError::StaleSnapshot)
        }
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> LoweredEditor<'p> {
    /// Explicit partial orders compact arenas. Any surviving reference to a deleted ID fails.
    pub(in crate::backend) fn rebuild(
        mut self,
        values: &[ValueHandle<'p>],
        blocks: &[BlockHandle<'p>],
        objects: &[ObjectHandle<'p>],
    ) -> Result<(Self, LoweredRemap<'p>), EditError> {
        let (new_values, values) = self.draft.values.rebuild(values)?;
        let (new_blocks, blocks) = self.draft.blocks.rebuild(blocks)?;
        let (new_objects, objects) = self.draft.objects.rebuild(objects)?;
        let remap = LoweredRemap {
            source: self.source.clone(),
            values,
            blocks,
            objects,
        };
        self.draft.values = new_values;
        self.draft.blocks = new_blocks;
        self.draft.objects = new_objects;
        self.draft.entry = self
            .draft
            .entry
            .map(|id| remap.blocks.get(id))
            .transpose()?;
        for id in &mut self.draft.inputs {
            *id = remap.values.get(*id)?;
        }
        if let Some(trace) = &mut self.draft.trace_plan {
            trace.record = trace.record.map(|id| remap.objects.get(id)).transpose()?;
        }
        for handle in self.draft.blocks.handles().collect::<Vec<_>>() {
            let block = self.draft.blocks.get_mut(handle)?;
            if let Some(ids) = &mut block.parameters {
                for id in ids {
                    *id = remap.values.get(*id)?;
                }
            }
            for instruction in &mut block.instructions {
                references::operation(
                    &mut instruction.operation,
                    &mut |id| remap.values.get(id),
                    &mut |id| remap.objects.get(id),
                    &mut |id| remap.blocks.get(id),
                    true,
                )?;
                for id in &mut instruction.results {
                    *id = remap.values.get(*id)?;
                }
                instruction.effects = instruction
                    .effects
                    .try_map_objects(|id| remap.objects.get(id))?;
            }
            if let Some(t) = &mut block.terminator {
                references::terminal(
                    t,
                    &mut |id| remap.values.get(id),
                    &mut |id| remap.blocks.get(id),
                    true,
                )?;
            }
            if let Some(e) = &mut block.terminal_effects {
                *e = e.try_map_objects(|id| remap.objects.get(id))?;
            }
        }
        self.refresh()?;
        Ok((self, remap))
    }
}
