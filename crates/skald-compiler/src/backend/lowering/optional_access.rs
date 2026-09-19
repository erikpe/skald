//! Checked optional payload views and per-layer optional-box guards.

use super::{context::Lowerer, LowerError};
use crate::{
    backend::{
        lir::{Constant, MemoryRepresentation, Operation, Terminator},
        plan::{PlanError, ScalarType},
    },
    mir::{
        BlockId, MirOptionalBoxViewBegin, MirOptionalBoxViewEnd, MirOptionalViewBegin,
        MirOptionalViewEnd, MirPresenceTestKind, MirTerminator, StorageId,
    },
    primitive_comparison::PrimitiveComparisonPredicate,
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn optional_box_presence(
        &mut self,
        block: BlockId,
        owner: StorageId,
        target: crate::identity::OptionalBoxTypeId,
        layer: usize,
        kind: MirPresenceTestKind,
    ) -> Result<
        Operation<
            crate::backend::lir::ValueHandle<'plan>,
            crate::backend::lir::ObjectHandle<'plan>,
            crate::backend::lir::BlockHandle<'plan>,
        >,
        LowerError,
    > {
        let state = self.load_optional_box_state(block, owner, target, layer)?;
        let zero = self.builder.append(
            self.active_blocks[block.index()],
            Operation::Constant(Constant::U64(0)),
        )?[0];
        Ok(Operation::Compare {
            predicate: match kind {
                MirPresenceTestKind::Some => PrimitiveComparisonPredicate::NotEqual,
                MirPresenceTestKind::None => PrimitiveComparisonPredicate::Equal,
            },
            left: state,
            right: zero,
        })
    }

    pub(super) fn optional_access_terminator(
        &mut self,
        block: BlockId,
        terminator: &MirTerminator,
    ) -> Result<bool, LowerError> {
        match terminator {
            MirTerminator::BeginOptionalView {
                begin,
                success_target,
                absent_target,
                overflow_target,
                ..
            } => {
                self.begin_optional_view(
                    block,
                    begin,
                    *success_target,
                    *absent_target,
                    *overflow_target,
                )?;
                Ok(true)
            }
            MirTerminator::BeginOptionalBoxView {
                begin,
                success_target,
                absent_target,
                overflow_target,
                ..
            } => {
                self.begin_optional_box_view(
                    block,
                    begin,
                    *success_target,
                    *absent_target,
                    *overflow_target,
                )?;
                Ok(true)
            }
            MirTerminator::CheckOptionalMutation {
                source,
                success_target,
                failure_target,
                ..
            } => {
                let optional = self.optional_fact(source)?.optional;
                let state = self.load_optional_state(block, source, optional)?;
                let zero = self.constant_u64(block, 0)?;
                let one = self.constant_u64(block, 1)?;
                let absent =
                    self.compare(block, PrimitiveComparisonPredicate::Equal, state, zero)?;
                let test_one = self.new_optional_block()?;
                self.builder.terminate(
                    self.active_blocks[block.index()],
                    Terminator::Branch {
                        condition: absent,
                        true_edge: self.edge(*success_target),
                        false_edge: edge(test_one),
                    },
                )?;
                let single = self.builder.append(
                    test_one,
                    Operation::Compare {
                        predicate: PrimitiveComparisonPredicate::Equal,
                        left: state,
                        right: one,
                    },
                )?[0];
                self.builder.terminate(
                    test_one,
                    Terminator::Branch {
                        condition: single,
                        true_edge: self.edge(*success_target),
                        false_edge: self.edge(*failure_target),
                    },
                )?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub(super) fn end_optional_view(
        &mut self,
        block: BlockId,
        end: &MirOptionalViewEnd,
    ) -> Result<(), LowerError> {
        let state = self.load_optional_state(block, &end.source, end.optional)?;
        let one = self.constant_u64(block, 1)?;
        let next = self.builder.append(
            self.active_blocks[block.index()],
            Operation::Binary {
                operation: crate::backend::lir::BinaryOperation::Subtract,
                left: state,
                right: one,
            },
        )?[0];
        self.store_state_value(block, &end.source, next)
    }

    pub(super) fn end_optional_box_view(
        &mut self,
        block: BlockId,
        end: &MirOptionalBoxViewEnd,
    ) -> Result<(), LowerError> {
        let state = self.load_optional_box_state(block, end.owner, end.box_target, end.layer)?;
        let one = self.constant_u64(block, 1)?;
        let next = self.builder.append(
            self.active_blocks[block.index()],
            Operation::Binary {
                operation: crate::backend::lir::BinaryOperation::Subtract,
                left: state,
                right: one,
            },
        )?[0];
        self.store_optional_box_state(block, end.owner, end.box_target, end.layer, next)
    }

    fn begin_optional_view(
        &mut self,
        block: BlockId,
        begin: &MirOptionalViewBegin,
        success: BlockId,
        absent: BlockId,
        overflow: BlockId,
    ) -> Result<(), LowerError> {
        let state = self.load_optional_state(block, &begin.source, begin.optional)?;
        self.begin_guard(block, state, success, absent, overflow, |lowerer, value| {
            lowerer.store_state_value(block, &begin.source, value)
        })
    }

    fn begin_optional_box_view(
        &mut self,
        block: BlockId,
        begin: &MirOptionalBoxViewBegin,
        success: BlockId,
        absent: BlockId,
        overflow: BlockId,
    ) -> Result<(), LowerError> {
        let state =
            self.load_optional_box_state(block, begin.owner, begin.box_target, begin.layer)?;
        self.begin_guard(block, state, success, absent, overflow, |lowerer, value| {
            lowerer.store_optional_box_state(
                block,
                begin.owner,
                begin.box_target,
                begin.layer,
                value,
            )
        })
    }

    fn begin_guard<F>(
        &mut self,
        block: BlockId,
        state: crate::backend::lir::ValueHandle<'plan>,
        success: BlockId,
        absent: BlockId,
        overflow: BlockId,
        store: F,
    ) -> Result<(), LowerError>
    where
        F: FnOnce(&mut Self, crate::backend::lir::ValueHandle<'plan>) -> Result<(), LowerError>,
    {
        let zero = self.constant_u64(block, 0)?;
        let is_absent = self.compare(block, PrimitiveComparisonPredicate::Equal, state, zero)?;
        let check_overflow = self.new_optional_block()?;
        self.builder.terminate(
            self.active_blocks[block.index()],
            Terminator::Branch {
                condition: is_absent,
                true_edge: self.edge(absent),
                false_edge: edge(check_overflow),
            },
        )?;
        self.active_blocks[block.index()] = check_overflow;
        let maximum = self.constant_u64(block, u64::MAX)?;
        let is_maximum =
            self.compare(block, PrimitiveComparisonPredicate::Equal, state, maximum)?;
        let increment = self.new_optional_block()?;
        self.builder.terminate(
            check_overflow,
            Terminator::Branch {
                condition: is_maximum,
                true_edge: self.edge(overflow),
                false_edge: edge(increment),
            },
        )?;
        self.active_blocks[block.index()] = increment;
        let one = self.constant_u64(block, 1)?;
        let next = self.builder.append(
            increment,
            Operation::Binary {
                operation: crate::backend::lir::BinaryOperation::Add,
                left: state,
                right: one,
            },
        )?[0];
        store(self, next)?;
        self.builder.terminate(
            self.active_blocks[block.index()],
            Terminator::Jump(self.edge(success)),
        )?;
        Ok(())
    }

    fn optional_box_state_address(
        &mut self,
        block: BlockId,
        owner: StorageId,
        target: crate::identity::OptionalBoxTypeId,
        layer: usize,
    ) -> Result<crate::backend::lir::ValueHandle<'plan>, LowerError> {
        let fact = self
            .plan()
            .semantic()
            .optional_box(target)
            .cloned()
            .ok_or(PlanError::UnknownDeclaration)?;
        let layer = *fact
            .layer_offsets
            .get(layer)
            .ok_or(PlanError::InvalidLayout)?;
        let handle = self.load_storage(block, owner)?;
        let header = self
            .plan()
            .semantic()
            .shared_header
            .ok_or(PlanError::InvalidLayout)?;
        self.byte_offset(
            block,
            handle,
            header
                .header_size
                .checked_add(layer)
                .ok_or(PlanError::SizeOverflow)?,
        )
    }

    fn load_optional_box_state(
        &mut self,
        block: BlockId,
        owner: StorageId,
        target: crate::identity::OptionalBoxTypeId,
        layer: usize,
    ) -> Result<crate::backend::lir::ValueHandle<'plan>, LowerError> {
        let address = self.optional_box_state_address(block, owner, target, layer)?;
        self.load_optional_at(block, address, state_representation())
    }

    fn store_optional_box_state(
        &mut self,
        block: BlockId,
        owner: StorageId,
        target: crate::identity::OptionalBoxTypeId,
        layer: usize,
        value: crate::backend::lir::ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        let address = self.optional_box_state_address(block, owner, target, layer)?;
        self.store_optional_at(block, address, value, state_representation())
    }

    fn store_state_value(
        &mut self,
        block: BlockId,
        place: &crate::mir::MirPlace,
        value: crate::backend::lir::ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        let fact = self.optional_fact(place)?;
        let address = self.optional_state_address(block, place, &fact)?;
        self.store_optional_at(block, address, value, state_representation())
    }

    fn constant_u64(
        &mut self,
        block: BlockId,
        value: u64,
    ) -> Result<crate::backend::lir::ValueHandle<'plan>, LowerError> {
        Ok(self.builder.append(
            self.active_blocks[block.index()],
            Operation::Constant(Constant::U64(value)),
        )?[0])
    }
    fn compare(
        &mut self,
        block: BlockId,
        predicate: PrimitiveComparisonPredicate,
        left: crate::backend::lir::ValueHandle<'plan>,
        right: crate::backend::lir::ValueHandle<'plan>,
    ) -> Result<crate::backend::lir::ValueHandle<'plan>, LowerError> {
        Ok(self.builder.append(
            self.active_blocks[block.index()],
            Operation::Compare {
                predicate,
                left,
                right,
            },
        )?[0])
    }
}

fn state_representation() -> MemoryRepresentation {
    MemoryRepresentation {
        scalar: ScalarType::U64,
        bytes: 8,
        alignment: 8,
    }
}

fn edge<'p>(
    target: crate::backend::lir::BlockHandle<'p>,
) -> crate::backend::lir::Edge<
    crate::backend::lir::ValueHandle<'p>,
    crate::backend::lir::BlockHandle<'p>,
> {
    crate::backend::lir::Edge {
        target,
        arguments: vec![],
    }
}
