//! Primitive tagged optionals and nullable shared-owner optionals.

use super::{context::Lowerer, LowerError};
use crate::{
    backend::{
        lir::{
            BlockHandle, CallAttribution, Constant, Edge, MemoryRepresentation, ObjectHandle,
            Operation, Terminator, ValueHandle,
        },
        plan::{HelperFamily, OptionalLayoutFact, PlanError, ScalarType, SemanticType},
    },
    mir::{
        BlockId, MirOptionalAssign, MirOptionalInitialize, MirOptionalSharedAssign,
        MirOptionalSharedCleanup, MirOptionalSharedInitialize, MirOptionalSharedSource,
        MirOptionalSource, MirPlace, MirPresenceTestKind, MirTerminator, StorageId,
    },
    primitive_comparison::PrimitiveComparisonPredicate,
    source::Span,
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn optional_primitive_copy(
        &mut self,
        block: BlockId,
        destination: &MirPlace,
        source: &MirPlace,
    ) -> Result<(), LowerError> {
        self.write_primitive_optional(block, destination, &MirOptionalSource::Copy(source.clone()))
    }

    pub(super) fn optional_shared_copy(
        &mut self,
        block: BlockId,
        destination: &MirPlace,
        source: &MirPlace,
        span: Span,
        assign: bool,
    ) -> Result<(), LowerError> {
        let replacement = self.optional_shared_source(
            block,
            &MirOptionalSharedSource::Copy(source.clone()),
            span,
        )?;
        if assign {
            let previous = self.load_place(block, destination)?;
            self.call_owner_helper_if_present(block, HelperFamily::Release, previous, span)?;
        }
        self.store_place(block, destination, replacement)
    }

    pub(super) fn optional_initialize(
        &mut self,
        block: BlockId,
        initialize: &MirOptionalInitialize,
    ) -> Result<(), LowerError> {
        self.write_primitive_optional(block, &initialize.destination, &initialize.source)
    }

    pub(super) fn optional_assign(
        &mut self,
        block: BlockId,
        assign: &MirOptionalAssign,
    ) -> Result<(), LowerError> {
        self.write_primitive_optional(block, &assign.destination, &assign.source)
    }

    pub(super) fn optional_shared_initialize(
        &mut self,
        block: BlockId,
        initialize: &MirOptionalSharedInitialize,
    ) -> Result<(), LowerError> {
        let handle = self.optional_shared_source(block, &initialize.source, initialize.span)?;
        self.store_place(block, &initialize.destination, handle)
    }

    pub(super) fn optional_shared_assign(
        &mut self,
        block: BlockId,
        assign: &MirOptionalSharedAssign,
    ) -> Result<(), LowerError> {
        let replacement = self.optional_shared_source(block, &assign.source, assign.span)?;
        let previous = self.load_place(block, &assign.destination)?;
        self.call_owner_helper_if_present(block, HelperFamily::Release, previous, assign.span)?;
        self.store_place(block, &assign.destination, replacement)
    }

    pub(super) fn optional_shared_cleanup(
        &mut self,
        block: BlockId,
        cleanup: &MirOptionalSharedCleanup,
    ) -> Result<(), LowerError> {
        let handle = self.load_place(block, &cleanup.destination)?;
        self.call_owner_helper_if_present(block, HelperFamily::Release, handle, cleanup.span)
    }

    pub(super) fn optional_presence(
        &mut self,
        block: BlockId,
        source: &MirPlace,
        kind: MirPresenceTestKind,
    ) -> Result<Operation<ValueHandle<'plan>, ObjectHandle<'plan>, BlockHandle<'plan>>, LowerError>
    {
        let fact = self.optional_fact(source)?;
        let (value, absent) = if fact.nullable_niche {
            (
                self.load_place(block, source)?,
                self.append_optional(
                    block,
                    Operation::Constant(Constant::Null(ScalarType::DataAddress)),
                )?[0],
            )
        } else {
            let address = self.optional_state_address(block, source, &fact)?;
            (
                self.load_optional_at(block, address, state_representation())?,
                self.append_optional(block, Operation::Constant(Constant::U64(0)))?[0],
            )
        };
        Ok(Operation::Compare {
            predicate: match kind {
                MirPresenceTestKind::Some => PrimitiveComparisonPredicate::NotEqual,
                MirPresenceTestKind::None => PrimitiveComparisonPredicate::Equal,
            },
            left: value,
            right: absent,
        })
    }

    pub(super) fn optional_terminator(
        &mut self,
        block: BlockId,
        terminator: &MirTerminator,
    ) -> Result<bool, LowerError> {
        match terminator {
            MirTerminator::OptionalUnwrap {
                source,
                destination,
                success_target,
                failure_target,
                ..
            } => {
                self.unwrap_primitive(
                    block,
                    source,
                    *destination,
                    *success_target,
                    *failure_target,
                )?;
                Ok(true)
            }
            MirTerminator::OptionalSharedUnwrap {
                unwrap,
                success_target,
                failure_target,
                ..
            } => {
                self.unwrap_shared(
                    block,
                    &unwrap.source,
                    unwrap.destination,
                    *success_target,
                    *failure_target,
                    unwrap.span,
                )?;
                Ok(true)
            }
            other => self.optional_access_terminator(block, other),
        }
    }

    fn write_primitive_optional(
        &mut self,
        block: BlockId,
        destination: &MirPlace,
        source: &MirOptionalSource,
    ) -> Result<(), LowerError> {
        let fact = self.optional_fact(destination)?;
        if fact.nullable_niche || fact.storage != crate::backend::plan::OptionalStorageFact::Scalar
        {
            return Err(PlanError::InvalidDomain.into());
        }
        let destination_base = self.place_address(block, destination)?;
        let destination_state = self.byte_offset(
            block,
            destination_base,
            fact.state_offset.ok_or(PlanError::InvalidLayout)?,
        )?;
        let destination_payload = self.byte_offset(block, destination_base, fact.payload_offset)?;
        match source {
            MirOptionalSource::Absent => {
                let absent = self.append_optional(block, Operation::Constant(Constant::U64(0)))?[0];
                self.store_optional_at(block, destination_state, absent, state_representation())
            }
            MirOptionalSource::Present(value) => {
                self.store_optional_at(
                    block,
                    destination_payload,
                    self.values[value.index()],
                    self.optional_payload_representation(&fact)?,
                )?;
                let present =
                    self.append_optional(block, Operation::Constant(Constant::U64(1)))?[0];
                self.store_optional_at(block, destination_state, present, state_representation())
            }
            MirOptionalSource::Copy(source) => {
                let source_base = self.place_address(block, source)?;
                let source_state = self.byte_offset(
                    block,
                    source_base,
                    fact.state_offset.ok_or(PlanError::InvalidLayout)?,
                )?;
                let state = self.load_optional_at(block, source_state, state_representation())?;
                let zero = self.append_optional(block, Operation::Constant(Constant::U64(0)))?[0];
                let present = self.append_optional(
                    block,
                    Operation::Compare {
                        predicate: PrimitiveComparisonPredicate::NotEqual,
                        left: state,
                        right: zero,
                    },
                )?[0];
                let present_block = self.new_optional_block()?;
                let absent_block = self.new_optional_block()?;
                let complete = self.new_optional_block()?;
                self.branch_active(block, present, present_block, absent_block)?;

                self.store_optional_at_handle(
                    absent_block,
                    destination_state,
                    zero,
                    state_representation(),
                )?;
                self.jump_optional(absent_block, complete)?;

                let source_payload =
                    self.byte_offset_at(present_block, source_base, fact.payload_offset)?;
                let payload = self.load_optional_at_handle(
                    present_block,
                    source_payload,
                    self.optional_payload_representation(&fact)?,
                )?;
                self.store_optional_at_handle(
                    present_block,
                    destination_payload,
                    payload,
                    self.optional_payload_representation(&fact)?,
                )?;
                let one = self
                    .builder
                    .append(present_block, Operation::Constant(Constant::U64(1)))?[0];
                self.store_optional_at_handle(
                    present_block,
                    destination_state,
                    one,
                    state_representation(),
                )?;
                self.jump_optional(present_block, complete)?;
                self.active_blocks[block.index()] = complete;
                Ok(())
            }
        }
    }

    fn optional_shared_source(
        &mut self,
        block: BlockId,
        source: &MirOptionalSharedSource,
        span: Span,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let handle = match source {
            MirOptionalSharedSource::Absent => self.append_optional(
                block,
                Operation::Constant(Constant::Null(ScalarType::DataAddress)),
            )?[0],
            MirOptionalSharedSource::Present(storage) | MirOptionalSharedSource::Move(storage) => {
                self.load_storage(block, *storage)?
            }
            MirOptionalSharedSource::Copy(place) => {
                let handle = self.load_place(block, place)?;
                self.call_owner_helper_if_present(block, HelperFamily::Retain, handle, span)?;
                handle
            }
        };
        Ok(handle)
    }

    pub(super) fn call_owner_helper_if_present(
        &mut self,
        block: BlockId,
        family: HelperFamily,
        handle: ValueHandle<'plan>,
        span: Span,
    ) -> Result<(), LowerError> {
        let null = self.append_optional(
            block,
            Operation::Constant(Constant::Null(ScalarType::DataAddress)),
        )?[0];
        let present = self.append_optional(
            block,
            Operation::Compare {
                predicate: PrimitiveComparisonPredicate::NotEqual,
                left: handle,
                right: null,
            },
        )?[0];
        let call = self.new_optional_block()?;
        let complete = self.new_optional_block()?;
        let current = self.active_blocks[block.index()];
        self.builder.terminate(
            current,
            Terminator::Branch {
                condition: present,
                true_edge: edge(call),
                false_edge: edge(complete),
            },
        )?;
        let attribution = self.synthetic_attribution(call, span)?;
        self.call_owner_helper_at(call, family, handle, attribution)?;
        self.jump_optional(call, complete)?;
        self.active_blocks[block.index()] = complete;
        Ok(())
    }

    fn unwrap_primitive(
        &mut self,
        block: BlockId,
        source: &MirPlace,
        destination: StorageId,
        success_target: BlockId,
        failure_target: BlockId,
    ) -> Result<(), LowerError> {
        let fact = self.optional_fact(source)?;
        if fact.nullable_niche || fact.storage != crate::backend::plan::OptionalStorageFact::Scalar
        {
            return Err(PlanError::InvalidDomain.into());
        }
        let base = self.place_address(block, source)?;
        let state_address = self.byte_offset(
            block,
            base,
            fact.state_offset.ok_or(PlanError::InvalidLayout)?,
        )?;
        let state = self.load_optional_at(block, state_address, state_representation())?;
        let zero = self.append_optional(block, Operation::Constant(Constant::U64(0)))?[0];
        let present = self.append_optional(
            block,
            Operation::Compare {
                predicate: PrimitiveComparisonPredicate::NotEqual,
                left: state,
                right: zero,
            },
        )?[0];
        let extract = self.new_optional_block()?;
        let current = self.active_blocks[block.index()];
        self.builder.terminate(
            current,
            Terminator::Branch {
                condition: present,
                true_edge: edge(extract),
                false_edge: self.edge(failure_target),
            },
        )?;
        let payload_address = self.byte_offset_at(extract, base, fact.payload_offset)?;
        let payload = self.load_optional_at_handle(
            extract,
            payload_address,
            self.optional_payload_representation(&fact)?,
        )?;
        let destination_address = self.address_at(extract, destination)?;
        self.store_optional_at_handle(
            extract,
            destination_address,
            payload,
            self.representation(destination)?,
        )?;
        self.builder
            .terminate(extract, Terminator::Jump(self.edge(success_target)))?;
        Ok(())
    }

    fn unwrap_shared(
        &mut self,
        block: BlockId,
        source: &MirPlace,
        destination: StorageId,
        success_target: BlockId,
        failure_target: BlockId,
        span: Span,
    ) -> Result<(), LowerError> {
        let handle = self.load_place(block, source)?;
        let null = self.append_optional(
            block,
            Operation::Constant(Constant::Null(ScalarType::DataAddress)),
        )?[0];
        let present = self.append_optional(
            block,
            Operation::Compare {
                predicate: PrimitiveComparisonPredicate::NotEqual,
                left: handle,
                right: null,
            },
        )?[0];
        let secure = self.new_optional_block()?;
        let current = self.active_blocks[block.index()];
        self.builder.terminate(
            current,
            Terminator::Branch {
                condition: present,
                true_edge: edge(secure),
                false_edge: self.edge(failure_target),
            },
        )?;
        let attribution: CallAttribution = self.synthetic_attribution(secure, span)?;
        self.call_owner_helper_at(secure, HelperFamily::Retain, handle, attribution)?;
        let destination_address = self.address_at(secure, destination)?;
        self.store_optional_at_handle(
            secure,
            destination_address,
            handle,
            self.representation(destination)?,
        )?;
        self.builder
            .terminate(secure, Terminator::Jump(self.edge(success_target)))?;
        Ok(())
    }

    pub(super) fn optional_fact(&self, place: &MirPlace) -> Result<OptionalLayoutFact, LowerError> {
        let SemanticType::Optional(optional) = self.place_type(place)? else {
            return Err(PlanError::InvalidDomain.into());
        };
        self.plan()
            .semantic()
            .optional(optional)
            .cloned()
            .ok_or_else(|| PlanError::UnknownDeclaration.into())
    }

    fn optional_payload_representation(
        &self,
        fact: &OptionalLayoutFact,
    ) -> Result<MemoryRepresentation, LowerError> {
        let layout = self
            .plan()
            .layout(self.plan().layout_id(fact.payload_layout.index())?)?;
        Ok(MemoryRepresentation {
            scalar: match fact.payload {
                SemanticType::I64 => ScalarType::I64,
                SemanticType::U64 => ScalarType::U64,
                SemanticType::U8 => ScalarType::U8,
                SemanticType::F64 => ScalarType::F64,
                SemanticType::Bool => ScalarType::Bool,
                _ => return Err(PlanError::InvalidDomain.into()),
            },
            bytes: layout.size,
            alignment: layout.alignment,
        })
    }

    pub(super) fn optional_state_address(
        &mut self,
        block: BlockId,
        place: &MirPlace,
        fact: &OptionalLayoutFact,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let base = self.place_address(block, place)?;
        self.byte_offset(
            block,
            base,
            fact.state_offset.ok_or(PlanError::InvalidLayout)?,
        )
    }

    pub(super) fn new_optional_block(
        &mut self,
    ) -> Result<crate::backend::lir::BlockHandle<'plan>, LowerError> {
        let block = self.builder.reserve_block()?;
        self.builder.define_block(block, &[])?;
        Ok(block)
    }

    pub(super) fn branch_active(
        &mut self,
        block: BlockId,
        condition: ValueHandle<'plan>,
        yes: crate::backend::lir::BlockHandle<'plan>,
        no: crate::backend::lir::BlockHandle<'plan>,
    ) -> Result<(), LowerError> {
        self.builder.terminate(
            self.active_blocks[block.index()],
            Terminator::Branch {
                condition,
                true_edge: edge(yes),
                false_edge: edge(no),
            },
        )?;
        Ok(())
    }

    pub(super) fn jump_optional(
        &mut self,
        from: crate::backend::lir::BlockHandle<'plan>,
        to: crate::backend::lir::BlockHandle<'plan>,
    ) -> Result<(), LowerError> {
        self.builder.terminate(from, Terminator::Jump(edge(to)))?;
        Ok(())
    }

    fn append_optional(
        &mut self,
        block: BlockId,
        operation: Operation<ValueHandle<'plan>, ObjectHandle<'plan>, BlockHandle<'plan>>,
    ) -> Result<Vec<ValueHandle<'plan>>, LowerError> {
        Ok(self
            .builder
            .append(self.active_blocks[block.index()], operation)?)
    }

    pub(super) fn load_optional_at(
        &mut self,
        block: BlockId,
        address: ValueHandle<'plan>,
        representation: MemoryRepresentation,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        Ok(self.append_optional(
            block,
            Operation::Load {
                address,
                representation,
            },
        )?[0])
    }

    pub(super) fn load_optional_at_handle(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        address: ValueHandle<'plan>,
        representation: MemoryRepresentation,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        Ok(self.builder.append(
            block,
            Operation::Load {
                address,
                representation,
            },
        )?[0])
    }

    pub(super) fn store_optional_at(
        &mut self,
        block: BlockId,
        address: ValueHandle<'plan>,
        value: ValueHandle<'plan>,
        representation: MemoryRepresentation,
    ) -> Result<(), LowerError> {
        self.append_optional(
            block,
            Operation::Store {
                address,
                value,
                representation,
            },
        )?;
        Ok(())
    }

    pub(super) fn store_optional_at_handle(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        address: ValueHandle<'plan>,
        value: ValueHandle<'plan>,
        representation: MemoryRepresentation,
    ) -> Result<(), LowerError> {
        self.builder.append(
            block,
            Operation::Store {
                address,
                value,
                representation,
            },
        )?;
        Ok(())
    }

    pub(super) fn byte_offset_at(
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

    pub(super) fn address_at(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        storage: StorageId,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        if let Some(address) = self.entry_addresses.get(&storage) {
            return Ok(*address);
        }
        let object = self.objects[storage.index()].ok_or(PlanError::InvalidDomain)?;
        Ok(self
            .builder
            .append(block, Operation::ObjectAddress(object))?[0])
    }
}

fn state_representation() -> MemoryRepresentation {
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
