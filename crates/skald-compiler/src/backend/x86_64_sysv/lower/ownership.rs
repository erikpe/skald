//! x86-64 realization of verified shared-owner operations.
//!
//! This is the sole owner of allocation-header offsets, count transitions,
//! runtime allocation symbols, and last-owner finalizer selection.

use crate::{
    backend::{BackendError, Target},
    mir::{
        MirPlace, MirSharedAdopt, MirSharedAllocate, MirSharedCast, MirSharedCastSource,
        MirSharedCastTransfer, MirSharedCopy, MirSharedFieldCopy, MirSharedFieldInitialize,
        MirSharedFieldReplace, MirSharedMove, MirSharedPublish, MirSharedRelease, MirType,
        StorageId,
    },
};

use super::{
    super::{
        layout::SHARED_DYNAMIC_METADATA_OFFSET,
        machine::{Instruction, Label, Register},
        symbol,
    },
    value, InstructionSelector,
};

mod count;
mod helpers;

pub(super) use count::{emit_release_loaded_handle, emit_retain_loaded_handle};

pub(super) fn lower_helpers(
    program: &crate::mir::MirProgram,
    dispatch: &super::super::dispatch::DispatchMetadata,
) -> Vec<super::super::machine::AssemblyFunction> {
    helpers::lower_all(program, dispatch)
}

const STRONG_COUNT_OFFSET: i32 = 0;
const RUNTIME_ALLOC: &str = "ska_rt_alloc";
const RUNTIME_FREE: &str = "ska_rt_free";
const PRESERVED_HANDLE_STACK_SIZE: u32 = 16;

impl InstructionSelector<'_, '_> {
    pub(super) fn select_shared_allocate(
        &mut self,
        allocation: &MirSharedAllocate,
    ) -> Result<(), BackendError> {
        let byte_count = self.data_layout.shared_allocation_size(allocation.class)?;
        self.output.push(Instruction::MoveImmediate64 {
            bits: byte_count,
            destination: Register::Rdi,
        });
        self.output
            .push(Instruction::Call(RUNTIME_ALLOC.to_owned()));
        value::store_rax(
            value::frame_storage(self.frame, allocation.allocation),
            self.output,
        );
        Ok(())
    }

    pub(super) fn select_shared_publish(
        &mut self,
        publish: &MirSharedPublish,
    ) -> Result<(), BackendError> {
        let class = match self
            .function
            .storage(publish.allocation)
            .expect("verified publication names storage")
            .ty
        {
            MirType::Class(class) => class,
            _ => {
                return Err(
                    self.ownership_error("shared publication storage has no exact dynamic class")
                )
            }
        };
        self.load_shared_handle(publish.allocation, Register::R11);
        self.output.push(Instruction::LoadSymbolAddress {
            symbol: symbol::dispatch_table(class),
            destination: Register::Rax,
        });
        value::store_rax(
            value::memory(Register::R11, SHARED_DYNAMIC_METADATA_OFFSET),
            self.output,
        );
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::Rax,
        });
        value::store_rax(
            value::memory(Register::R11, STRONG_COUNT_OFFSET),
            self.output,
        );
        Ok(())
    }

    pub(super) fn select_shared_adopt(&mut self, adopt: &MirSharedAdopt) {
        value::load_rax(
            value::frame_storage(self.frame, adopt.allocation),
            self.output,
        );
        value::store_rax(
            value::frame_storage(self.frame, adopt.destination),
            self.output,
        );
    }

    pub(super) fn select_shared_copy(&mut self, copy: &MirSharedCopy) {
        let (invalid, overflow, complete) = self.retain_labels("retain");
        self.load_shared_handle(copy.source, Register::Rax);
        emit_retain_loaded_handle(invalid.clone(), overflow.clone(), self.output);
        value::store_rax(
            value::frame_storage(self.frame, copy.destination),
            self.output,
        );
        emit_retain_outcome_blocks(invalid, overflow, complete.clone(), self.output);
        self.output.push(Instruction::Label(complete));
    }

    pub(super) fn select_shared_cast(&mut self, cast: &MirSharedCast) -> Result<(), BackendError> {
        self.load_shared_cast_source(&cast.source)?;
        if cast.transfer == MirSharedCastTransfer::Copy {
            let (invalid, overflow, complete) = self.retain_labels("cast_retain");
            emit_retain_loaded_handle(invalid.clone(), overflow.clone(), self.output);
            value::store_rax(
                value::frame_storage(self.frame, cast.destination),
                self.output,
            );
            emit_retain_outcome_blocks(invalid, overflow, complete.clone(), self.output);
            self.output.push(Instruction::Label(complete));
        } else {
            value::store_rax(
                value::frame_storage(self.frame, cast.destination),
                self.output,
            );
        }
        Ok(())
    }

    pub(super) fn load_shared_cast_metadata(
        &mut self,
        source: &MirSharedCastSource,
    ) -> Result<(), BackendError> {
        self.load_shared_cast_source(source)?;
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rax, SHARED_DYNAMIC_METADATA_OFFSET),
            destination: Register::R11.into(),
        });
        Ok(())
    }

    fn load_shared_cast_source(
        &mut self,
        source: &MirSharedCastSource,
    ) -> Result<(), BackendError> {
        match source {
            MirSharedCastSource::Owner { storage, .. } => {
                self.load_shared_handle(*storage, Register::Rax);
                Ok(())
            }
            MirSharedCastSource::Field { place, .. } => self.load_shared_place(place),
        }
    }

    pub(super) fn select_shared_move(&mut self, transfer: &MirSharedMove) {
        value::load_rax(
            value::frame_storage(self.frame, transfer.source),
            self.output,
        );
        value::store_rax(
            value::frame_storage(self.frame, transfer.destination),
            self.output,
        );
    }

    pub(super) fn select_shared_release(&mut self, release: &MirSharedRelease) {
        let (failure, complete) = self.ownership_labels("release");
        let last = self.ownership_label("release_last");
        self.load_shared_handle(release.owner, Register::Rax);
        emit_release_loaded_handle(
            failure,
            last,
            complete.clone(),
            self.dispatch.finalizer_displacement(),
            self.output,
        );
        self.output.push(Instruction::Label(complete));
    }

    pub(super) fn select_shared_field_copy(
        &mut self,
        copy: &MirSharedFieldCopy,
    ) -> Result<(), BackendError> {
        self.load_shared_place(&copy.source)?;
        let (invalid, overflow, complete) = self.retain_labels("field_retain");
        emit_retain_loaded_handle(invalid.clone(), overflow.clone(), self.output);
        value::store_rax(
            value::frame_storage(self.frame, copy.destination),
            self.output,
        );
        emit_retain_outcome_blocks(invalid, overflow, complete.clone(), self.output);
        self.output.push(Instruction::Label(complete));
        Ok(())
    }

    pub(super) fn select_shared_field_initialize(
        &mut self,
        initialize: &MirSharedFieldInitialize,
    ) -> Result<(), BackendError> {
        self.load_shared_handle(initialize.source, Register::Rax);
        self.store_shared_place(&initialize.destination)
    }

    pub(super) fn select_shared_field_replace(
        &mut self,
        replace: &MirSharedFieldReplace,
    ) -> Result<(), BackendError> {
        self.release_shared_place(&replace.destination, "field_replace")?;
        self.load_shared_handle(replace.source, Register::Rax);
        self.store_shared_place(&replace.destination)
    }

    pub(super) fn select_shared_field_construction(
        &mut self,
        destination: &MirPlace,
        source: &MirPlace,
    ) -> Result<(), BackendError> {
        self.load_shared_place(source)?;
        let (invalid, overflow, complete) = self.retain_labels("field_copy_construct");
        emit_retain_loaded_handle(invalid.clone(), overflow.clone(), self.output);
        self.store_shared_place(destination)?;
        emit_retain_outcome_blocks(invalid, overflow, complete.clone(), self.output);
        self.output.push(Instruction::Label(complete));
        Ok(())
    }

    pub(super) fn select_shared_field_assignment(
        &mut self,
        destination: &MirPlace,
        source: &MirPlace,
    ) -> Result<(), BackendError> {
        self.load_shared_place(source)?;
        let (invalid, overflow, retained) = self.retain_labels("field_copy_assign_retain");
        emit_retain_loaded_handle(invalid.clone(), overflow.clone(), self.output);
        self.output
            .push(Instruction::ReserveStack(PRESERVED_HANDLE_STACK_SIZE));
        value::store_rax(value::memory(Register::Rsp, 0), self.output);
        emit_retain_outcome_blocks(invalid, overflow, retained.clone(), self.output);
        self.output.push(Instruction::Label(retained));

        self.release_shared_place(destination, "field_copy_assign_release")?;

        value::load_rax(value::memory(Register::Rsp, 0), self.output);
        self.output
            .push(Instruction::ReleaseStack(PRESERVED_HANDLE_STACK_SIZE));
        self.store_shared_place(destination)
    }

    pub(super) fn release_shared_place(
        &mut self,
        place: &MirPlace,
        purpose: &str,
    ) -> Result<(), BackendError> {
        self.load_shared_place(place)?;
        let (failure, complete) = self.ownership_labels(purpose);
        let last = self.ownership_label(&format!("{purpose}_last"));
        emit_release_loaded_handle(
            failure,
            last,
            complete.clone(),
            self.dispatch.finalizer_displacement(),
            self.output,
        );
        self.output.push(Instruction::Label(complete));
        Ok(())
    }

    fn load_shared_handle(&mut self, storage: StorageId, destination: Register) {
        self.output.push(Instruction::Move {
            source: value::frame_storage(self.frame, storage),
            destination: destination.into(),
        });
    }

    fn load_shared_place(&mut self, place: &MirPlace) -> Result<(), BackendError> {
        let (layout, source) = self.frame_place(place)?;
        debug_assert!(matches!(layout.ty(), MirType::Shared(_)));
        value::load_rax(source, self.output);
        Ok(())
    }

    pub(super) fn store_shared_place(&mut self, place: &MirPlace) -> Result<(), BackendError> {
        self.output
            .push(Instruction::ReserveStack(PRESERVED_HANDLE_STACK_SIZE));
        value::store_rax(value::memory(Register::Rsp, 0), self.output);
        self.materialize_place_address(place, Register::Rdx)?;
        value::load_rax(value::memory(Register::Rsp, 0), self.output);
        self.output
            .push(Instruction::ReleaseStack(PRESERVED_HANDLE_STACK_SIZE));
        value::store_rax(value::memory(Register::Rdx, 0), self.output);
        Ok(())
    }

    fn ownership_labels(&self, operation: &str) -> (Label, Label) {
        (
            self.ownership_label(&format!("{operation}_invalid")),
            self.ownership_label(&format!("{operation}_complete")),
        )
    }

    fn retain_labels(&self, operation: &str) -> (Label, Label, Label) {
        (
            self.ownership_label(&format!("{operation}_invalid")),
            self.ownership_label(&format!("{operation}_overflow")),
            self.ownership_label(&format!("{operation}_complete")),
        )
    }

    fn ownership_label(&self, purpose: &str) -> Label {
        Label::new(format!(
            ".Lska_{}_ownership_{}_{}",
            symbol::local_label_stem(self.function.callable()),
            purpose,
            self.output.len()
        ))
    }

    fn ownership_error(&self, message: impl Into<String>) -> BackendError {
        BackendError::new(Target::X86_64SysV, Some(self.function.callable()), message)
    }
}

fn emit_retain_outcome_blocks(
    invalid: Label,
    overflow: Label,
    complete: Label,
    output: &mut Vec<Instruction>,
) {
    output.push(Instruction::Jump(complete));
    output.push(Instruction::Label(overflow));
    super::terminator::emit_ownership_overflow(output);
    output.push(Instruction::Label(invalid));
    // Verified shared retain operations always carry a non-null, live owner.
    output.push(Instruction::Trap);
}
