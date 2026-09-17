use super::{super::*, EditablePayload, SelectedEditor};
use crate::backend::graph::{
    EditError, IdMap, LocalHandle, SelectedBlockId, SelectedObjectId, SelectedValueId,
};
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct SelectedRemap<'p> {
    source: SelectedReceipt<'p>,
    pub values: IdMap<SelectedValueId>,
    pub blocks: IdMap<SelectedBlockId>,
    pub objects: IdMap<SelectedObjectId>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl SelectedRemap<'_> {
    pub(in crate::backend) fn require_source(
        &self,
        receipt: &SelectedReceipt<'_>,
    ) -> Result<(), EditError> {
        if self.source.same_snapshot(receipt) {
            Ok(())
        } else {
            Err(EditError::StaleSnapshot)
        }
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p, P: EditablePayload> SelectedEditor<'p, P> {
    pub(in crate::backend) fn rebuild(
        mut self,
        values: &[LocalHandle<'p, SelectedValueId>],
        blocks: &[LocalHandle<'p, SelectedBlockId>],
        objects: &[LocalHandle<'p, SelectedObjectId>],
    ) -> Result<(Self, SelectedRemap<'p>), EditError> {
        let (new_values, values) = self.draft.values.rebuild(values)?;
        let (new_blocks, blocks) = self.draft.blocks.rebuild(blocks)?;
        let (new_objects, objects) = self.draft.objects.rebuild(objects)?;
        let remap = SelectedRemap {
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
        for handle in self.draft.blocks.handles().collect::<Vec<_>>() {
            let block = self.draft.blocks.get_mut(handle)?;
            for id in &mut block.parameters {
                *id = remap.values.get(*id)?;
            }
            for op in &mut block.instructions {
                op.remap(&remap)?;
            }
            if let Some(term) = &mut block.terminal {
                term.payload.remap(&remap)?;
                for (target, args) in &mut term.edges {
                    *target = remap.blocks.get(*target)?;
                    for id in args {
                        *id = remap.values.get(*id)?;
                    }
                }
            }
        }
        self.draft
            .origins
            .retain(|_, id| match remap.values.get(*id) {
                Ok(new) => {
                    *id = new;
                    true
                }
                Err(_) => false,
            });
        self.draft
            .block_origins
            .retain(|_, id| match remap.blocks.get(*id) {
                Ok(new) => {
                    *id = new;
                    true
                }
                Err(_) => false,
            });
        self.draft
            .object_origins
            .retain(|_, id| match remap.objects.get(*id) {
                Ok(new) => {
                    *id = new;
                    true
                }
                Err(_) => false,
            });
        self.refresh()?;
        Ok((self, remap))
    }
}
