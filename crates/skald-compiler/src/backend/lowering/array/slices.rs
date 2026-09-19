//! Slice validation, snapshot construction, and element assignment.

use super::{array_element, edge, Lowerer};
use crate::{
    backend::{
        lir::{BinaryOperation, Operation, Terminator},
        plan::{HelperFamily, PlanError},
    },
    identity::ArrayTypeId,
    mir::{BlockId, MirArrayAssignElement, MirPlace, StorageId},
    primitive_comparison::PrimitiveComparisonPredicate,
    source::Span,
};

pub(super) struct SliceCopy<'mir> {
    pub(super) destination: StorageId,
    pub(super) source: &'mir MirPlace,
    pub(super) start: StorageId,
    pub(super) end: StorageId,
    pub(super) array: ArrayTypeId,
    pub(super) span: Span,
}

pub(super) struct SliceAssignment<'mir> {
    pub(super) destination: &'mir MirPlace,
    pub(super) source: &'mir MirPlace,
    pub(super) destination_index: StorageId,
    pub(super) source_index: StorageId,
    pub(super) operation: MirArrayAssignElement,
    pub(super) span: Span,
}

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn check_slice_bounds(
        &mut self,
        block: BlockId,
        start: StorageId,
        end: StorageId,
    ) -> Result<(), super::LowerError> {
        let start = self.load_storage(block, start)?;
        let end = self.load_storage(block, end)?;
        let valid = self.compare(block, PrimitiveComparisonPredicate::LessEqual, start, end)?;
        self.array_status.insert(block, valid);
        Ok(())
    }

    pub(super) fn check_slice_length(
        &mut self,
        block: BlockId,
        destination_start: StorageId,
        destination_end: StorageId,
        source: &MirPlace,
        array: ArrayTypeId,
    ) -> Result<(), super::LowerError> {
        let source_length = self.load_array_length(block, source, array)?;
        let start = self.load_storage(block, destination_start)?;
        let end = self.load_storage(block, destination_end)?;
        let destination_length = self.append(
            block,
            Operation::Binary {
                operation: BinaryOperation::Subtract,
                left: end,
                right: start,
            },
        )?[0];
        let valid = self.compare(
            block,
            PrimitiveComparisonPredicate::Equal,
            destination_length,
            source_length,
        )?;
        self.array_status.insert(block, valid);
        Ok(())
    }

    pub(super) fn copy_array_slice(
        &mut self,
        block: BlockId,
        copy: SliceCopy<'_>,
    ) -> Result<(), super::LowerError> {
        let SliceCopy {
            destination,
            source,
            start,
            end,
            array,
            span,
        } = copy;
        let fact = self.array_fact(array)?.clone();
        let (source, shared) = self.array_owner(block, source)?;
        let source = if shared {
            self.byte_offset(
                block,
                source,
                fact.shared_element_offset - fact.element_offset,
            )?
        } else {
            source
        };
        let start = self.load_storage(block, start)?;
        let end = self.load_storage(block, end)?;
        let length = self.append(
            block,
            Operation::Binary {
                operation: BinaryOperation::Subtract,
                left: end,
                right: start,
            },
        )?[0];
        let stride = self.constant_u64(block, fact.stride as u64)?;
        let displacement = self.append(
            block,
            Operation::Binary {
                operation: BinaryOperation::Multiply,
                left: start,
                right: stride,
            },
        )?[0];
        let source = self.append(
            block,
            Operation::ByteOffset {
                base: source,
                offset: displacement,
            },
        )?[0];
        let values = self.call_array_helper(
            block,
            array,
            HelperFamily::ArraySliceClone,
            vec![source, length],
            span,
        )?;
        let handle = values
            .into_iter()
            .next()
            .ok_or(PlanError::InvalidSignature)?;
        self.store(block, destination, handle)
    }

    pub(super) fn assign_array_slice(
        &mut self,
        block: BlockId,
        assignment: SliceAssignment<'_>,
    ) -> Result<(), super::LowerError> {
        let SliceAssignment {
            destination,
            source,
            destination_index,
            source_index,
            operation,
            span,
        } = assignment;
        let array = match self.place_type(source)? {
            crate::backend::plan::SemanticType::Array(array) => array,
            _ => return Err(PlanError::InvalidDomain.into()),
        };
        let length = self.load_array_length(block, source, array)?;
        if operation == MirArrayAssignElement::Primitive {
            let fact = self.array_fact(array)?.clone();
            let (mut destination_elements, destination_shared) =
                self.array_owner(block, destination)?;
            let (mut source_elements, source_shared) = self.array_owner(block, source)?;
            if destination_shared {
                destination_elements = self.byte_offset(
                    block,
                    destination_elements,
                    fact.shared_element_offset - fact.element_offset,
                )?;
            }
            if source_shared {
                source_elements = self.byte_offset(
                    block,
                    source_elements,
                    fact.shared_element_offset - fact.element_offset,
                )?;
            }
            destination_elements =
                self.byte_offset(block, destination_elements, fact.element_offset)?;
            source_elements = self.byte_offset(block, source_elements, fact.element_offset)?;
            let destination_start = self.load_storage(block, destination_index)?;
            let source_start = self.load_storage(block, source_index)?;
            let copied = self.append(
                block,
                Operation::Binary {
                    operation: BinaryOperation::Subtract,
                    left: length,
                    right: source_start,
                },
            )?[0];
            let stride = self.constant_u64(block, fact.stride as u64)?;
            let destination_offset = self.append(
                block,
                Operation::Binary {
                    operation: BinaryOperation::Multiply,
                    left: destination_start,
                    right: stride,
                },
            )?[0];
            let source_offset = self.append(
                block,
                Operation::Binary {
                    operation: BinaryOperation::Multiply,
                    left: source_start,
                    right: stride,
                },
            )?[0];
            destination_elements = self.append(
                block,
                Operation::ByteOffset {
                    base: destination_elements,
                    offset: destination_offset,
                },
            )?[0];
            source_elements = self.append(
                block,
                Operation::ByteOffset {
                    base: source_elements,
                    offset: source_offset,
                },
            )?[0];
            self.call_array_helper(
                block,
                array,
                HelperFamily::ArrayPrimitiveSliceAssign,
                vec![destination_elements, source_elements, copied],
                span,
            )?;
            let destination_end = self.append(
                block,
                Operation::Binary {
                    operation: BinaryOperation::Add,
                    left: destination_start,
                    right: copied,
                },
            )?[0];
            self.store(block, destination_index, destination_end)?;
            return self.store(block, source_index, length);
        }
        let header = self.new_array_block()?;
        let body = self.new_array_block()?;
        let complete = self.new_array_block()?;
        self.builder.terminate(
            self.active_blocks[block.index()],
            Terminator::Jump(edge(header)),
        )?;

        self.active_blocks[block.index()] = header;
        let index = self.load_storage(block, source_index)?;
        let running = self.compare(block, PrimitiveComparisonPredicate::LessThan, index, length)?;
        self.builder.terminate(
            self.active_blocks[block.index()],
            Terminator::Branch {
                condition: running,
                true_edge: edge(body),
                false_edge: edge(complete),
            },
        )?;

        self.active_blocks[block.index()] = body;
        let destination = array_element(destination.clone(), array, destination_index);
        let source = array_element(source.clone(), array, source_index);
        self.assign_array_element(block, &destination, &source, operation, span)?;
        self.advance_index(block, destination_index)?;
        self.advance_index(block, source_index)?;
        self.builder.terminate(
            self.active_blocks[block.index()],
            Terminator::Jump(edge(header)),
        )?;
        self.active_blocks[block.index()] = complete;
        Ok(())
    }
}
