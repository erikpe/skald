//! Shared-owner element construction for inline and shared outer arrays.

use crate::{
    backend::BackendError,
    identity::{ArrayTypeId, ClassId, InitializerId, OptionalBoxTypeId},
    mir::MirPlace,
};

use super::super::{
    super::{
        layout::{
            ARRAY_OWNER_COUNT_OFFSET, SHARED_ARRAY_LENGTH_OFFSET, SHARED_DYNAMIC_METADATA_OFFSET,
        },
        machine::{Instruction, Register},
        symbol,
    },
    value, InstructionSelector,
};

const RUNTIME_ALLOC: &str = "ska_rt_alloc";
const PRESERVED_HANDLE_SIZE: u32 = 16;

impl InstructionSelector<'_, '_> {
    pub(super) fn select_default_shared_class_element(
        &mut self,
        destination: &MirPlace,
        class: ClassId,
        initializer: InitializerId,
    ) -> Result<(), BackendError> {
        let byte_count = self.data_layout.shared_allocation_size(class)?;
        self.output.push(Instruction::MoveImmediate64 {
            bits: byte_count,
            destination: Register::Rdi,
        });
        self.emit_source_operation_call(RUNTIME_ALLOC.to_owned())?;
        self.preserve_shared_element_handle();
        self.select_shared_initialize_at_handle(initializer, value::memory(Register::Rsp, 0))?;
        self.publish_preserved_class_handle(class);
        self.restore_shared_element_handle();
        self.store_shared_place(destination)
    }

    pub(super) fn select_default_shared_array_element(
        &mut self,
        destination: &MirPlace,
        array: ArrayTypeId,
    ) -> Result<(), BackendError> {
        let layout = self
            .data_layout
            .array(array)
            .expect("verified shared array element has a target layout");
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::try_from(layout.shared_element_offset())
                .expect("shared empty array size fits u64"),
            destination: Register::Rdi,
        });
        self.emit_source_operation_call(RUNTIME_ALLOC.to_owned())?;
        self.preserve_shared_element_handle();
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::R11.into(),
        });
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rax,
        });
        value::store_rax(
            value::memory(Register::R11, SHARED_ARRAY_LENGTH_OFFSET),
            self.output,
        );
        self.output.push(Instruction::LoadSymbolAddress {
            symbol: symbol::shared_array_metadata(array),
            destination: Register::Rax,
        });
        value::store_rax(
            value::memory(Register::R11, SHARED_DYNAMIC_METADATA_OFFSET),
            self.output,
        );
        self.publish_preserved_count();
        self.restore_shared_element_handle();
        self.store_shared_place(destination)
    }

    pub(super) fn select_default_shared_optional_box_element(
        &mut self,
        destination: &MirPlace,
        target: OptionalBoxTypeId,
    ) -> Result<(), BackendError> {
        let layout = self.data_layout.exact_optional_box(target)?;
        let payload_offset = i32::try_from(layout.payload_offset()).map_err(|_| {
            BackendError::new(
                crate::backend::Target::X86_64SysV,
                Some(self.function.callable()),
                format!("optional-box {target} payload offset exceeds x86-64 limits"),
            )
        })?;
        self.output.push(Instruction::MoveImmediate64 {
            bits: layout.byte_count(),
            destination: Register::Rdi,
        });
        self.emit_source_operation_call(RUNTIME_ALLOC.to_owned())?;
        self.preserve_shared_element_handle();
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::R11.into(),
        });
        // Every canonical optional representation denotes absence with a zero
        // outer state word. The rest of an absent payload is unpublished.
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rax,
        });
        value::store_rax(value::memory(Register::R11, payload_offset), self.output);
        self.output.push(Instruction::LoadSymbolAddress {
            symbol: symbol::optional_box_metadata(target),
            destination: Register::Rax,
        });
        value::store_rax(
            value::memory(Register::R11, SHARED_DYNAMIC_METADATA_OFFSET),
            self.output,
        );
        self.publish_preserved_count();
        self.restore_shared_element_handle();
        self.store_shared_place(destination)
    }

    fn preserve_shared_element_handle(&mut self) {
        self.output
            .push(Instruction::ReserveStack(PRESERVED_HANDLE_SIZE));
        value::store_rax(value::memory(Register::Rsp, 0), self.output);
    }

    fn publish_preserved_class_handle(&mut self, class: ClassId) {
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rsp, 0),
            destination: Register::R11.into(),
        });
        self.output.push(Instruction::LoadSymbolAddress {
            symbol: symbol::dispatch_table(self.program, class),
            destination: Register::Rax,
        });
        value::store_rax(
            value::memory(Register::R11, SHARED_DYNAMIC_METADATA_OFFSET),
            self.output,
        );
        self.publish_preserved_count();
    }

    fn publish_preserved_count(&mut self) {
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::Rax,
        });
        value::store_rax(
            value::memory(Register::R11, ARRAY_OWNER_COUNT_OFFSET),
            self.output,
        );
    }

    fn restore_shared_element_handle(&mut self) {
        value::load_rax(value::memory(Register::Rsp, 0), self.output);
        self.output
            .push(Instruction::ReleaseStack(PRESERVED_HANDLE_SIZE));
    }
}
