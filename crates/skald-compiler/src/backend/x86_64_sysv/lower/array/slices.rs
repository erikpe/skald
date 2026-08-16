//! Checked slice normalization, copied-slice construction, and slice writes.

use crate::{
    backend::BackendError,
    mir::{
        MirArrayAssignElement, MirArrayBoundary, MirClassOptionalAssign, MirClassOptionalSource,
        MirOptionalSharedAssign, MirOptionalSharedSource, MirOptionalSource, MirPlace,
        MirPlaceBase, MirPlaceProjection, MirType, StorageId,
    },
    source::Span,
};

use super::{
    super::{
        super::{
            layout::{ARRAY_LENGTH_OFFSET, ARRAY_OWNER_COUNT_OFFSET, SHARED_ARRAY_LENGTH_OFFSET},
            machine::{Instruction, Register},
            symbol,
        },
        value, InstructionSelector,
    },
    array_element_place,
};

const SLICE_COPY_HOMES_SIZE: u32 = 32;
const SLICE_ASSIGN_HOME_SIZE: u32 = 16;
const RUNTIME_ALLOC: &str = "ska_rt_alloc";

impl InstructionSelector<'_, '_> {
    pub(super) fn select_array_range_offset(
        &mut self,
        destination: StorageId,
        owner: &MirPlace,
        offset: crate::mir::ValueId,
    ) -> Result<(), BackendError> {
        self.load_array_length(owner)?;
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdx.into(),
        });
        value::load_rax(value::frame_value(self.frame, offset), self.output);

        let valid = self.next_array_label("range_offset_valid");
        let complete = self.next_array_label("range_offset_complete");
        self.output.push(Instruction::Compare {
            source: Register::Rdx,
            destination: Register::Rax,
        });
        self.output.push(Instruction::JumpIfBelow(valid.clone()));
        self.output.push(Instruction::JumpIfEqual(valid.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::MAX,
            destination: Register::Rax,
        });
        self.output.push(Instruction::Jump(complete.clone()));
        self.output.push(Instruction::Label(valid));
        self.output.push(Instruction::Label(complete));
        value::store_rax(value::frame_storage(self.frame, destination), self.output);
        Ok(())
    }

    pub(super) fn load_array_length(&mut self, owner: &MirPlace) -> Result<(), BackendError> {
        let shared = self.load_array_owner(owner)?;
        let empty = self.next_array_label("length_empty");
        let complete = self.next_array_label("length_complete");
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(empty.clone()));
        value::load_rax(
            value::memory(
                Register::Rax,
                if shared {
                    SHARED_ARRAY_LENGTH_OFFSET
                } else {
                    ARRAY_LENGTH_OFFSET
                },
            ),
            self.output,
        );
        self.output.push(Instruction::Jump(complete.clone()));
        self.output.push(Instruction::Label(empty));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rax,
        });
        self.output.push(Instruction::Label(complete));
        Ok(())
    }

    pub(super) fn select_array_slice_boundary(
        &mut self,
        destination: StorageId,
        owner: &MirPlace,
        boundary: MirArrayBoundary,
    ) -> Result<(), BackendError> {
        match boundary {
            MirArrayBoundary::Start => self.output.push(Instruction::MoveImmediate64 {
                bits: 0,
                destination: Register::Rax,
            }),
            MirArrayBoundary::End => self.load_array_length(owner)?,
        }
        value::store_rax(value::frame_storage(self.frame, destination), self.output);
        Ok(())
    }

    pub(super) fn select_array_slice_normalize(
        &mut self,
        destination: StorageId,
        owner: &MirPlace,
        index: crate::mir::ValueId,
    ) -> Result<(), BackendError> {
        self.load_array_length(owner)?;
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdx.into(),
        });
        value::load_rax(value::frame_value(self.frame, index), self.output);
        let compare = self.next_array_label("slice_normalize_compare");
        let valid = self.next_array_label("slice_normalize_valid");
        let complete = self.next_array_label("slice_normalize_complete");
        self.output.push(Instruction::Test(Register::Rax));
        self.output
            .push(Instruction::JumpIfNotSign(compare.clone()));
        self.output.push(Instruction::Add {
            source: Register::Rdx,
            destination: Register::Rax,
        });
        self.output.push(Instruction::Label(compare));
        self.output.push(Instruction::Compare {
            source: Register::Rdx,
            destination: Register::Rax,
        });
        self.output.push(Instruction::JumpIfBelow(valid.clone()));
        self.output.push(Instruction::JumpIfEqual(valid.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::MAX,
            destination: Register::Rax,
        });
        self.output.push(Instruction::Jump(complete.clone()));
        self.output.push(Instruction::Label(valid));
        self.output.push(Instruction::Label(complete));
        value::store_rax(value::frame_storage(self.frame, destination), self.output);
        Ok(())
    }

    pub(super) fn select_array_slice_bounds_check(&mut self, start: StorageId, end: StorageId) {
        let valid = self.next_array_label("slice_bounds_valid");
        let complete = self.next_array_label("slice_bounds_complete");
        value::load_rax(value::frame_storage(self.frame, start), self.output);
        self.output.push(Instruction::Move {
            source: value::frame_storage(self.frame, end),
            destination: Register::Rdx.into(),
        });
        self.output.push(Instruction::Compare {
            source: Register::Rdx,
            destination: Register::Rax,
        });
        self.output.push(Instruction::JumpIfBelow(valid.clone()));
        self.output.push(Instruction::JumpIfEqual(valid.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::R11,
        });
        self.output.push(Instruction::Jump(complete.clone()));
        self.output.push(Instruction::Label(valid));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        });
        self.output.push(Instruction::Label(complete));
    }

    pub(super) fn select_array_slice_length_check(
        &mut self,
        destination_start: StorageId,
        destination_end: StorageId,
        source: &MirPlace,
    ) -> Result<(), BackendError> {
        self.load_array_length(source)?;
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdx.into(),
        });
        value::load_rax(
            value::frame_storage(self.frame, destination_end),
            self.output,
        );
        self.output.push(Instruction::Move {
            source: value::frame_storage(self.frame, destination_start),
            destination: Register::R11.into(),
        });
        self.output.push(Instruction::Subtract {
            source: Register::R11,
            destination: Register::Rax,
        });
        self.select_equality_status(Register::Rdx, Register::Rax, "slice_length");
        Ok(())
    }

    pub(super) fn select_array_slice_copy(
        &mut self,
        destination: StorageId,
        source: &MirPlace,
        start: StorageId,
        end: StorageId,
        array: crate::identity::ArrayTypeId,
    ) -> Result<(), BackendError> {
        let layout = self
            .data_layout
            .array(array)
            .ok_or_else(|| self.array_error(format!("array {array} has no slice layout")))?;
        let empty = self.next_array_label("slice_copy_empty");
        let header = self.next_array_label("slice_copy_header");
        let body = self.next_array_label("slice_copy_body");
        let complete = self.next_array_label("slice_copy_complete");

        value::load_rax(value::frame_storage(self.frame, end), self.output);
        self.output.push(Instruction::Move {
            source: value::frame_storage(self.frame, start),
            destination: Register::R11.into(),
        });
        self.output.push(Instruction::Subtract {
            source: Register::R11,
            destination: Register::Rax,
        });
        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(empty.clone()));
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdx.into(),
        });
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::try_from(layout.stride()).expect("array stride fits u64"),
            destination: Register::R11,
        });
        self.output.push(Instruction::Multiply {
            source: Register::R11,
            destination: Register::Rax,
        });
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::try_from(layout.element_offset()).expect("array offset fits u64"),
            destination: Register::R11,
        });
        self.output.push(Instruction::Add {
            source: Register::R11,
            destination: Register::Rax,
        });
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdi.into(),
        });
        self.emit_source_operation_call(RUNTIME_ALLOC.to_owned())?;
        value::store_rax(value::frame_storage(self.frame, destination), self.output);
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::R11.into(),
        });
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::Rax,
        });
        value::store_rax(
            value::memory(Register::R11, ARRAY_OWNER_COUNT_OFFSET),
            self.output,
        );
        value::load_rax(value::frame_storage(self.frame, end), self.output);
        self.output.push(Instruction::Move {
            source: value::frame_storage(self.frame, start),
            destination: Register::Rdx.into(),
        });
        self.output.push(Instruction::Subtract {
            source: Register::Rdx,
            destination: Register::Rax,
        });
        value::store_rax(
            value::memory(Register::R11, ARRAY_LENGTH_OFFSET),
            self.output,
        );

        self.output
            .push(Instruction::ReserveStack(SLICE_COPY_HOMES_SIZE));
        let shared_source = self.load_array_owner(source)?;
        if shared_source {
            self.output.push(Instruction::MoveImmediate64 {
                bits: u64::try_from(layout.shared_element_offset() - layout.element_offset())
                    .expect("shared slice source adjustment fits u64"),
                destination: Register::R11,
            });
            self.output.push(Instruction::Add {
                source: Register::R11,
                destination: Register::Rax,
            });
        }
        value::store_rax(value::memory(Register::Rsp, 0), self.output);
        value::load_rax(value::frame_storage(self.frame, destination), self.output);
        value::store_rax(value::memory(Register::Rsp, 8), self.output);
        value::load_rax(value::frame_storage(self.frame, start), self.output);
        value::store_rax(value::memory(Register::Rsp, 16), self.output);
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rax,
        });
        value::store_rax(value::memory(Register::Rsp, 24), self.output);

        self.output.push(Instruction::Label(header.clone()));
        value::load_rax(value::memory(Register::Rsp, 24), self.output);
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rsp, 8),
            destination: Register::R11.into(),
        });
        self.output.push(Instruction::Move {
            source: value::memory(Register::R11, ARRAY_LENGTH_OFFSET),
            destination: Register::R10.into(),
        });
        self.output.push(Instruction::Compare {
            source: Register::R10,
            destination: Register::Rax,
        });
        self.output.push(Instruction::JumpIfBelow(body.clone()));
        self.output
            .push(Instruction::ReleaseStack(SLICE_COPY_HOMES_SIZE));
        self.output.push(Instruction::Jump(complete.clone()));

        self.output.push(Instruction::Label(body));
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rsp, 8),
            destination: Register::Rdi.into(),
        });
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rsp, 0),
            destination: Register::Rsi.into(),
        });
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rsp, 24),
            destination: Register::Rdx.into(),
        });
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rsp, 16),
            destination: Register::Rcx.into(),
        });
        self.emit_source_operation_call(symbol::array_copy_element(array))?;
        self.increment_stack_home(16);
        self.increment_stack_home(24);
        self.output.push(Instruction::Jump(header));

        self.output.push(Instruction::Label(empty));
        self.clear_storage(destination);
        self.output.push(Instruction::Label(complete));
        Ok(())
    }

    pub(super) fn select_array_slice_assign(
        &mut self,
        destination: &MirPlace,
        source: &MirPlace,
        destination_index: StorageId,
        source_index: StorageId,
        operation: MirArrayAssignElement,
        span: Span,
    ) -> Result<(), BackendError> {
        let array = self.array_for_storage_type(source)?;
        let header = self.next_array_label("slice_assign_header");
        let body = self.next_array_label("slice_assign_body");
        let complete = self.next_array_label("slice_assign_complete");
        self.output
            .push(Instruction::ReserveStack(SLICE_ASSIGN_HOME_SIZE));
        self.load_array_length(source)?;
        value::store_rax(value::memory(Register::Rsp, 0), self.output);
        self.output.push(Instruction::Label(header.clone()));
        value::load_rax(value::frame_storage(self.frame, source_index), self.output);
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rsp, 0),
            destination: Register::R11.into(),
        });
        self.output.push(Instruction::Compare {
            source: Register::R11,
            destination: Register::Rax,
        });
        self.output.push(Instruction::JumpIfBelow(body.clone()));
        self.output
            .push(Instruction::ReleaseStack(SLICE_ASSIGN_HOME_SIZE));
        self.output.push(Instruction::Jump(complete.clone()));

        self.output.push(Instruction::Label(body));
        let destination_element =
            array_element_place(destination.clone(), array, destination_index);
        let source_element = array_element_place(source.clone(), array, source_index);
        self.select_array_element_assignment(destination_element, source_element, operation, span)?;
        self.advance_array_index(destination_index);
        self.advance_array_index(source_index);
        self.output.push(Instruction::Jump(header));
        self.output.push(Instruction::Label(complete));
        Ok(())
    }

    fn select_array_element_assignment(
        &mut self,
        destination: MirPlace,
        source: MirPlace,
        operation: MirArrayAssignElement,
        span: Span,
    ) -> Result<(), BackendError> {
        match operation {
            MirArrayAssignElement::Primitive => self.copy_array_primitive(&destination, &source),
            MirArrayAssignElement::OptionalPrimitive => {
                self.select_optional_write(&destination, &MirOptionalSource::Copy(source))
            }
            MirArrayAssignElement::Class { operation, .. } => {
                self.select_assignment_operation(operation, destination, source)
            }
            MirArrayAssignElement::OptionalClass {
                class,
                copy_constructor,
                copy_assignment,
            } => self.select_class_optional_assign(&MirClassOptionalAssign {
                optional: self
                    .program
                    .optional_for_payload(MirType::Class(class))
                    .expect("verified optional-class array metadata exists"),
                destination,
                source: MirClassOptionalSource::Copy(source),
                class,
                copy_constructor: Some(copy_constructor),
                copy_assignment: Some(copy_assignment),
                authorization: None,
                final_authorization: None,
                span,
            }),
            MirArrayAssignElement::Array(inner) => {
                self.select_array_copy_assignment(&destination, &source, inner)
            }
            MirArrayAssignElement::Shared(_) => {
                self.select_shared_field_assignment(&destination, &source)
            }
            MirArrayAssignElement::OptionalShared(target) => {
                self.select_optional_shared_assign(&MirOptionalSharedAssign {
                    optional: self
                        .program
                        .optional_for_payload(MirType::Shared(target))
                        .expect("verified optional-owner array metadata exists"),
                    destination,
                    source: MirOptionalSharedSource::Copy(source),
                    target,
                    authorization: None,
                    final_authorization: None,
                    span,
                })
            }
            MirArrayAssignElement::Optional(optional) => {
                self.select_aggregate_optional_assign(&crate::mir::MirAggregateOptionalAssign {
                    optional,
                    destination,
                    source: crate::mir::MirAggregateOptionalSource::Copy(source),
                    authorization: None,
                    final_authorization: None,
                    span,
                })
            }
        }
    }

    fn array_for_storage_type(
        &self,
        place: &MirPlace,
    ) -> Result<crate::identity::ArrayTypeId, BackendError> {
        let mut ty = self
            .function
            .storage(place.base.expect_local_storage())
            .expect("verified slice owner has storage")
            .ty;
        if matches!(place.base, MirPlaceBase::SharedPointee(_)) {
            let MirType::Shared(target) = ty else {
                return Err(self.array_error("shared slice owner has no shared target"));
            };
            ty = self
                .program
                .shared_target_type(target)
                .ok_or_else(|| self.array_error("shared slice owner has no payload type"))?;
        }
        for projection in &place.projections {
            ty = match *projection {
                MirPlaceProjection::Base(class) | MirPlaceProjection::OptionalPayload(class) => {
                    MirType::Class(class)
                }
                MirPlaceProjection::AggregateOptionalPayload(optional)
                | MirPlaceProjection::CheckedOptionalPayload(optional) => {
                    self.program
                        .optional_type(optional)
                        .expect("verified optional")
                        .payload
                }
                MirPlaceProjection::Field(field) => {
                    self.program.field(field).expect("verified field exists").ty
                }
                MirPlaceProjection::ArrayElement { array, .. } => {
                    self.program
                        .array_type(array)
                        .expect("verified array exists")
                        .element
                }
            };
        }
        match ty {
            MirType::Array(array) => Ok(array),
            _ => Err(self.array_error("slice owner is not an executable array")),
        }
    }

    fn select_equality_status(&mut self, left: Register, right: Register, purpose: &str) {
        let equal = self.next_array_label(&format!("{purpose}_equal"));
        let complete = self.next_array_label(&format!("{purpose}_complete"));
        self.output.push(Instruction::Compare {
            source: left,
            destination: right,
        });
        self.output.push(Instruction::JumpIfEqual(equal.clone()));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::R11,
        });
        self.output.push(Instruction::Jump(complete.clone()));
        self.output.push(Instruction::Label(equal));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        });
        self.output.push(Instruction::Label(complete));
    }

    fn increment_stack_home(&mut self, displacement: i32) {
        value::load_rax(value::memory(Register::Rsp, displacement), self.output);
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        });
        self.output.push(Instruction::Add {
            source: Register::R11,
            destination: Register::Rax,
        });
        value::store_rax(value::memory(Register::Rsp, displacement), self.output);
    }
}
