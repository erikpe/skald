use super::super::{
    super::model::{MirArrayInstruction, MirBasicBlock, MirDefinitionRef, MirStorageKind, MirType},
    context::Verifier,
};

impl Verifier<'_> {
    pub(in crate::mir::verify) fn verify_array_anchor_instruction(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        instruction: &MirArrayInstruction,
    ) {
        match instruction {
            MirArrayInstruction::AnchorBegin {
                anchor,
                owner,
                array,
                kind,
                ..
            } => {
                let valid = function.storage(*anchor).is_some_and(|storage| {
                    storage.kind == MirStorageKind::ArrayAnchor(*kind)
                        && storage.ty == MirType::Array(*array)
                }) && self
                    .verify_place(function, block, owner)
                    .map(|place| place.ty)
                    == Some(MirType::Array(*array));
                if !valid {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array anchor has incompatible storage, kind, owner, or type",
                    );
                }
            }
            MirArrayInstruction::AnchorEnd { anchor, .. }
                if !function.storage(*anchor).is_some_and(|storage| {
                    matches!(storage.kind, MirStorageKind::ArrayAnchor(_))
                }) =>
            {
                self.block_error(
                    function.callable(),
                    block.id,
                    "array anchor end names non-anchor storage",
                );
            }
            _ => {}
        }
    }
}
