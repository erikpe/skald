//! Aggregate optional lifecycle lowering.

use crate::{
    backend::BackendError,
    mir::{
        MirAggregateOptionalAssign, MirAggregateOptionalCleanup, MirAggregateOptionalInitialize,
        MirAggregateOptionalSource, MirClassOptionalAssign, MirClassOptionalCleanup,
        MirClassOptionalInitialize, MirClassOptionalSource, MirOptionalAssign,
        MirOptionalInitialize, MirOptionalSharedAssign, MirOptionalSharedCleanup,
        MirOptionalSharedInitialize, MirOptionalSharedSource, MirOptionalSource, MirPlace,
    },
};

use super::super::{
    super::machine::{Instruction, Register},
    InstructionSelector,
};

impl InstructionSelector<'_, '_> {
    pub(in crate::backend::x86_64_sysv::lower) fn select_aggregate_optional_initialize(
        &mut self,
        initialize: &MirAggregateOptionalInitialize,
    ) -> Result<(), BackendError> {
        match &initialize.source {
            MirAggregateOptionalSource::Absent | MirAggregateOptionalSource::Unpublished => {
                self.store_state(&initialize.destination, false)
            }
            MirAggregateOptionalSource::Copy(source) => self.copy_initialize_aggregate_optional(
                initialize.optional,
                &initialize.destination,
                source,
            ),
        }
    }

    pub(in crate::backend::x86_64_sysv::lower) fn select_aggregate_optional_publish(
        &mut self,
        publish: &crate::mir::MirAggregateOptionalPublish,
    ) -> Result<(), BackendError> {
        self.store_state(&publish.destination, true)
    }

    pub(in crate::backend::x86_64_sysv::lower) fn select_aggregate_optional_cleanup(
        &mut self,
        cleanup: &MirAggregateOptionalCleanup,
    ) -> Result<(), BackendError> {
        self.cleanup_aggregate_optional(cleanup.optional, &cleanup.destination)
    }

    pub(in crate::backend::x86_64_sysv::lower) fn select_aggregate_optional_assign(
        &mut self,
        assignment: &MirAggregateOptionalAssign,
    ) -> Result<(), BackendError> {
        if matches!(&assignment.source, MirAggregateOptionalSource::Copy(source) if source == &assignment.destination)
        {
            return Ok(());
        }
        let source = match &assignment.source {
            MirAggregateOptionalSource::Absent | MirAggregateOptionalSource::Unpublished => {
                return self
                    .cleanup_aggregate_optional(assignment.optional, &assignment.destination);
            }
            MirAggregateOptionalSource::Copy(source) => source,
        };
        let source_present = self.next_optional_label("nested_source_present");
        let destination_present = self.next_optional_label("nested_destination_present");
        let finished = self.next_optional_label("nested_assign_finished");
        self.load_state(source)?;
        self.output.push(Instruction::Test(Register::Rax));
        self.output
            .push(Instruction::JumpIfNotZero(source_present.clone()));
        self.cleanup_aggregate_optional(assignment.optional, &assignment.destination)?;
        self.output.push(Instruction::Jump(finished.clone()));
        self.output.push(Instruction::Label(source_present));
        self.load_state(&assignment.destination)?;
        self.output.push(Instruction::Test(Register::Rax));
        self.output
            .push(Instruction::JumpIfNotZero(destination_present.clone()));
        self.copy_initialize_aggregate_payload(
            assignment.optional,
            assignment
                .destination
                .clone()
                .project_aggregate_optional_payload(assignment.optional),
            source
                .clone()
                .project_aggregate_optional_payload(assignment.optional),
        )?;
        self.store_state(&assignment.destination, true)?;
        self.output.push(Instruction::Jump(finished.clone()));
        self.output.push(Instruction::Label(destination_present));
        self.assign_aggregate_payload(
            assignment.optional,
            assignment
                .destination
                .clone()
                .project_aggregate_optional_payload(assignment.optional),
            source
                .clone()
                .project_aggregate_optional_payload(assignment.optional),
        )?;
        self.output.push(Instruction::Label(finished));
        Ok(())
    }

    fn copy_initialize_aggregate_optional(
        &mut self,
        optional: crate::identity::OptionalTypeId,
        destination: &MirPlace,
        source: &MirPlace,
    ) -> Result<(), BackendError> {
        let present = self.next_optional_label("nested_copy_present");
        let finished = self.next_optional_label("nested_copy_finished");
        self.load_state(source)?;
        self.output.push(Instruction::Test(Register::Rax));
        self.output
            .push(Instruction::JumpIfNotZero(present.clone()));
        self.store_state(destination, false)?;
        self.output.push(Instruction::Jump(finished.clone()));
        self.output.push(Instruction::Label(present));
        self.copy_initialize_aggregate_payload(
            optional,
            destination
                .clone()
                .project_aggregate_optional_payload(optional),
            source.clone().project_aggregate_optional_payload(optional),
        )?;
        self.store_state(destination, true)?;
        self.output.push(Instruction::Label(finished));
        Ok(())
    }

    fn cleanup_aggregate_optional(
        &mut self,
        optional: crate::identity::OptionalTypeId,
        destination: &MirPlace,
    ) -> Result<(), BackendError> {
        let finished = self.next_optional_label("nested_cleanup_finished");
        self.load_state(destination)?;
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(finished.clone()));
        self.cleanup_aggregate_payload(
            optional,
            destination
                .clone()
                .project_aggregate_optional_payload(optional),
        )?;
        self.store_state(destination, false)?;
        self.output.push(Instruction::Label(finished));
        Ok(())
    }

    fn copy_initialize_aggregate_payload(
        &mut self,
        optional: crate::identity::OptionalTypeId,
        destination: MirPlace,
        source: MirPlace,
    ) -> Result<(), BackendError> {
        match self
            .program
            .optional_type(optional)
            .expect("verified optional identity must exist")
            .storage
        {
            crate::mir::MirOptionalStorage::Nested(payload) => {
                self.copy_initialize_optional_value(payload, destination, source)
            }
            crate::mir::MirOptionalStorage::InlineArray(array) => {
                self.select_array_copy_construction(&destination, &source, array)
            }
            _ => unreachable!("aggregate optional operation requires aggregate metadata"),
        }
    }

    fn assign_aggregate_payload(
        &mut self,
        optional: crate::identity::OptionalTypeId,
        destination: MirPlace,
        source: MirPlace,
    ) -> Result<(), BackendError> {
        match self
            .program
            .optional_type(optional)
            .expect("verified optional identity must exist")
            .storage
        {
            crate::mir::MirOptionalStorage::Nested(payload) => {
                self.assign_optional_value(payload, destination, source)
            }
            crate::mir::MirOptionalStorage::InlineArray(array) => {
                self.select_array_copy_assignment(&destination, &source, array)
            }
            _ => unreachable!("aggregate optional operation requires aggregate metadata"),
        }
    }

    fn cleanup_aggregate_payload(
        &mut self,
        optional: crate::identity::OptionalTypeId,
        destination: MirPlace,
    ) -> Result<(), BackendError> {
        match self
            .program
            .optional_type(optional)
            .expect("verified optional identity must exist")
            .storage
        {
            crate::mir::MirOptionalStorage::Nested(payload) => {
                self.cleanup_optional_value(payload, destination)
            }
            crate::mir::MirOptionalStorage::InlineArray(array) => {
                self.select_array_field_cleanup(&destination, array)
            }
            _ => unreachable!("aggregate optional operation requires aggregate metadata"),
        }
    }

    fn copy_initialize_optional_value(
        &mut self,
        optional: crate::identity::OptionalTypeId,
        destination: MirPlace,
        source: MirPlace,
    ) -> Result<(), BackendError> {
        let metadata = self
            .program
            .optional_type(optional)
            .expect("verified optional identity must exist");
        match metadata.storage {
            crate::mir::MirOptionalStorage::Scalar => {
                self.select_optional_initialize(&MirOptionalInitialize {
                    destination,
                    source: MirOptionalSource::Copy(source),
                    span: self.active_instruction_span.expect("active instruction"),
                })
            }
            crate::mir::MirOptionalStorage::InlineClass(class) => {
                let crate::mir::MirOptionalCopyPlan::Class { operation, .. } = metadata
                    .lifecycle
                    .copy
                    .expect("verified nested optional must be copyable")
                else {
                    unreachable!("inline class optional requires class copy plan")
                };
                self.select_class_optional_initialize(&MirClassOptionalInitialize {
                    optional,
                    destination,
                    source: MirClassOptionalSource::Copy(source),
                    class,
                    copy_constructor: Some(operation),
                    span: self.active_instruction_span.expect("active instruction"),
                })
            }
            crate::mir::MirOptionalStorage::SharedOwner(target) => self
                .select_optional_shared_initialize(&MirOptionalSharedInitialize {
                    optional,
                    destination,
                    source: MirOptionalSharedSource::Copy(source),
                    target,
                    span: self.active_instruction_span.expect("active instruction"),
                }),
            crate::mir::MirOptionalStorage::Nested(_)
            | crate::mir::MirOptionalStorage::InlineArray(_) => {
                self.copy_initialize_aggregate_optional(optional, &destination, &source)
            }
        }
    }

    fn assign_optional_value(
        &mut self,
        optional: crate::identity::OptionalTypeId,
        destination: MirPlace,
        source: MirPlace,
    ) -> Result<(), BackendError> {
        let metadata = self
            .program
            .optional_type(optional)
            .expect("verified optional identity");
        match metadata.storage {
            crate::mir::MirOptionalStorage::Scalar => {
                self.select_optional_assign(&MirOptionalAssign {
                    destination,
                    source: MirOptionalSource::Copy(source),
                    authorization: None,
                    final_authorization: None,
                    span: self.active_instruction_span.expect("active instruction"),
                })
            }
            crate::mir::MirOptionalStorage::InlineClass(class) => {
                let crate::mir::MirOptionalAssignmentPlan::Class {
                    copy_constructor,
                    copy_assignment,
                    ..
                } = metadata
                    .lifecycle
                    .assignment
                    .expect("verified nested optional must be assignable")
                else {
                    unreachable!("inline class optional requires class assignment plan")
                };
                self.select_class_optional_assign(&MirClassOptionalAssign {
                    optional,
                    destination,
                    source: MirClassOptionalSource::Copy(source),
                    class,
                    copy_constructor: Some(copy_constructor),
                    copy_assignment: Some(copy_assignment),
                    authorization: None,
                    final_authorization: None,
                    span: self.active_instruction_span.expect("active instruction"),
                })
            }
            crate::mir::MirOptionalStorage::SharedOwner(target) => self
                .select_optional_shared_assign(&MirOptionalSharedAssign {
                    optional,
                    destination,
                    source: MirOptionalSharedSource::Copy(source),
                    target,
                    authorization: None,
                    final_authorization: None,
                    span: self.active_instruction_span.expect("active instruction"),
                }),
            crate::mir::MirOptionalStorage::Nested(_)
            | crate::mir::MirOptionalStorage::InlineArray(_) => self
                .select_aggregate_optional_assign(&MirAggregateOptionalAssign {
                    optional,
                    destination,
                    source: MirAggregateOptionalSource::Copy(source),
                    authorization: None,
                    final_authorization: None,
                    span: self.active_instruction_span.expect("active instruction"),
                }),
        }
    }

    fn cleanup_optional_value(
        &mut self,
        optional: crate::identity::OptionalTypeId,
        destination: MirPlace,
    ) -> Result<(), BackendError> {
        let metadata = self
            .program
            .optional_type(optional)
            .expect("verified optional identity");
        match metadata.storage {
            crate::mir::MirOptionalStorage::Scalar => Ok(()),
            crate::mir::MirOptionalStorage::InlineClass(class) => self
                .select_class_optional_cleanup(&MirClassOptionalCleanup {
                    optional,
                    destination,
                    class,
                    span: self.active_instruction_span.expect("active instruction"),
                }),
            crate::mir::MirOptionalStorage::SharedOwner(target) => self
                .select_optional_shared_cleanup(&MirOptionalSharedCleanup {
                    optional,
                    destination,
                    target,
                    span: self.active_instruction_span.expect("active instruction"),
                }),
            crate::mir::MirOptionalStorage::Nested(_)
            | crate::mir::MirOptionalStorage::InlineArray(_) => {
                self.cleanup_aggregate_optional(optional, &destination)
            }
        }
    }
}
