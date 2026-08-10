//! Primitive optional layout operations and checked-access control flow.

use crate::{
    backend::BackendError,
    mir::{
        MirClassOptionalAssign, MirClassOptionalCleanup, MirClassOptionalInitialize,
        MirClassOptionalPublish, MirClassOptionalSource, MirNestedOptionalAssign,
        MirNestedOptionalCleanup, MirNestedOptionalInitialize, MirNestedOptionalSource,
        MirOptionalAssign, MirOptionalInitialize, MirOptionalSharedAssign,
        MirOptionalSharedCleanup, MirOptionalSharedInitialize, MirOptionalSharedSource,
        MirOptionalSource, MirOptionalViewEnd, MirPlace, MirPresenceTestKind, MirPrimitiveType,
        MirTerminator, MirType, StorageId, ValueId,
    },
};

use super::{
    super::{
        machine::{ByteRegister, Instruction, Label, Operand, Register, XmmRegister},
        symbol,
    },
    block_label,
    ownership::{emit_release_loaded_handle, emit_retain_loaded_handle},
    value, InstructionSelector,
};

const OPTIONAL_SHARED_PRESERVED_HANDLE_SIZE: u32 = 16;

type OptionalPayload = (MirPrimitiveType, Operand);

impl InstructionSelector<'_, '_> {
    pub(super) fn select_optional_shared_initialize(
        &mut self,
        initialize: &MirOptionalSharedInitialize,
    ) -> Result<(), BackendError> {
        self.load_optional_shared_source(&initialize.source, true)?;
        self.store_optional_shared_place(&initialize.destination)
    }

    pub(super) fn select_optional_shared_assign(
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

    pub(super) fn select_optional_shared_cleanup(
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
                    super::terminator::emit_ownership_overflow(
                        super::call::TraceAttribution::SourceOperation,
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

    pub(super) fn release_optional_shared_place(
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
            super::call::TraceAttribution::SourceOperation,
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

    pub(super) fn select_class_optional_assign(
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
        let MirType::Optional(optional) = frame.ty() else {
            unreachable!("verified class optional operation has optional storage");
        };
        let offset = i32::try_from(self.data_layout.optional_type(optional)?.state_offset())
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

    pub(super) fn select_nested_optional_initialize(
        &mut self,
        initialize: &MirNestedOptionalInitialize,
    ) -> Result<(), BackendError> {
        match &initialize.source {
            MirNestedOptionalSource::Absent | MirNestedOptionalSource::Unpublished => {
                self.store_state(&initialize.destination, false)
            }
            MirNestedOptionalSource::Copy(source) => self.copy_initialize_nested_optional(
                initialize.optional,
                &initialize.destination,
                source,
            ),
        }
    }

    pub(super) fn select_nested_optional_publish(
        &mut self,
        publish: &crate::mir::MirNestedOptionalPublish,
    ) -> Result<(), BackendError> {
        self.store_state(&publish.destination, true)
    }

    pub(super) fn select_nested_optional_cleanup(
        &mut self,
        cleanup: &MirNestedOptionalCleanup,
    ) -> Result<(), BackendError> {
        self.cleanup_nested_optional(cleanup.optional, &cleanup.destination)
    }

    pub(super) fn select_nested_optional_assign(
        &mut self,
        assignment: &MirNestedOptionalAssign,
    ) -> Result<(), BackendError> {
        if matches!(&assignment.source, MirNestedOptionalSource::Copy(source) if source == &assignment.destination)
        {
            return Ok(());
        }
        let source = match &assignment.source {
            MirNestedOptionalSource::Absent | MirNestedOptionalSource::Unpublished => {
                return self.cleanup_nested_optional(assignment.optional, &assignment.destination);
            }
            MirNestedOptionalSource::Copy(source) => source,
        };
        let source_present = self.next_optional_label("nested_source_present");
        let destination_present = self.next_optional_label("nested_destination_present");
        let finished = self.next_optional_label("nested_assign_finished");
        self.load_state(source)?;
        self.output.push(Instruction::Test(Register::Rax));
        self.output
            .push(Instruction::JumpIfNotZero(source_present.clone()));
        self.cleanup_nested_optional(assignment.optional, &assignment.destination)?;
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
                .project_nested_optional_payload(assignment.optional),
            source
                .clone()
                .project_nested_optional_payload(assignment.optional),
        )?;
        self.store_state(&assignment.destination, true)?;
        self.output.push(Instruction::Jump(finished.clone()));
        self.output.push(Instruction::Label(destination_present));
        self.assign_aggregate_payload(
            assignment.optional,
            assignment
                .destination
                .clone()
                .project_nested_optional_payload(assignment.optional),
            source
                .clone()
                .project_nested_optional_payload(assignment.optional),
        )?;
        self.output.push(Instruction::Label(finished));
        Ok(())
    }

    fn copy_initialize_nested_optional(
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
                .project_nested_optional_payload(optional),
            source.clone().project_nested_optional_payload(optional),
        )?;
        self.store_state(destination, true)?;
        self.output.push(Instruction::Label(finished));
        Ok(())
    }

    fn cleanup_nested_optional(
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
                .project_nested_optional_payload(optional),
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
            crate::mir::MirOptionalStorage::Nested(_) => {
                self.copy_initialize_nested_optional(optional, &destination, &source)
            }
            crate::mir::MirOptionalStorage::InlineArray(array) => {
                self.select_array_copy_construction(&destination, &source, array)
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
                    span: self.active_instruction_span.expect("active instruction"),
                })
            }
            crate::mir::MirOptionalStorage::SharedOwner(target) => self
                .select_optional_shared_assign(&MirOptionalSharedAssign {
                    optional,
                    destination,
                    source: MirOptionalSharedSource::Copy(source),
                    target,
                    span: self.active_instruction_span.expect("active instruction"),
                }),
            crate::mir::MirOptionalStorage::Nested(_) => {
                self.select_nested_optional_assign(&MirNestedOptionalAssign {
                    optional,
                    destination,
                    source: MirNestedOptionalSource::Copy(source),
                    span: self.active_instruction_span.expect("active instruction"),
                })
            }
            crate::mir::MirOptionalStorage::InlineArray(array) => {
                self.select_array_copy_assignment(&destination, &source, array)
            }
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
            crate::mir::MirOptionalStorage::Nested(_) => {
                self.cleanup_nested_optional(optional, &destination)
            }
            crate::mir::MirOptionalStorage::InlineArray(array) => {
                self.select_array_field_cleanup(&destination, array)
            }
        }
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

    pub(super) fn select_optional_terminator(
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
                super::terminator::emit_ownership_overflow(
                    super::call::TraceAttribution::SourceOperation,
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

    pub(super) fn select_optional_view_end(
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

    fn trap_if_class_optional_pinned(&mut self, place: &MirPlace) -> Result<(), BackendError> {
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
        let (optional, operand) = self.optional_base(place)?;
        let layout = self.data_layout.optional_type(optional)?;
        let offset = i32::try_from(layout.state_offset())
            .expect("optional state offset fits the target displacement");
        offset_operand(operand, offset, self.function.callable())
    }

    fn optional_payload(&mut self, place: &MirPlace) -> Result<OptionalPayload, BackendError> {
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

    fn copy_payload_to_storage(
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

    fn next_optional_label(&mut self, suffix: &str) -> Label {
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

fn offset_operand(
    operand: Operand,
    offset: i32,
    callable: crate::identity::CallableId,
) -> Result<Operand, BackendError> {
    let displacement = match operand {
        Operand::Memory { base, displacement } => {
            return displacement
                .checked_add(offset)
                .map(|displacement| value::memory(base, displacement))
                .ok_or_else(|| {
                    BackendError::new(
                        crate::backend::Target::X86_64SysV,
                        Some(callable),
                        "optional payload displacement exceeds x86-64 limits",
                    )
                });
        }
        Operand::IndexedMemory {
            base,
            index,
            scale,
            displacement,
        } => displacement
            .checked_add(offset)
            .map(|displacement| value::indexed_memory(base, index, scale, displacement)),
        Operand::Register(_) => None,
    };
    displacement.ok_or_else(|| {
        BackendError::new(
            crate::backend::Target::X86_64SysV,
            Some(callable),
            "optional payload displacement exceeds x86-64 limits",
        )
    })
}

fn optional_label(program: &crate::mir::MirProgram, result: ValueId, suffix: &str) -> Label {
    Label::new(format!(
        ".Lska.{}.optional_test_{}_{}",
        symbol::local_label_stem(program, result.callable()),
        result.index(),
        suffix
    ))
}
