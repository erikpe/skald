//! Optional presence tests, checked access, and view guards.

use crate::{
    backend::BackendError,
    mir::{
        MirOptionalBoxViewBegin, MirOptionalBoxViewEnd, MirOptionalViewEnd, MirPlace,
        MirPresenceTestKind, MirTerminator, MirType, ValueId,
    },
};

use super::super::{
    super::{
        machine::{Instruction, Label, Register},
        symbol,
    },
    block_label,
    ownership::emit_retain_loaded_handle,
    value, InstructionSelector,
};

impl InstructionSelector<'_, '_> {
    pub(in crate::backend::x86_64_sysv::lower) fn select_optional_presence(
        &mut self,
        source: &MirPlace,
        kind: MirPresenceTestKind,
        result: ValueId,
    ) -> Result<(), BackendError> {
        let destination = value::frame_value(self.frame, result);
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rax,
        });
        value::store_canonical_rax(MirType::Bool, destination, self.output);

        self.load_state(source)?;
        self.output.push(Instruction::Test(Register::Rax));
        let matched = optional_label(self.program, result, "matched");
        let finished = optional_label(self.program, result, "finished");
        self.output.push(match kind {
            MirPresenceTestKind::Some => Instruction::JumpIfNotZero(matched.clone()),
            MirPresenceTestKind::None => Instruction::JumpIfEqual(matched.clone()),
        });
        self.output.push(Instruction::Jump(finished.clone()));
        self.output.push(Instruction::Label(matched));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::Rax,
        });
        value::store_canonical_rax(MirType::Bool, destination, self.output);
        self.output.push(Instruction::Label(finished));
        Ok(())
    }

    pub(in crate::backend::x86_64_sysv::lower) fn select_optional_box_presence(
        &mut self,
        owner: crate::mir::StorageId,
        target: crate::identity::OptionalBoxTypeId,
        layer: usize,
        kind: MirPresenceTestKind,
        result: ValueId,
    ) -> Result<(), BackendError> {
        let destination = value::frame_value(self.frame, result);
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rax,
        });
        value::store_canonical_rax(MirType::Bool, destination, self.output);

        self.load_optional_box_state_parts(owner, target, layer)?;
        self.output.push(Instruction::Test(Register::Rax));
        let matched = optional_label(self.program, result, "box_matched");
        let finished = optional_label(self.program, result, "box_finished");
        self.output.push(match kind {
            MirPresenceTestKind::Some => Instruction::JumpIfNotZero(matched.clone()),
            MirPresenceTestKind::None => Instruction::JumpIfEqual(matched.clone()),
        });
        self.output.push(Instruction::Jump(finished.clone()));
        self.output.push(Instruction::Label(matched));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::Rax,
        });
        value::store_canonical_rax(MirType::Bool, destination, self.output);
        self.output.push(Instruction::Label(finished));
        Ok(())
    }

    pub(in crate::backend::x86_64_sysv::lower) fn select_optional_terminator(
        &mut self,
        terminator: &MirTerminator,
    ) -> Result<bool, BackendError> {
        match terminator {
            MirTerminator::OptionalSharedUnwrap {
                unwrap,
                success_target,
                failure_target,
                span,
            } => {
                let (_, source) = self.frame_place(&unwrap.source)?;
                value::load_rax(source, self.output);
                self.output.push(Instruction::Test(Register::Rax));
                self.output.push(Instruction::JumpIfEqual(block_label(
                    self.program,
                    *failure_target,
                )));
                let invalid = self.next_optional_label("shared_unwrap_invalid");
                let overflow = self.next_optional_label("shared_unwrap_overflow");
                emit_retain_loaded_handle(invalid.clone(), overflow.clone(), self.output);
                value::store_rax(
                    value::frame_storage(self.frame, unwrap.destination),
                    self.output,
                );
                self.output.push(Instruction::Jump(block_label(
                    self.program,
                    *success_target,
                )));
                self.output.push(Instruction::Label(overflow));
                let location = self.runtime_trace_location(*span)?;
                super::super::terminator::emit_ownership_overflow(
                    super::super::call::TraceAttribution::SourceOperation,
                    location.as_ref(),
                    self.output,
                );
                self.output.push(Instruction::Label(invalid));
                // The presence branch proves a non-null verified live handle.
                self.output.push(Instruction::Trap);
                Ok(true)
            }
            MirTerminator::OptionalUnwrap {
                source,
                destination,
                success_target,
                failure_target,
                ..
            } => {
                self.load_state(source)?;
                self.output.push(Instruction::Test(Register::Rax));
                self.output.push(Instruction::JumpIfEqual(block_label(
                    self.program,
                    *failure_target,
                )));
                let payload = self.optional_payload(source)?;
                self.copy_payload_to_storage(*destination, payload)?;
                self.output.push(Instruction::Jump(block_label(
                    self.program,
                    *success_target,
                )));
                Ok(true)
            }
            MirTerminator::BeginOptionalView {
                begin,
                success_target,
                absent_target,
                overflow_target,
                ..
            } => {
                self.load_class_optional_state(&begin.source)?;
                self.output.push(Instruction::Test(Register::Rax));
                self.output.push(Instruction::JumpIfEqual(block_label(
                    self.program,
                    *absent_target,
                )));
                self.output.push(Instruction::MoveImmediate64 {
                    bits: u64::MAX,
                    destination: Register::Rcx,
                });
                self.output.push(Instruction::Compare {
                    source: Register::Rcx,
                    destination: Register::Rax,
                });
                self.output.push(Instruction::JumpIfEqual(block_label(
                    self.program,
                    *overflow_target,
                )));
                self.output.push(Instruction::MoveImmediate64 {
                    bits: 1,
                    destination: Register::Rdx,
                });
                self.output.push(Instruction::Add {
                    source: Register::Rdx,
                    destination: Register::Rax,
                });
                self.output.push(Instruction::Move {
                    source: Register::Rax.into(),
                    destination: Register::Rdx.into(),
                });
                let state = self.class_optional_state(&begin.source)?;
                self.output.push(Instruction::Move {
                    source: Register::Rdx.into(),
                    destination: Register::Rax.into(),
                });
                value::store_rax(state, self.output);
                self.output.push(Instruction::Jump(block_label(
                    self.program,
                    *success_target,
                )));
                Ok(true)
            }
            MirTerminator::BeginOptionalBoxView {
                begin,
                success_target,
                absent_target,
                overflow_target,
                ..
            } => {
                self.load_optional_box_state(begin)?;
                self.output.push(Instruction::Test(Register::Rax));
                self.output.push(Instruction::JumpIfEqual(block_label(
                    self.program,
                    *absent_target,
                )));
                self.output.push(Instruction::MoveImmediate64 {
                    bits: u64::MAX,
                    destination: Register::Rcx,
                });
                self.output.push(Instruction::Compare {
                    source: Register::Rcx,
                    destination: Register::Rax,
                });
                self.output.push(Instruction::JumpIfEqual(block_label(
                    self.program,
                    *overflow_target,
                )));
                self.output.push(Instruction::MoveImmediate64 {
                    bits: 1,
                    destination: Register::Rdx,
                });
                self.output.push(Instruction::Add {
                    source: Register::Rdx,
                    destination: Register::Rax,
                });
                self.store_optional_box_state(begin.owner, begin.box_target, begin.layer)?;
                self.output.push(Instruction::Jump(block_label(
                    self.program,
                    *success_target,
                )));
                Ok(true)
            }
            MirTerminator::CheckOptionalMutation {
                source,
                success_target,
                failure_target,
                ..
            } => {
                self.load_class_optional_state(source)?;
                self.output.push(Instruction::Test(Register::Rax));
                self.output.push(Instruction::JumpIfEqual(block_label(
                    self.program,
                    *success_target,
                )));
                self.output.push(Instruction::MoveImmediate64 {
                    bits: 1,
                    destination: Register::Rcx,
                });
                self.output.push(Instruction::Compare {
                    source: Register::Rcx,
                    destination: Register::Rax,
                });
                self.output.push(Instruction::JumpIfEqual(block_label(
                    self.program,
                    *success_target,
                )));
                self.output.push(Instruction::Jump(block_label(
                    self.program,
                    *failure_target,
                )));
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub(in crate::backend::x86_64_sysv::lower) fn select_optional_view_end(
        &mut self,
        end: &MirOptionalViewEnd,
    ) -> Result<(), BackendError> {
        self.load_class_optional_state(&end.source)?;
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::Rdx,
        });
        self.output.push(Instruction::Subtract {
            source: Register::Rdx,
            destination: Register::Rax,
        });
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdx.into(),
        });
        let state = self.class_optional_state(&end.source)?;
        self.output.push(Instruction::Move {
            source: Register::Rdx.into(),
            destination: Register::Rax.into(),
        });
        value::store_rax(state, self.output);
        Ok(())
    }

    pub(in crate::backend::x86_64_sysv::lower) fn select_optional_box_view_end(
        &mut self,
        end: &MirOptionalBoxViewEnd,
    ) -> Result<(), BackendError> {
        self.load_optional_box_state_parts(end.owner, end.box_target, end.layer)?;
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::Rdx,
        });
        self.output.push(Instruction::Subtract {
            source: Register::Rdx,
            destination: Register::Rax,
        });
        self.store_optional_box_state(end.owner, end.box_target, end.layer)
    }

    fn load_optional_box_state(
        &mut self,
        begin: &MirOptionalBoxViewBegin,
    ) -> Result<(), BackendError> {
        self.load_optional_box_state_parts(begin.owner, begin.box_target, begin.layer)
    }

    fn load_optional_box_state_parts(
        &mut self,
        owner: crate::mir::StorageId,
        target: crate::identity::OptionalBoxTypeId,
        layer: usize,
    ) -> Result<(), BackendError> {
        value::load_rax(value::frame_storage(self.frame, owner), self.output);
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::R11.into(),
        });
        let offset = super::super::super::layout::SHARED_HEADER_SIZE
            .checked_add(
                self.data_layout
                    .optional_object_box_layer_offset(target, layer)?,
            )
            .and_then(|offset| i32::try_from(offset).ok())
            .ok_or_else(|| {
                BackendError::new(
                    crate::backend::Target::X86_64SysV,
                    Some(self.function.callable()),
                    "optional-box guard offset exceeds x86-64 displacement limits",
                )
            })?;
        value::load_rax(value::memory(Register::R11, offset), self.output);
        Ok(())
    }

    fn store_optional_box_state(
        &mut self,
        owner: crate::mir::StorageId,
        target: crate::identity::OptionalBoxTypeId,
        layer: usize,
    ) -> Result<(), BackendError> {
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdx.into(),
        });
        value::load_rax(value::frame_storage(self.frame, owner), self.output);
        let offset = super::super::super::layout::SHARED_HEADER_SIZE
            .checked_add(
                self.data_layout
                    .optional_object_box_layer_offset(target, layer)?,
            )
            .and_then(|offset| i32::try_from(offset).ok())
            .ok_or_else(|| {
                BackendError::new(
                    crate::backend::Target::X86_64SysV,
                    Some(self.function.callable()),
                    "optional-box guard offset exceeds x86-64 displacement limits",
                )
            })?;
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::R11.into(),
        });
        self.output.push(Instruction::Move {
            source: Register::Rdx.into(),
            destination: Register::Rax.into(),
        });
        value::store_rax(value::memory(Register::R11, offset), self.output);
        Ok(())
    }

    pub(super) fn trap_if_class_optional_pinned(
        &mut self,
        place: &MirPlace,
    ) -> Result<(), BackendError> {
        let allowed = self.next_optional_label("mutation_allowed");
        self.load_class_optional_state(place)?;
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(allowed.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::Rcx,
        });
        self.output.push(Instruction::Compare {
            source: Register::Rcx,
            destination: Register::Rax,
        });
        self.output.push(Instruction::JumpIfEqual(allowed.clone()));
        // MIR routes source-reachable guarded mutation through its static
        // termination reason; reaching this redundant check is a backend defect.
        self.output.push(Instruction::Trap);
        self.output.push(Instruction::Label(allowed));
        Ok(())
    }
}
fn optional_label(program: &crate::mir::MirProgram, result: ValueId, suffix: &str) -> Label {
    Label::new(format!(
        ".Lska.{}.optional_test_{}_{}",
        symbol::local_label_stem(program, result.callable()),
        result.index(),
        suffix
    ))
}
