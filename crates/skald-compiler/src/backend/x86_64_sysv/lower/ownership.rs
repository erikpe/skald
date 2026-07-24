//! x86-64 realization of verified shared-owner operations.
//!
//! This is the sole owner of allocation-header offsets, count transitions,
//! runtime allocation symbols, and last-owner finalizer selection.

use crate::{
    backend::{BackendError, Target},
    mir::{
        MirSharedAdopt, MirSharedAllocate, MirSharedCopy, MirSharedPublish, MirSharedRelease,
        MirType,
    },
};

use super::{
    super::{
        layout::SHARED_HEADER_SIZE,
        machine::{Instruction, Label, Register},
        symbol,
    },
    value, InstructionSelector,
};

const STRONG_COUNT_OFFSET: i32 = 0;
const DYNAMIC_METADATA_OFFSET: i32 = 8;
const RUNTIME_ALLOC: &str = "ska_rt_alloc";
const RUNTIME_FREE: &str = "ska_rt_free";

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
            value::memory(Register::R11, DYNAMIC_METADATA_OFFSET),
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
        let (failure, complete) = self.ownership_labels("retain");
        self.load_shared_handle(copy.source, Register::Rax);
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(failure.clone()));
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rax, STRONG_COUNT_OFFSET),
            destination: Register::Rcx.into(),
        });
        self.output.push(Instruction::Test(Register::Rcx));
        self.output.push(Instruction::JumpIfEqual(failure.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::MAX,
            destination: Register::R11,
        });
        self.output.push(Instruction::Compare {
            source: Register::R11,
            destination: Register::Rcx,
        });
        self.output.push(Instruction::JumpIfEqual(failure.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        });
        self.output.push(Instruction::Add {
            source: Register::R11,
            destination: Register::Rcx,
        });
        self.output.push(Instruction::Move {
            source: Register::Rcx.into(),
            destination: value::memory(Register::Rax, STRONG_COUNT_OFFSET),
        });
        value::store_rax(
            value::frame_storage(self.frame, copy.destination),
            self.output,
        );
        self.output.push(Instruction::Jump(complete.clone()));
        self.output.push(Instruction::Label(failure));
        self.output.push(Instruction::Trap);
        self.output.push(Instruction::Label(complete));
    }

    pub(super) fn select_shared_release(&mut self, release: &MirSharedRelease) {
        let (failure, complete) = self.ownership_labels("release");
        let last = self.ownership_label("release_last");
        self.load_shared_handle(release.owner, Register::Rax);
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(failure.clone()));
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rax, STRONG_COUNT_OFFSET),
            destination: Register::Rcx.into(),
        });
        self.output.push(Instruction::Test(Register::Rcx));
        self.output.push(Instruction::JumpIfEqual(failure.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        });
        self.output.push(Instruction::Compare {
            source: Register::R11,
            destination: Register::Rcx,
        });
        self.output.push(Instruction::JumpIfEqual(last.clone()));
        self.output.push(Instruction::Subtract {
            source: Register::R11,
            destination: Register::Rcx,
        });
        self.output.push(Instruction::Move {
            source: Register::Rcx.into(),
            destination: value::memory(Register::Rax, STRONG_COUNT_OFFSET),
        });
        self.output.push(Instruction::Jump(complete.clone()));

        self.output.push(Instruction::Label(last));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::R11,
        });
        self.output.push(Instruction::Move {
            source: Register::R11.into(),
            destination: value::memory(Register::Rax, STRONG_COUNT_OFFSET),
        });
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rax, DYNAMIC_METADATA_OFFSET),
            destination: Register::R11.into(),
        });
        self.output.push(Instruction::Test(Register::R11));
        self.output.push(Instruction::JumpIfEqual(failure.clone()));
        self.output.push(Instruction::Move {
            source: value::memory(Register::R11, self.dispatch.finalizer_displacement()),
            destination: Register::R11.into(),
        });
        self.output.push(Instruction::Test(Register::R11));
        self.output.push(Instruction::JumpIfEqual(failure.clone()));
        self.output.push(Instruction::LoadEffectiveAddress {
            source: value::memory(Register::Rax, SHARED_HEADER_SIZE as i32),
            destination: Register::Rdi,
        });
        self.output.push(Instruction::CallIndirect(Register::R11));
        self.load_shared_handle(release.owner, Register::Rdi);
        self.output.push(Instruction::Call(RUNTIME_FREE.to_owned()));
        self.output.push(Instruction::Jump(complete.clone()));

        self.output.push(Instruction::Label(failure));
        self.output.push(Instruction::Trap);
        self.output.push(Instruction::Label(complete));
    }

    fn load_shared_handle(&mut self, storage: crate::mir::StorageId, destination: Register) {
        self.output.push(Instruction::Move {
            source: value::frame_storage(self.frame, storage),
            destination: destination.into(),
        });
    }

    fn ownership_labels(&self, operation: &str) -> (Label, Label) {
        (
            self.ownership_label(&format!("{operation}_invalid")),
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
