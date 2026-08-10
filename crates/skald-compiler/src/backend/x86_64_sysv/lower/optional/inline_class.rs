//! Inline-class optional lifecycle lowering.

use crate::{
    backend::BackendError,
    mir::{
        MirClassOptionalAssign, MirClassOptionalCleanup, MirClassOptionalInitialize,
        MirClassOptionalPublish, MirClassOptionalSource, MirPlace, MirType,
    },
};

use super::{
    super::{
        super::machine::{Instruction, Label, Operand, Register},
        value, InstructionSelector,
    },
    offset_operand,
};

impl InstructionSelector<'_, '_> {
    pub(in crate::backend::x86_64_sysv::lower) fn select_class_optional_initialize(
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

    pub(in crate::backend::x86_64_sysv::lower) fn select_class_optional_publish(
        &mut self,
        publish: &MirClassOptionalPublish,
    ) -> Result<(), BackendError> {
        self.store_class_optional_state(&publish.destination, true)
    }

    pub(in crate::backend::x86_64_sysv::lower) fn select_class_optional_cleanup(
        &mut self,
        cleanup: &MirClassOptionalCleanup,
    ) -> Result<(), BackendError> {
        self.trap_if_class_optional_pinned(&cleanup.destination)?;
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

    pub(in crate::backend::x86_64_sysv::lower) fn select_class_optional_assign(
        &mut self,
        assignment: &MirClassOptionalAssign,
    ) -> Result<(), BackendError> {
        if matches!(&assignment.source, MirClassOptionalSource::Copy(source) if source == &assignment.destination)
        {
            return Ok(());
        }
        self.trap_if_class_optional_pinned(&assignment.destination)?;
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

    pub(super) fn load_class_optional_state(
        &mut self,
        place: &MirPlace,
    ) -> Result<(), BackendError> {
        let state = self.class_optional_state(place)?;
        value::load_rax(state, self.output);
        Ok(())
    }

    pub(super) fn store_class_optional_state(
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

    pub(super) fn class_optional_state(
        &mut self,
        place: &MirPlace,
    ) -> Result<Operand, BackendError> {
        let (frame, operand) = self.frame_place(place)?;
        let MirType::Optional(optional) = frame.ty() else {
            unreachable!("verified class optional operation has optional storage");
        };
        let offset = i32::try_from(self.data_layout.optional_type(optional)?.state_offset())
            .expect("optional state offset fits target displacement");
        offset_operand(operand, offset, self.function.callable())
    }
}
