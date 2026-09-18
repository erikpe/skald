use super::{Instruction, Opcode};
use crate::backend::{
    graph::{EditError, SelectedBlockId, SelectedValueId},
    selected::{EditablePayload, SelectedRemap},
};
impl Instruction {
    fn values_mut(&mut self) -> Vec<(&mut super::ValueRef, bool)> {
        match &mut self.opcode {
            Opcode::Numeric(n) => n.operands_mut(),
            Opcode::CheckBranch { condition, .. } => vec![(condition, false)],
            Opcode::Failure { arguments, .. } => arguments.iter_mut().map(|v| (v, false)).collect(),
            Opcode::Constant { out, .. }
            | Opcode::SymbolAddress { out, .. }
            | Opcode::ObjectAddress { out, .. } => vec![(out, true)],
            Opcode::Unary { input, out, .. } => {
                vec![(input, false), (out, true)]
            }
            Opcode::Alu {
                left, right, out, ..
            }
            | Opcode::IntegerCompare {
                left, right, out, ..
            }
            | Opcode::FloatCompare {
                left, right, out, ..
            } => vec![(left, false), (right, false), (out, true)],
            Opcode::ByteOffset { base, offset, out } => {
                vec![(base, false), (offset, false), (out, true)]
            }
            Opcode::Load { address, out, .. } => vec![(address, false), (out, true)],
            Opcode::Store { address, value, .. } => vec![(address, false), (value, false)],
            Opcode::Branch { condition } => vec![(condition, false)],
            Opcode::Return { values, .. } => values.iter_mut().map(|v| (v, false)).collect(),
            Opcode::Lifetime { .. } | Opcode::Jump => vec![],
        }
    }
}
impl EditablePayload for Instruction {
    fn replace_use(&mut self, slot: usize, value: SelectedValueId) -> Result<(), EditError> {
        let mut operands = self.values_mut();
        let (operand, definition) = operands.get_mut(slot).ok_or(EditError::InvalidLocation)?;
        if *definition {
            return Err(EditError::InvalidLocation);
        }
        operand.value = value;
        Ok(())
    }
    fn remap(&mut self, remap: &SelectedRemap<'_>) -> Result<(), EditError> {
        for (value, _) in self.values_mut() {
            value.value = remap.values.get(value.value)?;
        }
        match &mut self.opcode {
            Opcode::ObjectAddress { object, .. } | Opcode::Lifetime { object, .. } => {
                *object = remap.objects.get(*object)?
            }
            Opcode::Load {
                region: crate::backend::effects::MemoryRegion::Object(object),
                ..
            }
            | Opcode::Store {
                region: crate::backend::effects::MemoryRegion::Object(object),
                ..
            } => *object = remap.objects.get(*object)?,
            _ => {}
        }
        super::numeric::remap_metadata(&mut self.opcode, remap)?;
        self.refresh();
        Ok(())
    }
    fn relocate_split(
        &mut self,
        from: SelectedBlockId,
        to: SelectedBlockId,
        _at: usize,
    ) -> Result<(), EditError> {
        if let Opcode::Numeric(super::numeric::Numeric::Divide {
            overflow: Some(block),
            ..
        }) = &mut self.opcode
        {
            if *block == from {
                *block = to;
            }
        }
        Ok(())
    } // Stable lower provenance is independent of selected block placement.
}
