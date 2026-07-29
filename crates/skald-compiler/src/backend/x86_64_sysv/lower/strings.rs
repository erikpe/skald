//! Static literal-handle and exact string-descriptor materialization.

use crate::mir::{MirSharedStatic, MirStringInitialize, MirTerminator};

use super::{
    super::machine::{Instruction, Register},
    value, InstructionSelector,
};

impl InstructionSelector<'_, '_> {
    pub(super) fn select_panic_terminator(
        &mut self,
        terminator: &MirTerminator,
    ) -> Result<bool, crate::backend::BackendError> {
        let MirTerminator::Panic { message, .. } = terminator else {
            return Ok(false);
        };
        let item = self
            .program
            .string_language_item
            .expect("verified panic requires the string language item");
        self.materialize_place_address(message, Register::Rdx)?;
        let field_offset = |field| {
            let offset = self
                .data_layout
                .field(field)
                .expect("verified string field has target layout")
                .offset;
            i32::try_from(offset).expect("target layout bounds every field displacement")
        };

        self.output.push(Instruction::Move {
            source: value::memory(Register::Rdx, field_offset(item.storage_field)),
            destination: Register::Rdi.into(),
        });
        let data_offset = self
            .data_layout
            .array(item.storage_array)
            .expect("verified string backing has target layout")
            .shared_element_offset();
        self.output.push(Instruction::LoadEffectiveAddress {
            source: value::memory(
                Register::Rdi,
                i32::try_from(data_offset).expect("array data offset fits x86-64"),
            ),
            destination: Register::Rdi,
        });
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rdx, field_offset(item.start_field)),
            destination: Register::Rax.into(),
        });
        self.output.push(Instruction::Add {
            source: Register::Rax,
            destination: Register::Rdi,
        });
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rdx, field_offset(item.length_field)),
            destination: Register::Rsi.into(),
        });
        self.output
            .push(Instruction::Call("ska_rt_panic".to_owned()));
        // Returning from the runtime's `_Noreturn` reporter violates the
        // public ABI and must not fall through into another MIR block.
        self.output.push(Instruction::Trap);
        Ok(true)
    }

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
            bits: u64::try_from(initialize.start)
                .expect("verified literal string start must be nonnegative"),
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
