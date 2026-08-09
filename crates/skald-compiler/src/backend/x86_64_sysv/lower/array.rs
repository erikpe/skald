//! Executable inline and shared-array instruction and control-flow selection.

use crate::{
    backend::{BackendError, Target},
    mir::{
        MirArrayAssignElement, MirArrayCopyElement, MirArrayDefaultElement, MirArrayDestroyElement,
        MirArrayFailure, MirArrayInstruction, MirArrayOwnership, MirArrayPositionKind,
        MirClassOptionalCleanup, MirClassOptionalInitialize, MirClassOptionalSource, MirInitialize,
        MirOptionalSharedCleanup, MirOptionalSharedInitialize, MirOptionalSharedSource,
        MirOptionalSource, MirPlace, MirPlaceProjection, MirStore, MirTerminator, MirType,
    },
};

use super::{
    super::{
        frame::FramePlace,
        layout::{
            ARRAY_LENGTH_OFFSET, ARRAY_OWNER_COUNT_OFFSET, SHARED_ARRAY_LENGTH_OFFSET,
            SHARED_DYNAMIC_METADATA_OFFSET,
        },
        machine::{ByteRegister, Instruction, Label, Operand, Register},
        symbol,
    },
    block_label, value, InstructionSelector,
};

mod anchors;
mod helpers;
mod lifecycle;
mod shared_elements;
mod slices;

pub(super) fn lower_helpers(
    program: &crate::mir::MirProgram,
    data_layout: &super::super::layout::DataLayout,
) -> Result<Vec<super::super::machine::AssemblyFunction>, BackendError> {
    let mut functions = helpers::lower_all(program, data_layout)?;
    functions.extend(lifecycle::lower_class_copy_helpers(program, data_layout)?);
    Ok(functions)
}

const RUNTIME_ALLOC: &str = "ska_rt_alloc";

#[derive(Clone, Copy)]
enum ArrayAllocationLength {
    Value(crate::mir::ValueId),
    Constant(u64),
}

impl InstructionSelector<'_, '_> {
    pub(super) fn select_array_copy_construction(
        &mut self,
        destination: &MirPlace,
        source: &MirPlace,
        array: crate::identity::ArrayTypeId,
    ) -> Result<(), BackendError> {
        self.clone_array_preserving_destination(destination, source, array)?;
        value::store_rax(value::memory(Register::Rdx, 0), self.output);
        Ok(())
    }

    pub(super) fn select_array_copy_assignment(
        &mut self,
        destination: &MirPlace,
        source: &MirPlace,
        array: crate::identity::ArrayTypeId,
    ) -> Result<(), BackendError> {
        self.clone_array_preserving_destination(destination, source, array)?;
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rdx, 0),
            destination: Register::Rdi.into(),
        });
        value::store_rax(value::memory(Register::Rdx, 0), self.output);
        self.emit_source_operation_call(symbol::array_release(array))?;
        Ok(())
    }

    pub(super) fn select_array_field_cleanup(
        &mut self,
        owner: &MirPlace,
        array: crate::identity::ArrayTypeId,
    ) -> Result<(), BackendError> {
        let (_, owner) = self.frame_place(owner)?;
        value::load_rax(owner, self.output);
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdi.into(),
        });
        self.emit_source_operation_call(symbol::array_release(array))?;
        Ok(())
    }

    pub(super) fn select_array_instruction(
        &mut self,
        instruction: &MirArrayInstruction,
    ) -> Result<(), BackendError> {
        match instruction {
            MirArrayInstruction::Allocate {
                backing,
                array,
                length,
                ownership,
                ..
            } => self.select_array_allocate(*backing, *array, *length, *ownership),
            MirArrayInstruction::AllocateElements {
                backing,
                prefix,
                array,
                length,
                ownership,
                ..
            } => {
                self.clear_storage(*prefix);
                self.select_array_allocate_length(
                    *backing,
                    *array,
                    ArrayAllocationLength::Constant(*length),
                    *ownership,
                )
            }
            MirArrayInstruction::InitializeElement {
                backing,
                prefix,
                value,
                span,
                ..
            } => {
                let array = self.array_for_storage(*backing)?;
                let destination = array_element_place(MirPlace::base(*backing), array, *prefix);
                self.select_store(&MirStore {
                    destination,
                    value: *value,
                    span: *span,
                })?;
                self.advance_array_index(*prefix);
                Ok(())
            }
            MirArrayInstruction::CompleteElement { prefix, .. } => {
                self.advance_array_index(*prefix);
                Ok(())
            }
            MirArrayInstruction::InitializeNext {
                backing,
                index,
                operation,
                span,
            } => {
                let array = self.array_for_storage(*backing)?;
                let destination = array_element_place(MirPlace::base(*backing), array, *index);
                match *operation {
                    MirArrayDefaultElement::Primitive => {
                        self.clear_place(&destination)?;
                    }
                    MirArrayDefaultElement::OptionalAbsent => {
                        match self
                            .program
                            .array_type(array)
                            .expect("verified array declaration exists")
                            .element
                        {
                            MirType::OptionalPrimitive(_) => {
                                self.select_optional_write(
                                    &destination,
                                    &MirOptionalSource::Absent,
                                )?;
                            }
                            MirType::OptionalClass(class) => {
                                self.select_class_optional_initialize(
                                    &MirClassOptionalInitialize {
                                        destination,
                                        source: MirClassOptionalSource::Absent,
                                        class,
                                        copy_constructor: None,
                                        span: *span,
                                    },
                                )?;
                            }
                            MirType::OptionalShared(target) => {
                                self.select_optional_shared_initialize(
                                    &MirOptionalSharedInitialize {
                                        destination,
                                        source: MirOptionalSharedSource::Absent,
                                        target,
                                        span: *span,
                                    },
                                )?;
                            }
                            _ => {
                                unreachable!("verified absent default requires an optional element")
                            }
                        }
                    }
                    MirArrayDefaultElement::Class {
                        class: _,
                        initializer,
                    } => {
                        self.select_initialize(&MirInitialize {
                            destination,
                            target: initializer,
                            arguments: Vec::new(),
                            span: *span,
                        })?;
                    }
                    MirArrayDefaultElement::ArrayEmpty(_) => {
                        self.clear_place(&destination)?;
                    }
                    MirArrayDefaultElement::SharedClass { class, initializer } => {
                        self.select_default_shared_class_element(&destination, class, initializer)?;
                    }
                    MirArrayDefaultElement::SharedArrayEmpty(inner) => {
                        self.select_default_shared_array_element(&destination, inner)?;
                    }
                }
                self.advance_array_index(*index);
                Ok(())
            }
            MirArrayInstruction::CopyNext {
                backing,
                source,
                index,
                operation,
                span,
            } => {
                let array = self.array_for_storage(*backing)?;
                let destination = array_element_place(MirPlace::base(*backing), array, *index);
                let source = array_element_place(source.clone(), array, *index);
                match *operation {
                    MirArrayCopyElement::Primitive => {
                        self.copy_array_primitive(&destination, &source)?;
                    }
                    MirArrayCopyElement::OptionalPrimitive => {
                        self.select_optional_write(&destination, &MirOptionalSource::Copy(source))?;
                    }
                    MirArrayCopyElement::Class {
                        class: _,
                        operation,
                    } => {
                        self.select_construction_operation(operation, destination, source)?;
                    }
                    MirArrayCopyElement::OptionalClass { class, operation } => {
                        self.select_class_optional_initialize(&MirClassOptionalInitialize {
                            destination,
                            source: MirClassOptionalSource::Copy(source),
                            class,
                            copy_constructor: Some(operation),
                            span: *span,
                        })?;
                    }
                    MirArrayCopyElement::Array(inner) => {
                        self.select_array_copy_construction(&destination, &source, inner)?;
                    }
                    MirArrayCopyElement::Shared(_) => {
                        self.select_shared_field_construction(&destination, &source)?;
                    }
                    MirArrayCopyElement::OptionalShared(target) => {
                        self.select_optional_shared_initialize(&MirOptionalSharedInitialize {
                            destination,
                            source: MirOptionalSharedSource::Copy(source),
                            target,
                            span: *span,
                        })?;
                    }
                }
                self.advance_array_index(*index);
                Ok(())
            }
            MirArrayInstruction::Publish {
                backing,
                destination,
                ..
            } => {
                value::load_rax(value::frame_storage(self.frame, *backing), self.output);
                value::store_rax(value::frame_storage(self.frame, *destination), self.output);
                self.clear_storage(*backing);
                Ok(())
            }
            MirArrayInstruction::PublishShared {
                backing,
                destination,
                array,
                ..
            } => {
                value::load_rax(value::frame_storage(self.frame, *backing), self.output);
                self.output.push(Instruction::Move {
                    source: Register::Rax.into(),
                    destination: Register::R11.into(),
                });
                self.output.push(Instruction::LoadSymbolAddress {
                    symbol: symbol::shared_array_metadata(*array),
                    destination: Register::Rax,
                });
                value::store_rax(
                    value::memory(Register::R11, SHARED_DYNAMIC_METADATA_OFFSET),
                    self.output,
                );
                self.output.push(Instruction::MoveImmediate64 {
                    bits: 1,
                    destination: Register::Rax,
                });
                value::store_rax(
                    value::memory(Register::R11, ARRAY_OWNER_COUNT_OFFSET),
                    self.output,
                );
                self.output.push(Instruction::Move {
                    source: Register::R11.into(),
                    destination: value::frame_storage(self.frame, *destination),
                });
                self.clear_storage(*backing);
                Ok(())
            }
            MirArrayInstruction::Adopt {
                destination,
                source,
                ..
            } => {
                let (_, destination) = self.frame_place(destination)?;
                value::load_rax(value::frame_storage(self.frame, *source), self.output);
                value::store_rax(destination, self.output);
                self.clear_storage(*source);
                Ok(())
            }
            MirArrayInstruction::Replace {
                destination,
                source,
                array,
                ..
            } => {
                let (_, destination) = self.frame_place(destination)?;
                value::load_rax(destination, self.output);
                self.output.push(Instruction::Move {
                    source: Register::Rax.into(),
                    destination: Register::Rdi.into(),
                });
                value::load_rax(value::frame_storage(self.frame, *source), self.output);
                value::store_rax(destination, self.output);
                self.clear_storage(*source);
                self.emit_source_operation_call(symbol::array_release(*array))?;
                Ok(())
            }
            MirArrayInstruction::ElementAssign {
                destination,
                source,
                operation: MirArrayAssignElement::Shared(_),
                ..
            } => self.select_shared_field_assignment(destination, source),
            MirArrayInstruction::Release { owner, array, .. } => {
                let (_, source) = self.frame_place(owner)?;
                value::load_rax(source, self.output);
                self.output.push(Instruction::Move {
                    source: Register::Rax.into(),
                    destination: Register::Rdi.into(),
                });
                self.emit_source_operation_call(symbol::array_release(*array))?;
                self.clear_place(owner)
            }
            MirArrayInstruction::DestroyNext {
                owner,
                index,
                operation,
                span,
            } => {
                let array = array_for_place(self.program, self.function, owner)?;
                let element = array_element_place(owner.clone(), array, *index);
                match *operation {
                    MirArrayDestroyElement::Trivial => {}
                    MirArrayDestroyElement::Class(class) => {
                        self.select_destruction_plan(class, element)?;
                    }
                    MirArrayDestroyElement::OptionalClass(class) => {
                        self.select_class_optional_cleanup(&MirClassOptionalCleanup {
                            destination: element,
                            class,
                            span: *span,
                        })?;
                    }
                    MirArrayDestroyElement::Array(inner) => {
                        self.select_array_field_cleanup(&element, inner)?;
                    }
                    MirArrayDestroyElement::Shared(_) => {
                        self.release_shared_place(&element, "array_element_release")?;
                    }
                    MirArrayDestroyElement::OptionalShared(target) => {
                        self.select_optional_shared_cleanup(&MirOptionalSharedCleanup {
                            destination: element,
                            target,
                            span: *span,
                        })?;
                    }
                }
                Ok(())
            }
            MirArrayInstruction::AnchorBegin {
                anchor,
                owner,
                kind,
                ..
            } => self.select_array_anchor_begin(*anchor, owner, *kind),
            MirArrayInstruction::AnchorEnd { anchor, .. } => self.select_array_anchor_end(*anchor),
            MirArrayInstruction::AliasBind { alias, source, .. } => {
                self.select_array_alias_bind(*alias, source)
            }
            MirArrayInstruction::Normalize {
                destination,
                owner,
                index,
                kind: MirArrayPositionKind::Element,
                ..
            } => self.select_array_element_normalize(*destination, owner, *index),
            MirArrayInstruction::Normalize {
                destination,
                owner,
                index,
                kind: MirArrayPositionKind::SliceBound,
                ..
            } => self.select_array_slice_normalize(*destination, owner, *index),
            MirArrayInstruction::Normalize {
                kind: MirArrayPositionKind::RangeOffset,
                ..
            } => Err(BackendError::new(
                crate::backend::Target::X86_64SysV,
                Some(self.function.callable()),
                "range offsets must use the unsigned offset instruction",
            )),
            MirArrayInstruction::Offset {
                destination,
                owner,
                offset,
                ..
            } => self.select_array_range_offset(*destination, owner, *offset),
            MirArrayInstruction::Boundary {
                destination,
                owner,
                boundary,
                ..
            } => self.select_array_slice_boundary(*destination, owner, *boundary),
            MirArrayInstruction::SliceBoundsCheck { start, end, .. } => {
                self.select_array_slice_bounds_check(*start, *end);
                Ok(())
            }
            MirArrayInstruction::SliceLengthCheck {
                destination_start,
                destination_end,
                source,
                ..
            } => self.select_array_slice_length_check(*destination_start, *destination_end, source),
            MirArrayInstruction::SliceCopy {
                destination,
                source,
                start,
                end,
                array,
                ..
            } => self.select_array_slice_copy(*destination, source, *start, *end, *array),
            MirArrayInstruction::SliceAssignNext {
                destination,
                source,
                destination_index,
                source_index,
                operation,
                span,
            } => self.select_array_slice_assign(
                destination,
                source,
                *destination_index,
                *source_index,
                *operation,
                *span,
            ),
            _ => {
                Err(self.array_error("array instruction escaped the executable legality boundary"))
            }
        }
    }

    pub(super) fn select_array_length(
        &mut self,
        source: &MirPlace,
        result: crate::mir::ValueId,
    ) -> Result<(), BackendError> {
        self.load_array_length(source)?;
        value::store_rax(value::frame_value(self.frame, result), self.output);
        Ok(())
    }

    pub(super) fn select_array_terminator(
        &mut self,
        terminator: &MirTerminator,
    ) -> Result<bool, BackendError> {
        match terminator {
            MirTerminator::ArrayOperationCheck {
                failure:
                    MirArrayFailure::AllocationSize
                    | MirArrayFailure::InvalidSliceBounds
                    | MirArrayFailure::SliceLengthMismatch,
                success_target,
                failure_target,
                ..
            } => {
                self.output.push(Instruction::Test(Register::R11));
                self.output.push(Instruction::JumpIfNotZero(block_label(
                    self.program,
                    *success_target,
                )));
                self.output.push(Instruction::Jump(block_label(
                    self.program,
                    *failure_target,
                )));
                Ok(true)
            }
            MirTerminator::ArrayLoop {
                index,
                length,
                body_target,
                complete_target,
                ..
            } => {
                value::load_rax(value::frame_storage(self.frame, *index), self.output);
                self.output.push(Instruction::Move {
                    source: value::frame_storage(self.frame, *length),
                    destination: Register::R11.into(),
                });
                self.output.push(Instruction::Compare {
                    source: Register::R11,
                    destination: Register::Rax,
                });
                self.output.push(Instruction::JumpIfBelow(block_label(
                    self.program,
                    *body_target,
                )));
                self.output.push(Instruction::Jump(block_label(
                    self.program,
                    *complete_target,
                )));
                Ok(true)
            }
            MirTerminator::ArrayPositionCheck {
                position,
                kind:
                    MirArrayPositionKind::Element
                    | MirArrayPositionKind::SliceBound
                    | MirArrayPositionKind::RangeOffset,
                success_target,
                failure_target,
                ..
            } => {
                value::load_rax(value::frame_storage(self.frame, *position), self.output);
                self.output.push(Instruction::MoveImmediate64 {
                    bits: u64::MAX,
                    destination: Register::R11,
                });
                self.output.push(Instruction::Compare {
                    source: Register::R11,
                    destination: Register::Rax,
                });
                self.output.push(Instruction::JumpIfNotZero(block_label(
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

    fn select_array_allocate(
        &mut self,
        backing: crate::mir::StorageId,
        array: crate::identity::ArrayTypeId,
        length: crate::mir::ValueId,
        ownership: MirArrayOwnership,
    ) -> Result<(), BackendError> {
        self.select_array_allocate_length(
            backing,
            array,
            ArrayAllocationLength::Value(length),
            ownership,
        )
    }

    fn select_array_allocate_length(
        &mut self,
        backing: crate::mir::StorageId,
        array: crate::identity::ArrayTypeId,
        length: ArrayAllocationLength,
        ownership: MirArrayOwnership,
    ) -> Result<(), BackendError> {
        let layout = self
            .data_layout
            .array(array)
            .ok_or_else(|| self.array_error(format!("array {array} has no target layout")))?;
        let empty = self.next_array_label("allocate_empty");
        let failure = self.next_array_label("allocate_failure");
        let complete = self.next_array_label("allocate_complete");

        self.load_array_allocation_length(length);
        self.output.push(Instruction::MoveImmediate64 {
            bits: match ownership {
                MirArrayOwnership::Inline => layout.maximum_length(),
                MirArrayOwnership::Shared => layout.shared_maximum_length(),
            },
            destination: Register::R11,
        });
        self.output.push(Instruction::Compare {
            source: Register::R11,
            destination: Register::Rax,
        });
        self.output.push(Instruction::JumpIfAbove(failure.clone()));
        if ownership == MirArrayOwnership::Inline {
            self.output.push(Instruction::Test(Register::Rax));
            self.output.push(Instruction::JumpIfEqual(empty.clone()));
        }
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::try_from(layout.stride()).expect("array stride fits u64"),
            destination: Register::R11,
        });
        self.output.push(Instruction::Multiply {
            source: Register::R11,
            destination: Register::Rax,
        });
        self.output.push(Instruction::MoveImmediate64 {
            bits: u64::try_from(match ownership {
                MirArrayOwnership::Inline => layout.element_offset(),
                MirArrayOwnership::Shared => layout.shared_element_offset(),
            })
            .expect("array offset fits u64"),
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
        value::store_rax(value::frame_storage(self.frame, backing), self.output);
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdx.into(),
        });
        match ownership {
            MirArrayOwnership::Inline => {
                self.output.push(Instruction::MoveImmediate64 {
                    bits: 1,
                    destination: Register::Rax,
                });
                value::store_rax(
                    value::memory(Register::Rdx, ARRAY_OWNER_COUNT_OFFSET),
                    self.output,
                );
                self.load_array_allocation_length(length);
                value::store_rax(
                    value::memory(Register::Rdx, ARRAY_LENGTH_OFFSET),
                    self.output,
                );
            }
            MirArrayOwnership::Shared => {
                self.load_array_allocation_length(length);
                value::store_rax(
                    value::memory(Register::Rdx, SHARED_ARRAY_LENGTH_OFFSET),
                    self.output,
                );
            }
        }
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        });
        self.output.push(Instruction::Jump(complete.clone()));

        self.output.push(Instruction::Label(empty));
        self.clear_storage(backing);
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        });
        self.output.push(Instruction::Jump(complete.clone()));

        self.output.push(Instruction::Label(failure));
        self.clear_storage(backing);
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::R11,
        });
        self.output.push(Instruction::Label(complete));
        Ok(())
    }

    fn load_array_allocation_length(&mut self, length: ArrayAllocationLength) {
        match length {
            ArrayAllocationLength::Value(value_id) => {
                value::load_rax(value::frame_value(self.frame, value_id), self.output);
            }
            ArrayAllocationLength::Constant(bits) => {
                self.output.push(Instruction::MoveImmediate64 {
                    bits,
                    destination: Register::Rax,
                });
            }
        }
    }

    pub(super) fn select_array_element_place(
        &mut self,
        place: &MirPlace,
    ) -> Result<(FramePlace, Operand), BackendError> {
        let Some((
            element_index,
            MirPlaceProjection::ArrayElement {
                array,
                normalized_index,
            },
        )) = place
            .projections
            .iter()
            .enumerate()
            .rfind(|(_, projection)| matches!(projection, MirPlaceProjection::ArrayElement { .. }))
        else {
            return Err(self.array_error("array element place has no element projection"));
        };
        let declaration = self
            .program
            .array_type(*array)
            .ok_or_else(|| self.array_error(format!("array {array} is not declared")))?;
        let layout = self
            .data_layout
            .array(*array)
            .ok_or_else(|| self.array_error(format!("array {array} has no target layout")))?;
        let mut owner = place.clone();
        owner.projections.truncate(element_index);
        let shared = self.load_array_owner(&owner)?;
        let displacement = i32::try_from(if shared {
            layout.shared_element_offset()
        } else {
            layout.element_offset()
        })
        .map_err(|_| self.array_error(format!("array {array} offset cannot be encoded")))?;
        let base = if matches!(layout.stride(), 1 | 2 | 4 | 8) {
            self.output.push(Instruction::Move {
                source: Register::Rax.into(),
                destination: Register::R11.into(),
            });
            self.output.push(Instruction::Move {
                source: value::frame_storage(self.frame, *normalized_index),
                destination: Register::R10.into(),
            });
            None
        } else {
            self.output.push(Instruction::Move {
                source: Register::Rax.into(),
                destination: Register::R11.into(),
            });
            self.output.push(Instruction::Move {
                source: value::frame_storage(self.frame, *normalized_index),
                destination: Register::Rax.into(),
            });
            self.output.push(Instruction::MoveImmediate64 {
                bits: u64::try_from(layout.stride()).expect("array stride fits u64"),
                destination: Register::R10,
            });
            self.output.push(Instruction::Multiply {
                source: Register::R10,
                destination: Register::Rax,
            });
            self.output.push(Instruction::Add {
                source: Register::Rax,
                destination: Register::R11,
            });
            Some(Register::R11)
        };
        let mut ty = declaration.element;
        let mut displacement = displacement;
        for projection in &place.projections[element_index + 1..] {
            match *projection {
                MirPlaceProjection::Base(base) => {
                    let offset = self
                        .data_layout
                        .class(match ty {
                            MirType::Class(class) => class,
                            _ => {
                                return Err(self
                                    .array_error("array element base projection is not a class"))
                            }
                        })
                        .and_then(|layout| layout.base())
                        .filter(|layout| layout.class == base)
                        .ok_or_else(|| {
                            self.array_error("array element base projection has no target layout")
                        })?
                        .offset;
                    displacement =
                        checked_array_displacement(displacement, offset, self.function.callable())?;
                    ty = MirType::Class(base);
                }
                MirPlaceProjection::Field(field) => {
                    let offset = self
                        .data_layout
                        .field(field)
                        .ok_or_else(|| self.array_error(format!("field {field} has no layout")))?
                        .offset;
                    displacement =
                        checked_array_displacement(displacement, offset, self.function.callable())?;
                    ty = self.program.field(field).expect("verified field exists").ty;
                }
                MirPlaceProjection::OptionalPayload(class) => {
                    let offset = self.data_layout.optional_class(class)?.payload_offset();
                    displacement =
                        checked_array_displacement(displacement, offset, self.function.callable())?;
                    ty = MirType::Class(class);
                }
                MirPlaceProjection::ArrayElement { .. } => {
                    unreachable!("the final array projection was selected")
                }
            }
        }
        let operand = if let Some(base) = base {
            value::memory(base, displacement)
        } else {
            value::indexed_memory(
                Register::R11,
                Register::R10,
                u8::try_from(layout.stride()).expect("encodable array stride"),
                displacement,
            )
        };
        Ok((FramePlace::array_element(ty), operand))
    }

    fn select_array_element_normalize(
        &mut self,
        destination: crate::mir::StorageId,
        owner: &MirPlace,
        index: crate::mir::ValueId,
    ) -> Result<(), BackendError> {
        let shared = self.load_array_owner(owner)?;
        let empty = self.next_array_label("normalize_empty");
        let length_ready = self.next_array_label("normalize_length_ready");
        let valid = self.next_array_label("normalize_valid");
        let complete = self.next_array_label("normalize_complete");

        self.output.push(Instruction::Test(Register::Rax));
        self.output.push(Instruction::JumpIfEqual(empty.clone()));
        self.output.push(Instruction::Move {
            source: value::memory(
                Register::Rax,
                if shared {
                    SHARED_ARRAY_LENGTH_OFFSET
                } else {
                    ARRAY_LENGTH_OFFSET
                },
            ),
            destination: Register::Rdx.into(),
        });
        self.output.push(Instruction::Jump(length_ready.clone()));
        self.output.push(Instruction::Label(empty));
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rdx,
        });

        self.output.push(Instruction::Label(length_ready));
        value::load_rax(value::frame_value(self.frame, index), self.output);
        self.output.push(Instruction::Test(Register::Rax));
        let compare = self.next_array_label("normalize_compare");
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

    fn clear_storage(&mut self, storage: crate::mir::StorageId) {
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rax,
        });
        value::store_rax(value::frame_storage(self.frame, storage), self.output);
    }

    fn advance_array_index(&mut self, index: crate::mir::StorageId) {
        value::load_rax(value::frame_storage(self.frame, index), self.output);
        self.output.push(Instruction::MoveImmediate64 {
            bits: 1,
            destination: Register::R11,
        });
        self.output.push(Instruction::Add {
            source: Register::R11,
            destination: Register::Rax,
        });
        value::store_rax(value::frame_storage(self.frame, index), self.output);
    }

    fn copy_array_primitive(
        &mut self,
        destination: &MirPlace,
        source: &MirPlace,
    ) -> Result<(), BackendError> {
        let (source_layout, source) = self.frame_place(source)?;
        if source_layout.uses_byte_access() {
            self.output.push(Instruction::LoadZeroExtendByte {
                source,
                destination: Register::Rax,
            });
        } else {
            value::load_rax(source, self.output);
        }
        self.output.push(Instruction::ReserveStack(16));
        value::store_rax(value::memory(Register::Rsp, 0), self.output);
        let (destination_layout, destination) = self.frame_place(destination)?;
        value::load_rax(value::memory(Register::Rsp, 0), self.output);
        self.output.push(Instruction::ReleaseStack(16));
        if destination_layout.uses_byte_access() {
            self.output.push(Instruction::MoveByte {
                source: ByteRegister::Al,
                destination,
            });
        } else {
            value::store_rax(destination, self.output);
        }
        Ok(())
    }

    /// Loads an executable array backing into `rax` and reports whether its
    /// outer allocation uses the shared header. Nested array elements remain
    /// inline descriptors even when their containing outer array is shared.
    pub(super) fn load_array_owner(&mut self, owner: &MirPlace) -> Result<bool, BackendError> {
        let shared = owner.projections.is_empty()
            && (matches!(owner.base, crate::mir::MirPlaceBase::SharedPointee(_))
                || matches!(owner.base, crate::mir::MirPlaceBase::Storage(storage)
                    if self.array_backing_ownership(storage) == Some(MirArrayOwnership::Shared)));
        if shared {
            value::load_rax(
                value::frame_storage(self.frame, owner.base.expect_local_storage()),
                self.output,
            );
        } else {
            let (_, operand) = self.frame_place(owner)?;
            value::load_rax(operand, self.output);
        }
        Ok(shared)
    }

    fn array_backing_ownership(&self, backing: crate::mir::StorageId) -> Option<MirArrayOwnership> {
        self.function.body().blocks.iter().find_map(|block| {
            block
                .instructions
                .iter()
                .find_map(|instruction| match instruction {
                    crate::mir::MirInstruction::Array(
                        MirArrayInstruction::Allocate {
                            backing: candidate,
                            ownership,
                            ..
                        }
                        | MirArrayInstruction::AllocateElements {
                            backing: candidate,
                            ownership,
                            ..
                        },
                    ) if *candidate == backing => Some(*ownership),
                    _ => None,
                })
        })
    }

    fn clone_array_preserving_destination(
        &mut self,
        destination: &MirPlace,
        source: &MirPlace,
        array: crate::identity::ArrayTypeId,
    ) -> Result<(), BackendError> {
        self.materialize_place_address(destination, Register::Rdx)?;
        self.output.push(Instruction::ReserveStack(16));
        self.output.push(Instruction::Move {
            source: Register::Rdx.into(),
            destination: value::memory(Register::Rsp, 0),
        });
        let (_, source) = self.frame_place(source)?;
        value::load_rax(source, self.output);
        self.output.push(Instruction::Move {
            source: Register::Rax.into(),
            destination: Register::Rdi.into(),
        });
        self.emit_source_operation_call(symbol::array_clone(array))?;
        self.output.push(Instruction::Move {
            source: value::memory(Register::Rsp, 0),
            destination: Register::Rdx.into(),
        });
        self.output.push(Instruction::ReleaseStack(16));
        Ok(())
    }

    fn clear_place(&mut self, place: &MirPlace) -> Result<(), BackendError> {
        let (layout, destination) = self.frame_place(place)?;
        self.output.push(Instruction::MoveImmediate64 {
            bits: 0,
            destination: Register::Rax,
        });
        if layout.uses_byte_access() {
            self.output.push(Instruction::MoveByte {
                source: ByteRegister::Al,
                destination,
            });
        } else {
            value::store_rax(destination, self.output);
        }
        Ok(())
    }

    fn array_for_storage(
        &self,
        storage: crate::mir::StorageId,
    ) -> Result<crate::identity::ArrayTypeId, BackendError> {
        match self
            .function
            .storage(storage)
            .expect("verified array storage exists")
            .ty
        {
            MirType::Array(array) => Ok(array),
            _ => Err(self.array_error("array storage has no array type")),
        }
    }

    fn next_array_label(&mut self, purpose: &str) -> Label {
        let label = self.array_label(self.array_sequence, purpose);
        self.array_sequence += 1;
        label
    }

    fn array_label(&self, sequence: usize, purpose: &str) -> Label {
        Label::new(format!(
            ".Lska.{}.array_{}_{}_{}",
            symbol::local_label_stem(self.program, self.function.callable()),
            self.block.index(),
            sequence,
            purpose
        ))
    }

    fn array_error(&self, message: impl Into<String>) -> BackendError {
        BackendError::new(Target::X86_64SysV, Some(self.function.callable()), message)
    }
}

fn array_element_place(
    owner: MirPlace,
    array: crate::identity::ArrayTypeId,
    index: crate::mir::StorageId,
) -> MirPlace {
    owner.project_array_element(array, index)
}

fn array_for_place(
    program: &crate::mir::MirProgram,
    function: crate::mir::MirDefinitionRef<'_>,
    place: &MirPlace,
) -> Result<crate::identity::ArrayTypeId, BackendError> {
    let mut ty = match place.base {
        crate::mir::MirPlaceBase::StaticField(field)
        | crate::mir::MirPlaceBase::StaticLifecycleDestination(field) => {
            program
                .static_field(field)
                .expect("verified static array place has a declaration")
                .ty
        }
        _ => {
            function
                .storage(place.base.expect_local_storage())
                .expect("verified array place has storage")
                .ty
        }
    };
    for projection in &place.projections {
        ty = match *projection {
            MirPlaceProjection::Base(class) | MirPlaceProjection::OptionalPayload(class) => {
                MirType::Class(class)
            }
            MirPlaceProjection::Field(field) => {
                program.field(field).expect("verified field exists").ty
            }
            MirPlaceProjection::ArrayElement { array, .. } => {
                program
                    .array_type(array)
                    .expect("verified array exists")
                    .element
            }
        };
    }
    match ty {
        MirType::Array(array) => Ok(array),
        _ => Err(BackendError::new(
            Target::X86_64SysV,
            Some(function.callable()),
            "array destruction owner is not an inline array",
        )),
    }
}

fn checked_array_displacement(
    displacement: i32,
    offset: usize,
    callable: crate::identity::CallableId,
) -> Result<i32, BackendError> {
    i32::try_from(offset)
        .ok()
        .and_then(|offset| displacement.checked_add(offset))
        .ok_or_else(|| {
            BackendError::new(
                Target::X86_64SysV,
                Some(callable),
                "array element projection exceeds x86-64 displacement limits",
            )
        })
}
