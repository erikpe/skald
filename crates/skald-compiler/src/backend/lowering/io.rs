//! Standard-I/O calls over checked byte-array ranges.

use super::{context::Lowerer, LowerError};
use crate::{
    backend::{
        lir::{
            BinaryOperation, Call, CallArgument, CallAttribution, CallTarget, Constant, Edge,
            MemoryRepresentation, Operation, Terminator, ValueHandle,
        },
        plan::{ArtifactId, ComponentRole, PlanError, RuntimeService, ScalarType},
    },
    mir::{BlockId, MirIoBuffer, MirIoInstruction, MirIoOperation, StorageId},
    primitive_comparison::PrimitiveComparisonPredicate,
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn io_instruction(
        &mut self,
        block: BlockId,
        instruction: &MirIoInstruction,
    ) -> Result<(), LowerError> {
        let (service, values) = match &instruction.operation {
            MirIoOperation::StandardHandle { stream } => (
                RuntimeService::IoStandardHandle,
                vec![self.values[stream.index()]],
            ),
            MirIoOperation::Open { path, mode } => {
                let (pointer, length) = self.io_buffer_range(block, path, None)?;
                (
                    RuntimeService::IoOpen,
                    vec![pointer, length, self.values[mode.index()]],
                )
            }
            MirIoOperation::Read {
                handle,
                destination,
                offset,
            } => {
                let (pointer, length) = self.io_buffer_range(block, destination, Some(*offset))?;
                (
                    RuntimeService::IoRead,
                    vec![self.values[handle.index()], pointer, length],
                )
            }
            MirIoOperation::Write {
                handle,
                source,
                offset,
            } => {
                let (pointer, length) = self.io_buffer_range(block, source, Some(*offset))?;
                (
                    RuntimeService::IoWrite,
                    vec![self.values[handle.index()], pointer, length],
                )
            }
            MirIoOperation::Close { handle } => {
                (RuntimeService::IoClose, vec![self.values[handle.index()]])
            }
        };
        let target = ArtifactId::Runtime(service);
        let signature = self
            .plan()
            .artifact(self.plan().artifact_id(target)?, target.category())?
            .signature
            .ok_or(PlanError::InvalidSignature)?;
        let roles = self
            .plan()
            .signature(self.plan().signature_id(signature.index())?)?
            .inputs
            .iter()
            .map(|component| component.role)
            .collect::<Vec<_>>();
        if roles.len() != values.len()
            || roles
                .iter()
                .enumerate()
                .any(|(index, role)| *role != ComponentRole::RuntimeParameter(index))
        {
            return Err(PlanError::InvalidSignature.into());
        }
        let arguments = roles
            .into_iter()
            .zip(values)
            .map(|(role, value)| CallArgument { role, value })
            .collect();
        self.builder.append_into(
            self.active_blocks[block.index()],
            Operation::Call(Call {
                target: CallTarget::Direct(target),
                signature,
                arguments,
                // Standard-library code interprets recoverable I/O status and
                // attributes any language panic to its own source operation.
                attribution: CallAttribution::NonReporting,
            }),
            &[self.values[instruction.result.index()]],
        )?;
        Ok(())
    }

    fn io_buffer_range(
        &mut self,
        block: BlockId,
        buffer: &MirIoBuffer,
        offset: Option<StorageId>,
    ) -> Result<(ValueHandle<'plan>, ValueHandle<'plan>), LowerError> {
        let fact = self
            .plan()
            .semantic()
            .array(buffer.array)
            .ok_or(PlanError::UnknownDeclaration)?
            .clone();
        let (handle, shared) = self.array_owner(block, &buffer.place)?;
        let offset = offset
            .map(|storage| self.load_storage(block, storage))
            .transpose()?;
        let null = self.append(
            block,
            Operation::Constant(Constant::Null(ScalarType::DataAddress)),
        )?[0];
        let present = self.append(
            block,
            Operation::Compare {
                predicate: PrimitiveComparisonPredicate::NotEqual,
                left: handle,
                right: null,
            },
        )?[0];

        let nonempty = self.new_io_block()?;
        let empty = self.new_io_block()?;
        let pointer = self.builder.reserve_value(ScalarType::DataAddress, None)?;
        let length = self.builder.reserve_value(ScalarType::U64, None)?;
        let complete = self.builder.reserve_block()?;
        self.builder.define_block(complete, &[pointer, length])?;
        self.builder.terminate(
            self.active_blocks[block.index()],
            Terminator::Branch {
                condition: present,
                true_edge: io_edge(nonempty, &[]),
                false_edge: io_edge(empty, &[]),
            },
        )?;

        let length_offset = if shared {
            fact.shared_length_offset
        } else {
            fact.inline_length_offset
        };
        let element_offset = if shared {
            fact.shared_element_offset
        } else {
            fact.element_offset
        };
        let mut remaining = self.load_u64_at(nonempty, handle, length_offset)?;
        let mut data = self.offset_address_at(nonempty, handle, element_offset)?;
        if let Some(offset) = offset {
            data = self
                .builder
                .append(nonempty, Operation::ByteOffset { base: data, offset })?[0];
            remaining = self.builder.append(
                nonempty,
                Operation::Binary {
                    operation: BinaryOperation::Subtract,
                    left: remaining,
                    right: offset,
                },
            )?[0];
        }
        self.builder.terminate(
            nonempty,
            Terminator::Jump(io_edge(complete, &[data, remaining])),
        )?;

        let empty_pointer = self.builder.append(
            empty,
            Operation::Constant(Constant::Null(ScalarType::DataAddress)),
        )?[0];
        let empty_length = self
            .builder
            .append(empty, Operation::Constant(Constant::U64(0)))?[0];
        self.builder.terminate(
            empty,
            Terminator::Jump(io_edge(complete, &[empty_pointer, empty_length])),
        )?;
        self.active_blocks[block.index()] = complete;
        Ok((pointer, length))
    }

    fn new_io_block(&mut self) -> Result<crate::backend::lir::BlockHandle<'plan>, LowerError> {
        let block = self.builder.reserve_block()?;
        self.builder.define_block(block, &[])?;
        Ok(block)
    }

    fn offset_address_at(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        base: ValueHandle<'plan>,
        bytes: usize,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        if bytes == 0 {
            return Ok(base);
        }
        let offset = self.builder.append(
            block,
            Operation::Constant(Constant::U64(
                bytes.try_into().map_err(|_| PlanError::SizeOverflow)?,
            )),
        )?[0];
        Ok(self
            .builder
            .append(block, Operation::ByteOffset { base, offset })?[0])
    }

    fn load_u64_at(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        base: ValueHandle<'plan>,
        offset: usize,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let address = self.offset_address_at(block, base, offset)?;
        Ok(self.builder.append(
            block,
            Operation::Load {
                address,
                representation: MemoryRepresentation {
                    scalar: ScalarType::U64,
                    bytes: 8,
                    alignment: 8,
                },
            },
        )?[0])
    }
}

fn io_edge<'plan>(
    target: crate::backend::lir::BlockHandle<'plan>,
    arguments: &[ValueHandle<'plan>],
) -> Edge<ValueHandle<'plan>, crate::backend::lir::BlockHandle<'plan>> {
    Edge {
        target,
        arguments: arguments.to_vec(),
    }
}
