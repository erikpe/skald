//! Scalar optional writes, layout access, and payload movement.

use crate::{
    backend::BackendError,
    mir::{
        MirOptionalAssign, MirOptionalInitialize, MirOptionalSource, MirPlace, MirPrimitiveType,
        MirType, StorageId, ValueId,
    },
};

use super::{
    super::{
        super::{
            machine::{ByteRegister, Instruction, Label, Operand, Register, XmmRegister},
            symbol,
        },
        value, InstructionSelector,
    },
    offset_operand,
};

pub(super) type OptionalPayload = (MirPrimitiveType, Operand);

impl InstructionSelector<'_, '_> {
    pub(in crate::backend::x86_64_sysv::lower) fn select_optional_initialize(
        &mut self,
        initialize: &MirOptionalInitialize,
    ) -> Result<(), BackendError> {
        self.select_optional_write(&initialize.destination, &initialize.source)
    }

    pub(in crate::backend::x86_64_sysv::lower) fn select_optional_assign(
        &mut self,
        assignment: &MirOptionalAssign,
    ) -> Result<(), BackendError> {
        self.select_optional_write(&assignment.destination, &assignment.source)
    }
    pub(in crate::backend::x86_64_sysv::lower) fn select_optional_write(
        &mut self,
        destination: &MirPlace,
        source: &MirOptionalSource,
    ) -> Result<(), BackendError> {
        match source {
            MirOptionalSource::Absent => self.store_state(destination, false)?,
            MirOptionalSource::Present(value_id) => {
                let payload = self.optional_payload(destination)?;
                self.copy_value_to_payload(*value_id, payload)?;
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
                // Place lowering uses `r11` for indirect bases. Preserve the
                // destination address before lowering the source so projected
                // or parameter-backed places cannot alias the scratch register.
                let destination_payload = self.optional_payload(destination)?;
                let destination_payload = (
                    destination_payload.0,
                    self.stabilize_optional_operand(destination_payload.1),
                );
                let source_payload = self.optional_payload(source)?;
                self.copy_payload(source_payload, destination_payload);
                self.store_state(destination, true)?;
                self.output.push(Instruction::Label(finished));
            }
        }
        Ok(())
    }

    pub(super) fn load_state(&mut self, place: &MirPlace) -> Result<(), BackendError> {
        let (frame, _) = self.frame_place(place)?;
        let state = match frame.ty() {
            MirType::Optional(optional)
                if self
                    .program
                    .optional_type(optional)
                    .is_some_and(|metadata| {
                        metadata.representation
                            == crate::mir::MirOptionalRepresentation::NullableSharedOwner
                    }) =>
            {
                let (_, operand) = self.frame_place(place)?;
                operand
            }
            MirType::Optional(_) => self.optional_state(place)?,
            _ => unreachable!("verified presence test has optional storage"),
        };
        value::load_rax(state, self.output);
        Ok(())
    }

    pub(super) fn store_state(
        &mut self,
        place: &MirPlace,
        present: bool,
    ) -> Result<(), BackendError> {
        let state = self.optional_state(place)?;
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::from(present),
            destination: Register::Rax,
        });
        value::store_rax(state, self.output);
        Ok(())
    }

    fn optional_state(&mut self, place: &MirPlace) -> Result<Operand, BackendError> {
        let (optional, operand) = self.optional_base(place)?;
        let layout = self.data_layout.optional_type(optional)?;
        let offset = i32::try_from(layout.state_offset())
            .expect("optional state offset fits the target displacement");
        offset_operand(operand, offset, self.function.callable())
    }

    pub(super) fn optional_payload(
        &mut self,
        place: &MirPlace,
    ) -> Result<OptionalPayload, BackendError> {
        let (optional, operand) = self.optional_base(place)?;
        let payload = self
            .program
            .optional_type(optional)
            .and_then(crate::mir::MirOptionalType::primitive)
            .expect("verified scalar optional must have primitive metadata");
        let layout = self.data_layout.optional_type(optional)?;
        let offset = i32::try_from(layout.payload_offset())
            .expect("optional payload offset fits the target displacement");
        Ok((
            payload,
            offset_operand(operand, offset, self.function.callable())?,
        ))
    }

    fn optional_base(
        &mut self,
        place: &MirPlace,
    ) -> Result<(crate::identity::OptionalTypeId, Operand), BackendError> {
        let (layout, operand) = self.frame_place(place)?;
        let ty = layout.ty();
        let MirType::Optional(optional) = ty else {
            unreachable!("verified optional operation has optional storage");
        };
        Ok((optional, operand))
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

    pub(super) fn copy_payload_to_storage(
        &mut self,
        destination: StorageId,
        source: OptionalPayload,
    ) -> Result<(), BackendError> {
        let destination = value::frame_storage(self.frame, destination);
        match source.0.payload_type() {
            MirType::F64 => {
                value::load_float(
                    value::float_operand(source.1),
                    XmmRegister::Xmm14,
                    self.output,
                );
                value::store_float(
                    XmmRegister::Xmm14,
                    value::float_operand(destination),
                    self.output,
                );
            }
            MirType::U8 | MirType::Bool => {
                // MIR scalar values have canonical eight-byte homes even when
                // their source storage uses one byte. Clear the upper bytes so
                // every later full-register consumer observes the same value.
                value::load_byte_rax(source.1, self.output);
                value::store_rax(destination, self.output);
            }
            MirType::I64 | MirType::U64 => {
                value::load_rax(source.1, self.output);
                value::store_rax(destination, self.output);
            }
            _ => unreachable!("primitive optional payload must be primitive"),
        }
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

    fn stabilize_optional_operand(&mut self, operand: Operand) -> Operand {
        match operand {
            Operand::Memory {
                base: Register::R11,
                ..
            }
            | Operand::IndexedMemory {
                base: Register::R11,
                ..
            } => {
                self.output.push(Instruction::LoadEffectiveAddress {
                    source: operand,
                    destination: Register::Rdx,
                });
                value::memory(Register::Rdx, 0)
            }
            _ => operand,
        }
    }

    pub(super) fn next_optional_label(&mut self, suffix: &str) -> Label {
        let sequence = self.optional_sequence;
        self.optional_sequence += 1;
        Label::new(format!(
            ".Lska.{}.optional_{}_{}_{}",
            symbol::local_label_stem(self.program, self.function.callable()),
            self.block.index(),
            sequence,
            suffix
        ))
    }
}
