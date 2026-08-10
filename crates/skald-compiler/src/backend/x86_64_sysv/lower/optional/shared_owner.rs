//! Nullable shared-owner optional lifecycle lowering.

use crate::{
    backend::BackendError,
    mir::{
        MirOptionalSharedAssign, MirOptionalSharedCleanup, MirOptionalSharedInitialize,
        MirOptionalSharedSource, MirPlace,
    },
};

use super::super::{
    super::machine::{Instruction, Register},
    ownership::{emit_release_loaded_handle, emit_retain_loaded_handle},
    value, InstructionSelector,
};

const OPTIONAL_SHARED_PRESERVED_HANDLE_SIZE: u32 = 16;

impl InstructionSelector<'_, '_> {
    pub(in crate::backend::x86_64_sysv::lower) fn select_optional_shared_initialize(
        &mut self,
        initialize: &MirOptionalSharedInitialize,
    ) -> Result<(), BackendError> {
        self.load_optional_shared_source(&initialize.source, true)?;
        self.store_optional_shared_place(&initialize.destination)
    }

    pub(in crate::backend::x86_64_sysv::lower) fn select_optional_shared_assign(
        &mut self,
        assignment: &MirOptionalSharedAssign,
    ) -> Result<(), BackendError> {
        self.load_optional_shared_source(&assignment.source, true)?;
        self.output.push(Instruction::ReserveStack(
            OPTIONAL_SHARED_PRESERVED_HANDLE_SIZE,
        ));
        value::store_rax(value::memory(Register::Rsp, 0), self.output);
        self.release_optional_shared_place(&assignment.destination, "optional_assign")?;
        value::load_rax(value::memory(Register::Rsp, 0), self.output);
        self.output.push(Instruction::ReleaseStack(
            OPTIONAL_SHARED_PRESERVED_HANDLE_SIZE,
        ));
        self.store_optional_shared_place(&assignment.destination)
    }

    pub(in crate::backend::x86_64_sysv::lower) fn select_optional_shared_cleanup(
        &mut self,
        cleanup: &MirOptionalSharedCleanup,
    ) -> Result<(), BackendError> {
        self.release_optional_shared_place(&cleanup.destination, "optional_cleanup")
    }

    fn load_optional_shared_source(
        &mut self,
        source: &MirOptionalSharedSource,
        retain_copy: bool,
    ) -> Result<(), BackendError> {
        match source {
            MirOptionalSharedSource::Absent => {
                self.output.push(Instruction::MoveImmediate64 {
                    bits: 0,
                    destination: Register::Rax,
                });
            }
            MirOptionalSharedSource::Present(storage) | MirOptionalSharedSource::Move(storage) => {
                value::load_rax(value::frame_storage(self.frame, *storage), self.output);
            }
            MirOptionalSharedSource::Copy(place) => {
                let (_, operand) = self.frame_place(place)?;
                value::load_rax(operand, self.output);
                if retain_copy {
                    let absent = self.next_optional_label("shared_copy_absent");
                    let invalid = self.next_optional_label("shared_copy_invalid");
                    let overflow = self.next_optional_label("shared_copy_overflow");
                    let complete = self.next_optional_label("shared_copy_complete");
                    self.output.push(Instruction::Test(Register::Rax));
                    self.output.push(Instruction::JumpIfEqual(absent.clone()));
                    emit_retain_loaded_handle(invalid.clone(), overflow.clone(), self.output);
                    self.output.push(Instruction::Jump(complete.clone()));
                    self.output.push(Instruction::Label(overflow));
                    let location = self.current_runtime_trace_location()?;
                    super::super::terminator::emit_ownership_overflow(
                        super::super::call::TraceAttribution::SourceOperation,
                        location.as_ref(),
                        self.output,
                    );
                    self.output.push(Instruction::Label(invalid));
                    // A present optional owner must contain a verified live handle.
                    self.output.push(Instruction::Trap);
                    self.output.push(Instruction::Label(absent));
                    self.output.push(Instruction::Label(complete));
                }
            }
        }
        Ok(())
    }

    pub(in crate::backend::x86_64_sysv::lower) fn release_optional_shared_place(
        &mut self,
        place: &MirPlace,
        purpose: &str,
    ) -> Result<(), BackendError> {
        let (_, operand) = self.frame_place(place)?;
        value::load_rax(operand, self.output);
        let complete = self.next_optional_label(&format!("{purpose}_complete"));
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(complete.clone()));
        let failure = self.next_optional_label(&format!("{purpose}_invalid"));
        let last = self.next_optional_label(&format!("{purpose}_last"));
        let location = self.current_runtime_trace_location()?;
        emit_release_loaded_handle(
            failure,
            last,
            complete.clone(),
            self.dispatch.finalizer_displacement(),
            location.as_ref(),
            super::super::call::TraceAttribution::SourceOperation,
            self.output,
        );
        self.output.push(Instruction::Label(complete));
        Ok(())
    }

    fn store_optional_shared_place(&mut self, place: &MirPlace) -> Result<(), BackendError> {
        self.output.push(Instruction::ReserveStack(
            OPTIONAL_SHARED_PRESERVED_HANDLE_SIZE,
        ));
        value::store_rax(value::memory(Register::Rsp, 0), self.output);
        self.materialize_place_address(place, Register::Rdx)?;
        value::load_rax(value::memory(Register::Rsp, 0), self.output);
        self.output.push(Instruction::ReleaseStack(
            OPTIONAL_SHARED_PRESERVED_HANDLE_SIZE,
        ));
        value::store_rax(value::memory(Register::Rdx, 0), self.output);
        Ok(())
    }
}
