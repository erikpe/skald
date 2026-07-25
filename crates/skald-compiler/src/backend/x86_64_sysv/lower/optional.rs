//! Primitive optional layout operations and checked-access control flow.

use crate::{
    backend::BackendError,
    mir::{
        MirClassOptionalAssign, MirClassOptionalCleanup, MirClassOptionalInitialize,
        MirClassOptionalPublish, MirClassOptionalSource, MirOptionalAssign, MirOptionalInitialize,
        MirOptionalSource, MirPlace, MirPresenceTestKind, MirPrimitiveType, MirTerminationReason,
        MirTerminator, MirType, StorageId, ValueId,
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
    pub(super) fn select_class_optional_initialize(
        &mut self,
        initialize: &MirClassOptionalInitialize,
    ) -> Result<(), BackendError> {
        match &initialize.source {
            MirClassOptionalSource::Absent => {
                self.store_class_optional_state(&initialize.destination, false)
            }
            MirClassOptionalSource::Present(source) => {
                let operation = initialize
                    .copy_constructor
                    .expect("verified present class optional initialization requires copy");
                self.select_construction_operation(
                    operation,
                    initialize
                        .destination
                        .clone()
                        .project_optional_payload(initialize.class),
                    source.clone(),
                )?;
                self.store_class_optional_state(&initialize.destination, true)
            }
            MirClassOptionalSource::Copy(source) => self.select_class_optional_copy_initialize(
                &initialize.destination,
                source,
                initialize.class,
                initialize
                    .copy_constructor
                    .expect("verified class optional copy requires copy construction"),
            ),
        }
    }

    fn select_class_optional_copy_initialize(
        &mut self,
        destination: &MirPlace,
        source: &MirPlace,
        class: crate::identity::ClassId,
        operation: crate::mir::MirSelectedCopyOperation<crate::identity::CopyConstructorId>,
    ) -> Result<(), BackendError> {
        let present = self.next_optional_label("class_copy_present");
        let finished = self.next_optional_label("class_copy_finished");
        self.load_class_optional_state(source)?;
        self.output.push(Instruction::Test(Register::Rax));
        self.output
            .push(Instruction::JumpIfNotZero(present.clone()));
        self.store_class_optional_state(destination, false)?;
        self.output.push(Instruction::Jump(finished.clone()));
        self.output.push(Instruction::Label(present));
        self.select_construction_operation(
            operation,
            destination.clone().project_optional_payload(class),
            source.clone().project_optional_payload(class),
        )?;
        self.store_class_optional_state(destination, true)?;
        self.output.push(Instruction::Label(finished));
        Ok(())
    }

    pub(super) fn select_class_optional_publish(
        &mut self,
        publish: &MirClassOptionalPublish,
    ) -> Result<(), BackendError> {
        self.store_class_optional_state(&publish.destination, true)
    }

    pub(super) fn select_class_optional_cleanup(
        &mut self,
        cleanup: &MirClassOptionalCleanup,
    ) -> Result<(), BackendError> {
        let finished = self.next_optional_label("class_cleanup_finished");
        self.load_class_optional_state(&cleanup.destination)?;
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(finished.clone()));
        self.select_destruction_plan(
            cleanup.class,
            cleanup
                .destination
                .clone()
                .project_optional_payload(cleanup.class),
        )?;
        self.store_class_optional_state(&cleanup.destination, false)?;
        self.output.push(Instruction::Label(finished));
        Ok(())
    }

    pub(super) fn select_class_optional_assign(
        &mut self,
        assignment: &MirClassOptionalAssign,
    ) -> Result<(), BackendError> {
        if matches!(&assignment.source, MirClassOptionalSource::Copy(source) if source == &assignment.destination)
        {
            return Ok(());
        }
        let source_present = self.next_optional_label("class_source_present");
        let destination_present = self.next_optional_label("class_destination_present");
        let finished = self.next_optional_label("class_assign_finished");

        if let MirClassOptionalSource::Copy(source) = &assignment.source {
            self.load_class_optional_state(source)?;
            self.output.push(Instruction::Test(Register::Rax));
            self.output
                .push(Instruction::JumpIfNotZero(source_present.clone()));
            let absent_cleaned = self.next_optional_label("class_source_absent_cleaned");
            self.destroy_class_optional_if_present(
                &assignment.destination,
                assignment.class,
                &absent_cleaned,
            )?;
            self.output.push(Instruction::Jump(finished.clone()));
            self.output.push(Instruction::Label(source_present));
        } else if matches!(assignment.source, MirClassOptionalSource::Absent) {
            return self.destroy_class_optional_if_present(
                &assignment.destination,
                assignment.class,
                &finished,
            );
        }

        self.load_class_optional_state(&assignment.destination)?;
        self.output.push(Instruction::Test(Register::Rax));
        self.output
            .push(Instruction::JumpIfNotZero(destination_present.clone()));
        let source = match &assignment.source {
            MirClassOptionalSource::Present(source) => source.clone(),
            MirClassOptionalSource::Copy(source) => {
                source.clone().project_optional_payload(assignment.class)
            }
            MirClassOptionalSource::Absent => unreachable!(),
        };
        self.select_construction_operation(
            assignment
                .copy_constructor
                .expect("verified class optional assignment requires copy construction"),
            assignment
                .destination
                .clone()
                .project_optional_payload(assignment.class),
            source.clone(),
        )?;
        self.store_class_optional_state(&assignment.destination, true)?;
        self.output.push(Instruction::Jump(finished.clone()));
        self.output.push(Instruction::Label(destination_present));
        self.select_assignment_operation(
            assignment
                .copy_assignment
                .expect("verified class optional assignment requires copy assignment"),
            assignment
                .destination
                .clone()
                .project_optional_payload(assignment.class),
            source,
        )?;
        self.output.push(Instruction::Label(finished));
        Ok(())
    }

    fn destroy_class_optional_if_present(
        &mut self,
        destination: &MirPlace,
        class: crate::identity::ClassId,
        finished: &Label,
    ) -> Result<(), BackendError> {
        self.load_class_optional_state(destination)?;
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(finished.clone()));
        self.select_destruction_plan(class, destination.clone().project_optional_payload(class))?;
        self.store_class_optional_state(destination, false)?;
        self.output.push(Instruction::Label(finished.clone()));
        Ok(())
    }

    fn load_class_optional_state(&mut self, place: &MirPlace) -> Result<(), BackendError> {
        let state = self.class_optional_state(place)?;
        value::load_rax(state, self.output);
        Ok(())
    }

    fn store_class_optional_state(
        &mut self,
        place: &MirPlace,
        present: bool,
    ) -> Result<(), BackendError> {
        let state = self.class_optional_state(place)?;
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::from(present),
            destination: Register::Rax,
        });
        value::store_rax(state, self.output);
        Ok(())
    }

    fn class_optional_state(&mut self, place: &MirPlace) -> Result<Operand, BackendError> {
        let (frame, operand) = self.frame_place(place)?;
        let MirType::OptionalClass(class) = frame.ty() else {
            unreachable!("verified class optional operation has optional storage");
        };
        let offset = i32::try_from(self.data_layout.optional_class(class)?.state_offset())
            .expect("optional state offset fits target displacement");
        offset_operand(operand, offset, self.function.callable())
    }

    pub(super) fn select_optional_initialize(
        &mut self,
        initialize: &MirOptionalInitialize,
    ) -> Result<(), BackendError> {
        self.select_optional_write(&initialize.destination, &initialize.source)
    }

    pub(super) fn select_optional_assign(
        &mut self,
        assignment: &MirOptionalAssign,
    ) -> Result<(), BackendError> {
        self.select_optional_write(&assignment.destination, &assignment.source)
    }

    pub(super) fn select_optional_presence(
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
                self.load_state(source)?;
                self.output.push(Instruction::Test(Register::Rax));
                self.output
                    .push(Instruction::JumpIfEqual(block_label(*failure_target)));
                let payload = self.optional_payload(source)?;
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

    pub(super) fn select_optional_write(
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

    fn load_state(&mut self, place: &MirPlace) -> Result<(), BackendError> {
        let (frame, _) = self.frame_place(place)?;
        let state = match frame.ty() {
            MirType::OptionalPrimitive(_) => self.optional_state(place)?,
            MirType::OptionalClass(_) => self.class_optional_state(place)?,
            _ => unreachable!("verified presence test has optional storage"),
        };
        value::load_rax(state, self.output);
        Ok(())
    }

    fn store_state(&mut self, place: &MirPlace, present: bool) -> Result<(), BackendError> {
        let state = self.optional_state(place)?;
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::from(present),
            destination: Register::Rax,
        });
        value::store_rax(state, self.output);
        Ok(())
    }

    fn optional_state(&mut self, place: &MirPlace) -> Result<Operand, BackendError> {
        let (payload, operand) = self.optional_base(place)?;
        let layout = self.data_layout.optional(payload)?;
        let offset = i32::try_from(layout.state_offset())
            .expect("optional state offset fits the target displacement");
        offset_operand(operand, offset, self.function.callable())
    }

    fn optional_payload(&mut self, place: &MirPlace) -> Result<OptionalPayload, BackendError> {
        let (payload, operand) = self.optional_base(place)?;
        let layout = self.data_layout.optional(payload)?;
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
    ) -> Result<(MirPrimitiveType, Operand), BackendError> {
        let (layout, operand) = self.frame_place(place)?;
        let ty = layout.ty();
        let MirType::OptionalPrimitive(payload) = ty else {
            unreachable!("verified optional operation has optional storage");
        };
        Ok((payload, operand))
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

    fn stabilize_optional_operand(&mut self, operand: Operand) -> Operand {
        let Operand::Memory {
            base: Register::R11,
            displacement,
        } = operand
        else {
            return operand;
        };
        self.output.push(Instruction::Move {
            source: Register::R11.into(),
            destination: Register::Rcx.into(),
        });
        value::memory(Register::Rcx, displacement)
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

fn offset_operand(
    operand: Operand,
    offset: i32,
    callable: crate::identity::CallableId,
) -> Result<Operand, BackendError> {
    let Operand::Memory { base, displacement } = operand else {
        unreachable!("optional places lower to memory operands");
    };
    let displacement = displacement.checked_add(offset).ok_or_else(|| {
        BackendError::new(
            crate::backend::Target::X86_64SysV,
            Some(callable),
            "optional payload displacement exceeds x86-64 limits",
        )
    })?;
    Ok(value::memory(base, displacement))
}

fn optional_label(result: ValueId, suffix: &str) -> Label {
    Label::new(format!(
        ".Lska_{}_optional_test_{}_{}",
        symbol::local_label_stem(result.callable()),
        result.index(),
        suffix
    ))
}
