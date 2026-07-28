//! Static literal-handle and exact string-descriptor materialization.

use crate::mir::{MirSharedStatic, MirStringInitialize};

use super::{
    super::machine::{Instruction, Register},
    value, InstructionSelector,
};

impl InstructionSelector<'_, '_> {
    pub(super) fn select_shared_static(&mut self, static_owner: &MirSharedStatic) {
        self.output.push(Instruction::LoadSymbolAddress {
            symbol: self.literal_pool.symbol(static_owner.data).to_owned(),
            destination: Register::Rax,
        });
        value::store_rax(
            value::frame_storage(self.frame, static_owner.destination),
            self.output,
        );
    }

    pub(super) fn select_string_initialize(
        &mut self,
        initialize: &MirStringInitialize,
    ) -> Result<(), crate::backend::BackendError> {
        self.materialize_place_address(&initialize.destination, Register::Rdx)?;
        value::load_rax(
            value::frame_storage(self.frame, initialize.backing),
            self.output,
        );
        self.store_descriptor_word(initialize.storage_field, Register::Rax);

        self.output.push(Instruction::MoveImmediate64 {
            bits: initialize.start,
            destination: Register::Rax,
        });
        self.store_descriptor_word(initialize.start_field, Register::Rax);

        self.output.push(Instruction::MoveImmediate64 {
            bits: initialize.length,
            destination: Register::Rax,
        });
        self.store_descriptor_word(initialize.length_field, Register::Rax);
        Ok(())
    }

    fn store_descriptor_word(&mut self, field: crate::identity::FieldId, source: Register) {
        let offset = self
            .data_layout
            .field(field)
            .expect("verified string field has target layout")
            .offset;
        let offset = i32::try_from(offset).expect("target layout bounds every field displacement");
        self.output.push(Instruction::Move {
            source: source.into(),
            destination: value::memory(Register::Rdx, offset),
        });
    }
}
