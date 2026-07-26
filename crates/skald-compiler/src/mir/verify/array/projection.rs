use std::collections::HashSet;

use super::super::{
    super::model::{MirArrayInstruction, MirBasicBlock, MirDefinitionRef, MirType, ValueId},
    context::Verifier,
};

impl Verifier<'_> {
    pub(in crate::mir::verify) fn verify_array_projection_instruction(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        instruction: &MirArrayInstruction,
        defined: &HashSet<ValueId>,
    ) {
        if let MirArrayInstruction::Normalize {
            owner,
            index,
            array,
            ..
        } = instruction
        {
            if self
                .verify_place(function, block, owner)
                .map(|place| place.ty)
                != Some(MirType::Array(*array))
            {
                self.block_error(
                    function.callable(),
                    block.id,
                    "array normalization owner has the wrong exact type",
                );
            }
            if self.verify_value_use(function, block, *index, defined) != Some(MirType::I64) {
                self.block_error(
                    function.callable(),
                    block.id,
                    "array normalization source must be a block-local `i64` value",
                );
            }
        }
        if let MirArrayInstruction::Boundary { owner, array, .. } = instruction {
            if self
                .verify_place(function, block, owner)
                .map(|place| place.ty)
                != Some(MirType::Array(*array))
            {
                self.block_error(
                    function.callable(),
                    block.id,
                    "array boundary owner has the wrong exact type",
                );
            }
        }
    }
}
