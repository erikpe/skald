//! Primitive optional layout operations and checked-access control flow.

use crate::{
    backend::BackendError,
    mir::{
        MirOptionalAssign, MirOptionalInitialize, MirOptionalSource, MirPresenceTestKind,
        MirPrimitiveType, MirTerminationReason, MirTerminator, MirType, StorageId, ValueId,
    },
};

use super::{
    super::{
        machine::{ByteRegister, Instruction, Label, Operand, Register, XmmRegister},
        symbol,
    },
    block_label, value, InstructionSelector,
};

type OptionalPayload = (MirPrimitiveType, Operand);

impl InstructionSelector<'_, '_> {
    pub(super) fn select_optional_initialize(
        &mut self,
        initialize: &MirOptionalInitialize,
    ) -> Result<(), BackendError> {
        self.select_optional_write(initialize.destination, initialize.source)
    }

    pub(super) fn select_optional_assign(
        &mut self,
        assignment: &MirOptionalAssign,
    ) -> Result<(), BackendError> {
        self.select_optional_write(assignment.destination, assignment.source)
    }

    pub(super) fn select_optional_presence(
        &mut self,
        source: StorageId,
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
        let matched = optional_label(result, "matched");
        let finished = optional_label(result, "finished");
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

    pub(super) fn select_optional_terminator(
        &mut self,
        terminator: &MirTerminator,
    ) -> Result<bool, BackendError> {
        match terminator {
            MirTerminator::OptionalUnwrap {
                source,
                destination,
                success_target,
                failure_target,
                ..
            } => {
                self.load_state(*source)?;
                self.output.push(Instruction::Test(Register::Rax));
                self.output
                    .push(Instruction::JumpIfEqual(block_label(*failure_target)));
                let payload = self.optional_payload(*source)?;
                self.copy_payload_to_storage(*destination, payload)?;
                self.output
                    .push(Instruction::Jump(block_label(*success_target)));
                Ok(true)
            }
            MirTerminator::Terminate {
                reason: MirTerminationReason::OptionalAccessFailure,
                ..
            } => {
                self.output.push(Instruction::Trap);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn select_optional_write(
        &mut self,
        destination: StorageId,
        source: MirOptionalSource,
    ) -> Result<(), BackendError> {
        match source {
            MirOptionalSource::Absent => self.store_state(destination, false)?,
            MirOptionalSource::Present(value_id) => {
                let payload = self.optional_payload(destination)?;
                self.copy_value_to_payload(value_id, payload)?;
                self.store_state(destination, true)?;
            }
            MirOptionalSource::Copy(source) => {
                let present = self.next_optional_label("present");
                let finished = self.next_optional_label("finished");
                self.load_state(source)?;
                self.output.push(Instruction::Test(Register::Rax));
                self.output
                    .push(Instruction::JumpIfNotZero(present.clone()));
                self.store_state(destination, false)?;
                self.output.push(Instruction::Jump(finished.clone()));
                self.output.push(Instruction::Label(present));
                let source_payload = self.optional_payload(source)?;
                let destination_payload = self.optional_payload(destination)?;
                self.copy_payload(source_payload, destination_payload);
                self.store_state(destination, true)?;
                self.output.push(Instruction::Label(finished));
            }
        }
        Ok(())
    }

    fn load_state(&mut self, storage: StorageId) -> Result<(), BackendError> {
        let state = self.optional_state(storage)?;
        value::load_rax(state, self.output);
        Ok(())
    }

    fn store_state(&mut self, storage: StorageId, present: bool) -> Result<(), BackendError> {
        let state = self.optional_state(storage)?;
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::from(present),
            destination: Register::Rax,
        });
        value::store_rax(state, self.output);
        Ok(())
    }

    fn optional_state(&self, storage: StorageId) -> Result<Operand, BackendError> {
        let payload = self.optional_type(storage)?;
        let layout = self.data_layout.optional(payload)?;
        let offset = i32::try_from(layout.state_offset())
            .expect("optional state offset fits the target displacement");
        Ok(value::memory(
            Register::Rbp,
            self.frame.storage(storage) + offset,
        ))
    }

    fn optional_payload(&self, storage: StorageId) -> Result<OptionalPayload, BackendError> {
        let payload = self.optional_type(storage)?;
        let layout = self.data_layout.optional(payload)?;
        let offset = i32::try_from(layout.payload_offset())
            .expect("optional payload offset fits the target displacement");
        Ok((
            payload,
            value::memory(Register::Rbp, self.frame.storage(storage) + offset),
        ))
    }

    fn optional_type(&self, storage: StorageId) -> Result<MirPrimitiveType, BackendError> {
        let ty = self
            .function
            .storage(storage)
            .expect("verified optional operation identifies storage")
            .ty;
        let MirType::OptionalPrimitive(payload) = ty else {
            unreachable!("verified optional operation has optional storage");
        };
        Ok(payload)
    }

    fn copy_value_to_payload(
        &mut self,
        source: ValueId,
        destination: OptionalPayload,
    ) -> Result<(), BackendError> {
        let source = value::frame_value(self.frame, source);
        self.copy_payload((destination.0, source), destination);
        Ok(())
    }

    fn copy_payload_to_storage(
        &mut self,
        destination: StorageId,
        source: OptionalPayload,
    ) -> Result<(), BackendError> {
        let destination = value::frame_storage(self.frame, destination);
        self.copy_payload(source, (source.0, destination));
        Ok(())
    }

    fn copy_payload(&mut self, source: OptionalPayload, destination: OptionalPayload) {
        debug_assert_eq!(source.0, destination.0);
        match source.0.payload_type() {
            MirType::F64 => {
                value::load_float(
                    value::float_operand(source.1),
                    XmmRegister::Xmm14,
                    self.output,
                );
                value::store_float(
                    XmmRegister::Xmm14,
                    value::float_operand(destination.1),
                    self.output,
                );
            }
            MirType::U8 | MirType::Bool => {
                value::load_byte_rax(source.1, self.output);
                self.output.push(Instruction::MoveByte {
                    source: ByteRegister::Al,
                    destination: destination.1,
                });
            }
            MirType::I64 | MirType::U64 => {
                value::load_rax(source.1, self.output);
                value::store_rax(destination.1, self.output);
            }
            _ => unreachable!("primitive optional payload must be primitive"),
        }
    }

    fn next_optional_label(&mut self, suffix: &str) -> Label {
        let sequence = self.optional_sequence;
        self.optional_sequence += 1;
        Label::new(format!(
            ".Lska_{}_optional_{}_{}_{}",
            symbol::local_label_stem(self.function.callable()),
            self.block.index(),
            sequence,
            suffix
        ))
    }
}

fn optional_label(result: ValueId, suffix: &str) -> Label {
    Label::new(format!(
        ".Lska_{}_optional_test_{}_{}",
        symbol::local_label_stem(result.callable()),
        result.index(),
        suffix
    ))
}
