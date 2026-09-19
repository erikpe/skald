//! Checked array backing, position, loop, and anchor lowering.

use super::{context::Lowerer, LowerError};
use crate::{
    backend::{
        lir::{
            BinaryOperation, CallArgument, Constant, Conversion, Edge, MemoryRepresentation,
            Operation, Terminator, ValueHandle,
        },
        plan::{ArtifactId, ComponentRole, DataKey, PlanError, RuntimeService, ScalarType},
    },
    identity::ArrayTypeId,
    mir::{
        BlockId, MirArrayAnchorKind, MirArrayBoundary, MirArrayInstruction, MirArrayLoopKind,
        MirArrayOwnership, MirArrayPositionKind, MirPlace, MirPlaceBase, MirPlaceProjection,
        MirTerminator, MirType, StorageId, ValueId,
    },
    primitive_comparison::PrimitiveComparisonPredicate,
    source::Span,
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn array_instruction(
        &mut self,
        block: BlockId,
        instruction: &MirArrayInstruction,
    ) -> Result<(), LowerError> {
        match instruction {
            MirArrayInstruction::Allocate {
                backing,
                array,
                length,
                ownership,
                span,
                ..
            } => self.allocate_array(
                block,
                *backing,
                *array,
                self.values[length.index()],
                *ownership,
                *span,
            ),
            MirArrayInstruction::AllocateElements {
                backing,
                prefix,
                array,
                length,
                ownership,
                span,
                ..
            } => {
                self.store_u64(block, *prefix, 0)?;
                let length = self.constant_u64(block, *length)?;
                self.allocate_array(block, *backing, *array, length, *ownership, *span)
            }
            MirArrayInstruction::InitializeElement {
                backing,
                prefix,
                value,
                ..
            } => {
                let array = self.array_for_backing(*backing)?;
                let place = array_element(MirPlace::base(*backing), array, *prefix);
                let address = self.place_address(block, &place)?;
                let representation = self.place_representation(&place)?;
                self.append(
                    block,
                    Operation::Store {
                        address,
                        value: self.values[value.index()],
                        representation,
                    },
                )?;
                self.advance_index(block, *prefix)
            }
            MirArrayInstruction::CompleteElement { prefix, .. } => {
                self.advance_index(block, *prefix)
            }
            MirArrayInstruction::InitializeNext { backing, index, .. } => {
                let array = self.array_for_backing(*backing)?;
                let backing = self.load_storage(block, *backing)?;
                let backing =
                    self.byte_offset(block, backing, self.array_fact(array)?.element_offset)?;
                let index_value = self.load_storage(block, *index)?;
                self.call_array_helper(
                    block,
                    array,
                    crate::backend::plan::HelperFamily::ArrayElementInitializer,
                    vec![backing, index_value],
                    instruction.span(),
                )?;
                self.advance_index(block, *index)
            }
            MirArrayInstruction::CopyNext {
                backing,
                source,
                index,
                ..
            } => {
                let array = self.array_for_backing(*backing)?;
                let destination = self.load_storage(block, *backing)?;
                let (mut source, shared) = self.array_owner(block, source)?;
                if shared {
                    let fact = self.array_fact(array)?;
                    source = self.byte_offset(
                        block,
                        source,
                        fact.shared_element_offset - fact.element_offset,
                    )?;
                }
                let element_offset = self.array_fact(array)?.element_offset;
                let destination = self.byte_offset(block, destination, element_offset)?;
                source = self.byte_offset(block, source, element_offset)?;
                let index_value = self.load_storage(block, *index)?;
                self.call_array_helper(
                    block,
                    array,
                    crate::backend::plan::HelperFamily::ArrayElementCopier,
                    vec![destination, source, index_value, index_value],
                    instruction.span(),
                )?;
                self.advance_index(block, *index)
            }
            MirArrayInstruction::Publish {
                backing,
                destination,
                ..
            } => {
                let handle = self.load_storage(block, *backing)?;
                self.store(block, *destination, handle)?;
                self.clear_address_storage(block, *backing)
            }
            MirArrayInstruction::PublishShared {
                backing,
                destination,
                array,
                ..
            } => {
                let handle = self.load_storage(block, *backing)?;
                let metadata = self.data_address(block, DataKey::ArrayDescriptor(*array))?;
                let header = self.shared_header()?;
                self.store_at_offset(
                    block,
                    handle,
                    header.dynamic_metadata_offset,
                    metadata,
                    self.address_representation(),
                )?;
                let one = self.constant_u64(block, 1)?;
                self.store_at_offset(
                    block,
                    handle,
                    header.owner_count_offset,
                    one,
                    self.count_representation(),
                )?;
                self.store(block, *destination, handle)?;
                self.clear_address_storage(block, *backing)
            }
            MirArrayInstruction::Adopt {
                destination,
                source,
                ..
            } => {
                let handle = self.load_storage(block, *source)?;
                self.store_place(block, destination, handle)?;
                self.clear_address_storage(block, *source)
            }
            MirArrayInstruction::Replace {
                destination,
                source,
                array,
                span,
                ..
            } => {
                let previous = self.load_place(block, destination)?;
                let replacement = self.load_storage(block, *source)?;
                self.store_place(block, destination, replacement)?;
                self.clear_address_storage(block, *source)?;
                self.release_inline_array(block, previous, *array, *span)
            }
            MirArrayInstruction::ElementAssign {
                destination,
                source,
                operation,
                span,
            } => self.assign_array_element(block, destination, source, *operation, *span),
            MirArrayInstruction::DestroyNext { owner, index, .. } => {
                let array = match self.place_type(owner)? {
                    crate::backend::plan::SemanticType::Array(array) => array,
                    _ => return Err(PlanError::InvalidDomain.into()),
                };
                let (mut backing, shared) = self.array_owner(block, owner)?;
                if shared {
                    let fact = self.array_fact(array)?;
                    backing = self.byte_offset(
                        block,
                        backing,
                        fact.shared_element_offset - fact.element_offset,
                    )?;
                }
                backing =
                    self.byte_offset(block, backing, self.array_fact(array)?.element_offset)?;
                let index = self.load_storage(block, *index)?;
                self.call_array_helper(
                    block,
                    array,
                    crate::backend::plan::HelperFamily::ArrayElementDestroyer,
                    vec![backing, index],
                    instruction.span(),
                )?;
                Ok(())
            }
            MirArrayInstruction::Release { owner, array, span } => {
                let handle = self.load_place(block, owner)?;
                self.release_inline_array(block, handle, *array, *span)?;
                let null = self.null_address(block)?;
                self.store_place(block, owner, null)
            }
            MirArrayInstruction::AnchorBegin {
                anchor,
                owner,
                array,
                kind,
                span,
            } => self.begin_array_anchor(block, *anchor, owner, *array, *kind, *span),
            MirArrayInstruction::AnchorEnd { anchor, span } => {
                self.end_array_anchor(block, *anchor, *span)
            }
            MirArrayInstruction::AliasBind { alias, source, .. } => {
                let address = self.place_address(block, source)?;
                self.store(block, *alias, address)
            }
            MirArrayInstruction::Normalize {
                destination,
                owner,
                index,
                array,
                kind,
                ..
            } => self.normalize_array_position(block, *destination, owner, *index, *array, *kind),
            MirArrayInstruction::Offset {
                destination,
                owner,
                offset,
                array,
                ..
            } => self.offset_array_position(block, *destination, owner, *offset, *array),
            MirArrayInstruction::Boundary {
                destination,
                owner,
                array,
                boundary,
                ..
            } => {
                let value = match boundary {
                    MirArrayBoundary::Start => self.constant_u64(block, 0)?,
                    MirArrayBoundary::End => self.load_array_length(block, owner, *array)?,
                };
                self.store(block, *destination, value)
            }
            _ => Err(PlanError::InvalidDomain.into()),
        }
    }

    pub(super) fn array_length(
        &mut self,
        block: BlockId,
        result: ValueId,
        source: &MirPlace,
    ) -> Result<(), LowerError> {
        let crate::backend::plan::SemanticType::Array(array) = self.place_type(source)? else {
            return Err(PlanError::InvalidDomain.into());
        };
        let length = self.load_array_length(block, source, array)?;
        self.builder.append_into(
            self.active_blocks[block.index()],
            Operation::Convert {
                conversion: Conversion::Identity,
                value: length,
                target: ScalarType::U64,
                evidence: None,
            },
            &[self.values[result.index()]],
        )?;
        Ok(())
    }

    pub(super) fn array_terminator(
        &mut self,
        block: BlockId,
        terminator: &MirTerminator,
    ) -> Result<bool, LowerError> {
        let branch = match terminator {
            MirTerminator::ArrayPositionCheck {
                position,
                success_target,
                failure_target,
                ..
            } => {
                let value = self.load_storage(block, *position)?;
                let invalid = self.constant_u64(block, u64::MAX)?;
                let valid = self.compare(
                    block,
                    PrimitiveComparisonPredicate::NotEqual,
                    value,
                    invalid,
                )?;
                Some((valid, *success_target, *failure_target))
            }
            MirTerminator::ArrayOperationCheck {
                success_target,
                failure_target,
                ..
            } => Some((
                *self
                    .array_status
                    .get(&block)
                    .ok_or(PlanError::InvalidDomain)?,
                *success_target,
                *failure_target,
            )),
            MirTerminator::ArrayLoop {
                index,
                length,
                kind: MirArrayLoopKind::Ordinary,
                body_target,
                complete_target,
                ..
            } => {
                let index = self.load_storage(block, *index)?;
                let length = self.load_storage(block, *length)?;
                let running =
                    self.compare(block, PrimitiveComparisonPredicate::LessThan, index, length)?;
                Some((running, *body_target, *complete_target))
            }
            _ => None,
        };
        let Some((condition, yes, no)) = branch else {
            return Ok(false);
        };
        self.builder.terminate(
            self.active_blocks[block.index()],
            Terminator::Branch {
                condition,
                true_edge: self.edge(yes),
                false_edge: self.edge(no),
            },
        )?;
        Ok(true)
    }

    fn allocate_array(
        &mut self,
        block: BlockId,
        backing: StorageId,
        array: ArrayTypeId,
        length: ValueHandle<'plan>,
        ownership: MirArrayOwnership,
        span: Span,
    ) -> Result<(), LowerError> {
        let fact = self.array_fact(array)?.clone();
        let maximum = match ownership {
            MirArrayOwnership::Inline => fact.maximum_length,
            MirArrayOwnership::Shared => fact.shared_maximum_length,
        };
        let maximum = self.constant_u64(block, maximum)?;
        let valid = self.compare(
            block,
            PrimitiveComparisonPredicate::LessEqual,
            length,
            maximum,
        )?;
        let allocate = self.new_array_block()?;
        let rejected = self.new_array_block()?;
        let status = self.builder.reserve_value(ScalarType::Bool, Some(span))?;
        let complete = self.builder.reserve_block()?;
        self.builder.define_block(complete, &[status])?;
        self.branch_active(block, valid, allocate, rejected)?;

        let null = self.builder.append(
            rejected,
            Operation::Constant(Constant::Null(ScalarType::DataAddress)),
        )?[0];
        self.store_at_handle(rejected, backing, null)?;
        let no = self
            .builder
            .append(rejected, Operation::Constant(Constant::Bool(false)))?[0];
        self.builder
            .terminate(rejected, Terminator::Jump(edge_with(complete, no)))?;

        let allocated = if ownership == MirArrayOwnership::Inline {
            let zero = self
                .builder
                .append(allocate, Operation::Constant(Constant::U64(0)))?[0];
            let nonempty = self.builder.append(
                allocate,
                Operation::Compare {
                    predicate: PrimitiveComparisonPredicate::NotEqual,
                    left: length,
                    right: zero,
                },
            )?[0];
            let body = self.new_array_block()?;
            let empty = self.new_array_block()?;
            let handle = self
                .builder
                .reserve_value(ScalarType::DataAddress, Some(span))?;
            let joined = self.builder.reserve_block()?;
            self.builder.define_block(joined, &[handle])?;
            self.builder.terminate(
                allocate,
                Terminator::Branch {
                    condition: nonempty,
                    true_edge: edge(body),
                    false_edge: edge(empty),
                },
            )?;
            let null = self.builder.append(
                empty,
                Operation::Constant(Constant::Null(ScalarType::DataAddress)),
            )?[0];
            self.builder
                .terminate(empty, Terminator::Jump(edge_with(joined, null)))?;
            let allocated = self.allocate_array_bytes(body, length, &fact, ownership, span)?;
            self.initialize_array_header_at(body, allocated, length, &fact, ownership)?;
            self.builder
                .terminate(body, Terminator::Jump(edge_with(joined, allocated)))?;
            (joined, handle)
        } else {
            let handle = self.allocate_array_bytes(allocate, length, &fact, ownership, span)?;
            self.initialize_array_header_at(allocate, handle, length, &fact, ownership)?;
            (allocate, handle)
        };
        self.store_at_handle(allocated.0, backing, allocated.1)?;
        let yes = self
            .builder
            .append(allocated.0, Operation::Constant(Constant::Bool(true)))?[0];
        self.builder
            .terminate(allocated.0, Terminator::Jump(edge_with(complete, yes)))?;
        self.active_blocks[block.index()] = complete;
        self.array_status.insert(block, status);
        Ok(())
    }

    fn assign_array_element(
        &mut self,
        block: BlockId,
        destination: &MirPlace,
        source: &MirPlace,
        operation: crate::mir::MirArrayAssignElement,
        span: Span,
    ) -> Result<(), LowerError> {
        use crate::mir::MirArrayAssignElement;
        if destination == source {
            return Ok(());
        }
        match operation {
            MirArrayAssignElement::Primitive => {
                let source_address = self.place_address(block, source)?;
                let representation = self.place_representation(source)?;
                let value = self.append(
                    block,
                    Operation::Load {
                        address: source_address,
                        representation,
                    },
                )?[0];
                let destination_address = self.place_address(block, destination)?;
                self.append(
                    block,
                    Operation::Store {
                        address: destination_address,
                        value,
                        representation,
                    },
                )?;
                Ok(())
            }
            MirArrayAssignElement::OptionalPrimitive => {
                self.optional_primitive_copy(block, destination, source)
            }
            MirArrayAssignElement::Class { operation, .. } => self.copy_assignment_operation(
                block,
                operation,
                destination.clone(),
                source.clone(),
                span,
            ),
            MirArrayAssignElement::OptionalClass {
                class,
                copy_constructor,
                copy_assignment,
            } => {
                let optional = self
                    .admitted
                    .program()
                    .optional_for_payload(MirType::Class(class))
                    .ok_or(PlanError::UnknownDeclaration)?;
                self.class_optional_assign(
                    block,
                    &crate::mir::MirClassOptionalAssign {
                        optional,
                        destination: destination.clone(),
                        source: crate::mir::MirClassOptionalSource::Copy(source.clone()),
                        class,
                        copy_constructor: Some(copy_constructor),
                        copy_assignment: Some(copy_assignment),
                        authorization: None,
                        final_authorization: None,
                        span,
                    },
                )
            }
            MirArrayAssignElement::Array(array) => {
                let source_handle = self.load_place(block, source)?;
                let replacement = self.clone_inline_array(block, source_handle, array, span)?;
                let previous = self.load_place(block, destination)?;
                self.store_place(block, destination, replacement)?;
                self.release_inline_array(block, previous, array, span)
            }
            MirArrayAssignElement::Shared(_) => {
                self.shared_field_assign(block, destination, source, span)
            }
            MirArrayAssignElement::OptionalShared(_) => {
                self.optional_shared_copy(block, destination, source, span, true)
            }
            MirArrayAssignElement::Optional(optional) => self.aggregate_optional_assign(
                block,
                &crate::mir::MirAggregateOptionalAssign {
                    optional,
                    destination: destination.clone(),
                    source: crate::mir::MirAggregateOptionalSource::Copy(source.clone()),
                    authorization: None,
                    final_authorization: None,
                    span,
                },
            ),
        }
    }

    fn allocate_array_bytes(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        length: ValueHandle<'plan>,
        fact: &crate::backend::plan::ArrayLayoutFact,
        ownership: MirArrayOwnership,
        span: Span,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let stride = self.builder.append(
            block,
            Operation::Constant(Constant::U64(fact.stride as u64)),
        )?[0];
        let elements = self.builder.append(
            block,
            Operation::Binary {
                operation: BinaryOperation::Multiply,
                left: length,
                right: stride,
            },
        )?[0];
        let offset = match ownership {
            MirArrayOwnership::Inline => fact.element_offset,
            MirArrayOwnership::Shared => fact.shared_element_offset,
        };
        let offset = self
            .builder
            .append(block, Operation::Constant(Constant::U64(offset as u64)))?[0];
        let bytes = self.builder.append(
            block,
            Operation::Binary {
                operation: BinaryOperation::Add,
                left: elements,
                right: offset,
            },
        )?[0];
        let call = self.runtime_call_at(block, RuntimeService::Allocate, bytes, span)?;
        Ok(self.builder.append(block, Operation::Call(call))?[0])
    }

    fn initialize_array_header_at(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        handle: ValueHandle<'plan>,
        length: ValueHandle<'plan>,
        fact: &crate::backend::plan::ArrayLayoutFact,
        ownership: MirArrayOwnership,
    ) -> Result<(), LowerError> {
        if ownership == MirArrayOwnership::Inline {
            let one = self
                .builder
                .append(block, Operation::Constant(Constant::U64(1)))?[0];
            self.store_offset_at(block, handle, fact.inline_owner_count_offset, one)?;
        }
        let offset = match ownership {
            MirArrayOwnership::Inline => fact.inline_length_offset,
            MirArrayOwnership::Shared => fact.shared_length_offset,
        };
        self.store_offset_at(block, handle, offset, length)
    }

    fn normalize_array_position(
        &mut self,
        block: BlockId,
        destination: StorageId,
        owner: &MirPlace,
        index: ValueId,
        array: ArrayTypeId,
        kind: MirArrayPositionKind,
    ) -> Result<(), LowerError> {
        if kind == MirArrayPositionKind::RangeOffset {
            return Err(PlanError::InvalidDomain.into());
        }
        let length = self.load_array_length(block, owner, array)?;
        let index = self.values[index.index()];
        let zero = self.append(block, Operation::Constant(Constant::I64(0)))?[0];
        let negative = self.compare(block, PrimitiveComparisonPredicate::LessThan, index, zero)?;
        let index = self.append(
            block,
            Operation::Convert {
                conversion: Conversion::IntegerBits,
                value: index,
                target: ScalarType::U64,
                evidence: None,
            },
        )?[0];
        let adjusted = self.append(
            block,
            Operation::Binary {
                operation: BinaryOperation::Add,
                left: index,
                right: length,
            },
        )?[0];
        let selected = self.select_value(block, negative, adjusted, index, ScalarType::U64)?;
        let valid = self.compare(
            block,
            match kind {
                MirArrayPositionKind::Element => PrimitiveComparisonPredicate::LessThan,
                MirArrayPositionKind::SliceBound => PrimitiveComparisonPredicate::LessEqual,
                MirArrayPositionKind::RangeOffset => unreachable!(),
            },
            selected,
            length,
        )?;
        let invalid = self.constant_u64(block, u64::MAX)?;
        let value = self.select_value(block, valid, selected, invalid, ScalarType::U64)?;
        self.store(block, destination, value)
    }

    fn offset_array_position(
        &mut self,
        block: BlockId,
        destination: StorageId,
        owner: &MirPlace,
        offset: ValueId,
        array: ArrayTypeId,
    ) -> Result<(), LowerError> {
        let length = self.load_array_length(block, owner, array)?;
        let offset = self.values[offset.index()];
        let valid = self.compare(
            block,
            PrimitiveComparisonPredicate::LessEqual,
            offset,
            length,
        )?;
        let invalid = self.constant_u64(block, u64::MAX)?;
        let value = self.select_value(block, valid, offset, invalid, ScalarType::U64)?;
        self.store(block, destination, value)
    }

    pub(super) fn load_array_length(
        &mut self,
        block: BlockId,
        owner: &MirPlace,
        array: ArrayTypeId,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let (handle, shared) = self.array_owner(block, owner)?;
        let fact = self.array_fact(array)?;
        let offset = if shared {
            fact.shared_length_offset
        } else {
            fact.inline_length_offset
        };
        let null = self.null_address(block)?;
        let present = self.compare(block, PrimitiveComparisonPredicate::NotEqual, handle, null)?;
        let load = self.new_array_block()?;
        let empty = self.new_array_block()?;
        let result = self.builder.reserve_value(ScalarType::U64, None)?;
        let complete = self.builder.reserve_block()?;
        self.builder.define_block(complete, &[result])?;
        self.branch_active(block, present, load, empty)?;
        let address = self.byte_offset_at(load, handle, offset)?;
        let length = self.builder.append(
            load,
            Operation::Load {
                address,
                representation: count_representation(),
            },
        )?[0];
        self.builder
            .terminate(load, Terminator::Jump(edge_with(complete, length)))?;
        let zero = self
            .builder
            .append(empty, Operation::Constant(Constant::U64(0)))?[0];
        self.builder
            .terminate(empty, Terminator::Jump(edge_with(complete, zero)))?;
        self.active_blocks[block.index()] = complete;
        Ok(result)
    }

    pub(super) fn array_owner(
        &mut self,
        block: BlockId,
        owner: &MirPlace,
    ) -> Result<(ValueHandle<'plan>, bool), LowerError> {
        let shared = owner.projections.is_empty()
            && (matches!(owner.base, MirPlaceBase::SharedPointee(_))
                || matches!(owner.base, MirPlaceBase::Storage(storage)
                    if self.array_backings.get(&storage) == Some(&MirArrayOwnership::Shared)));
        let handle = if shared {
            self.load_storage(block, owner.base.expect_local_storage())?
        } else {
            self.load_place(block, owner)?
        };
        Ok((handle, shared))
    }

    fn begin_array_anchor(
        &mut self,
        block: BlockId,
        anchor: StorageId,
        owner: &MirPlace,
        array: ArrayTypeId,
        kind: MirArrayAnchorKind,
        span: Span,
    ) -> Result<(), LowerError> {
        let (mut handle, _) = self.array_owner(block, owner)?;
        if kind == MirArrayAnchorKind::InlineBacking
            && !matches!(owner.base, MirPlaceBase::AliasParameter(_))
        {
            self.call_owner_helper_if_present(
                block,
                crate::backend::plan::HelperFamily::Retain,
                handle,
                span,
            )?;
        } else if is_shared_anchor(kind) {
            let fact = self.array_fact(array)?;
            handle = self.byte_offset(
                block,
                handle,
                fact.shared_element_offset - fact.element_offset,
            )?;
        } else if kind == MirArrayAnchorKind::InlineBacking
            && matches!(owner.base, MirPlaceBase::AliasParameter(_))
        {
            handle = self.null_address(block)?;
        }
        self.store(block, anchor, handle)
    }

    fn end_array_anchor(
        &mut self,
        block: BlockId,
        anchor: StorageId,
        span: Span,
    ) -> Result<(), LowerError> {
        let storage = self
            .definition
            .storage(anchor)
            .ok_or(PlanError::InvalidDomain)?;
        if storage.kind
            == crate::mir::MirStorageKind::ArrayAnchor(MirArrayAnchorKind::InlineBacking)
        {
            let MirType::Array(array) = storage.ty else {
                return Err(PlanError::InvalidDomain.into());
            };
            let handle = self.load_storage(block, anchor)?;
            self.release_inline_array(block, handle, array, span)?;
        }
        self.clear_address_storage(block, anchor)
    }

    pub(super) fn release_inline_array(
        &mut self,
        block: BlockId,
        handle: ValueHandle<'plan>,
        array: ArrayTypeId,
        span: Span,
    ) -> Result<(), LowerError> {
        self.call_array_helper(
            block,
            array,
            crate::backend::plan::HelperFamily::ArrayRelease,
            vec![handle],
            span,
        )?;
        Ok(())
    }

    pub(super) fn clone_inline_array(
        &mut self,
        block: BlockId,
        handle: ValueHandle<'plan>,
        array: ArrayTypeId,
        span: Span,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let values = self.call_array_helper(
            block,
            array,
            crate::backend::plan::HelperFamily::ArrayClone,
            vec![handle],
            span,
        )?;
        values
            .into_iter()
            .next()
            .ok_or_else(|| PlanError::InvalidSignature.into())
    }

    fn call_array_helper(
        &mut self,
        block: BlockId,
        array: ArrayTypeId,
        family: crate::backend::plan::HelperFamily,
        values: Vec<ValueHandle<'plan>>,
        span: Span,
    ) -> Result<Vec<ValueHandle<'plan>>, LowerError> {
        let layout = self.array_fact(array)?.descriptor_layout;
        let target = self
            .plan()
            .resources()
            .generated
            .iter()
            .find_map(|fact| match fact.callable {
                crate::backend::plan::LirCallableId::Helper(key)
                    if key.family == family && key.layout == layout =>
                {
                    Some(fact.callable)
                }
                _ => None,
            })
            .ok_or(PlanError::UnknownDeclaration)?;
        let signature = self.callable_signature(target)?;
        let roles = self
            .plan()
            .signature(self.plan().signature_id(signature.index())?)?
            .inputs
            .iter()
            .map(|component| component.role)
            .collect::<Vec<_>>();
        if roles.len() != values.len() {
            return Err(PlanError::InvalidSignature.into());
        }
        let arguments = roles
            .into_iter()
            .zip(values)
            .map(|(role, value)| CallArgument { role, value })
            .collect();
        let attribution = self.attribution(block, span, false)?;
        self.append(
            block,
            Operation::Call(crate::backend::lir::Call {
                target: crate::backend::lir::CallTarget::Direct(ArtifactId::Callable(target)),
                signature,
                arguments,
                attribution,
            }),
        )
    }

    fn select_value(
        &mut self,
        block: BlockId,
        condition: ValueHandle<'plan>,
        yes: ValueHandle<'plan>,
        no: ValueHandle<'plan>,
        ty: ScalarType,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let yes_block = self.new_array_block()?;
        let no_block = self.new_array_block()?;
        let result = self.builder.reserve_value(ty, None)?;
        let complete = self.builder.reserve_block()?;
        self.builder.define_block(complete, &[result])?;
        self.branch_active(block, condition, yes_block, no_block)?;
        self.builder
            .terminate(yes_block, Terminator::Jump(edge_with(complete, yes)))?;
        self.builder
            .terminate(no_block, Terminator::Jump(edge_with(complete, no)))?;
        self.active_blocks[block.index()] = complete;
        Ok(result)
    }

    fn array_fact(
        &self,
        array: ArrayTypeId,
    ) -> Result<&crate::backend::plan::ArrayLayoutFact, LowerError> {
        self.plan()
            .semantic()
            .array(array)
            .ok_or_else(|| PlanError::UnknownDeclaration.into())
    }

    fn array_for_backing(&self, backing: StorageId) -> Result<ArrayTypeId, LowerError> {
        match self.definition.storage(backing).map(|storage| storage.ty) {
            Some(MirType::Array(array)) => Ok(array),
            _ => Err(PlanError::InvalidDomain.into()),
        }
    }

    fn advance_index(&mut self, block: BlockId, index: StorageId) -> Result<(), LowerError> {
        let value = self.load_storage(block, index)?;
        let one = self.constant_u64(block, 1)?;
        let next = self.append(
            block,
            Operation::Binary {
                operation: BinaryOperation::Add,
                left: value,
                right: one,
            },
        )?[0];
        self.store(block, index, next)
    }

    fn null_address(&mut self, block: BlockId) -> Result<ValueHandle<'plan>, LowerError> {
        Ok(self.append(
            block,
            Operation::Constant(Constant::Null(ScalarType::DataAddress)),
        )?[0])
    }

    fn clear_address_storage(
        &mut self,
        block: BlockId,
        storage: StorageId,
    ) -> Result<(), LowerError> {
        let null = self.null_address(block)?;
        self.store(block, storage, null)
    }

    fn store_u64(
        &mut self,
        block: BlockId,
        storage: StorageId,
        value: u64,
    ) -> Result<(), LowerError> {
        let value = self.constant_u64(block, value)?;
        self.store(block, storage, value)
    }

    fn new_array_block(&mut self) -> Result<crate::backend::lir::BlockHandle<'plan>, LowerError> {
        let block = self.builder.reserve_block()?;
        self.builder.define_block(block, &[])?;
        Ok(block)
    }

    fn store_offset_at(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        base: ValueHandle<'plan>,
        offset: usize,
        value: ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        let address = self.byte_offset_at(block, base, offset)?;
        self.builder.append(
            block,
            Operation::Store {
                address,
                value,
                representation: count_representation(),
            },
        )?;
        Ok(())
    }

    fn store_at_handle(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        storage: StorageId,
        value: ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        let address = self.address_at(block, storage)?;
        self.builder.append(
            block,
            Operation::Store {
                address,
                value,
                representation: self.representation(storage)?,
            },
        )?;
        Ok(())
    }

    fn runtime_call_at(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        service: RuntimeService,
        value: ValueHandle<'plan>,
        span: Span,
    ) -> Result<crate::backend::lir::Call<ValueHandle<'plan>>, LowerError> {
        let target = ArtifactId::Runtime(service);
        let signature = self
            .plan()
            .artifact(self.plan().artifact_id(target)?, target.category())?
            .signature
            .ok_or(PlanError::InvalidSignature)?;
        Ok(crate::backend::lir::Call {
            target: crate::backend::lir::CallTarget::Direct(target),
            signature,
            arguments: vec![CallArgument {
                role: ComponentRole::RuntimeParameter(0),
                value,
            }],
            attribution: self.synthetic_attribution(block, span)?,
        })
    }
}

fn array_element(mut owner: MirPlace, array: ArrayTypeId, index: StorageId) -> MirPlace {
    owner.projections.push(MirPlaceProjection::ArrayElement {
        array,
        normalized_index: index,
    });
    owner
}

fn count_representation() -> MemoryRepresentation {
    MemoryRepresentation {
        scalar: ScalarType::U64,
        bytes: 8,
        alignment: 8,
    }
}

fn edge<'plan>(
    target: crate::backend::lir::BlockHandle<'plan>,
) -> Edge<ValueHandle<'plan>, crate::backend::lir::BlockHandle<'plan>> {
    Edge {
        target,
        arguments: vec![],
    }
}

fn edge_with<'plan>(
    target: crate::backend::lir::BlockHandle<'plan>,
    value: ValueHandle<'plan>,
) -> Edge<ValueHandle<'plan>, crate::backend::lir::BlockHandle<'plan>> {
    Edge {
        target,
        arguments: vec![value],
    }
}

const fn is_shared_anchor(kind: MirArrayAnchorKind) -> bool {
    matches!(
        kind,
        MirArrayAnchorKind::StableSharedOwner
            | MirArrayAnchorKind::CopiedSharedOwner
            | MirArrayAnchorKind::AdoptedSharedOwner
            | MirArrayAnchorKind::SecuredOptionalSharedOwner
    )
}
