//! Private sparse state for one isolated callable rewrite transaction.

mod guards;
mod logical;
mod order;
mod slots;

use crate::identity::CallableId;

use super::super::{
    BlockId, MirBasicBlock, MirBody, MirLogicalExpression, MirPathCondition, MirStorage, MirValue,
    OptionalGuardId, PathConditionId, StorageId, ValueId,
};
use super::error::MirRewriteError;
use guards::OptionalGuardRegistry;
use logical::{LogicalRecordIndex, LogicalRecords};
use order::{LiveOrder, OrderPlacement};
use slots::SparseSlots;

pub(super) type BlockPlacement = OrderPlacement<BlockId>;

/// Owned sparse common state for one callable under transformation.
///
/// This type deliberately cannot be converted to [`MirBody`] or passed to a
/// verifier. Dense reconstruction is a separate atomic operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MirCallableEdit {
    callable: CallableId,
    entry: BlockId,
    storage: SparseSlots<StorageId, MirStorage>,
    values: SparseSlots<ValueId, MirValue>,
    blocks: SparseSlots<BlockId, MirBasicBlock>,
    block_order: LiveOrder<BlockId>,
    path_conditions: SparseSlots<PathConditionId, MirPathCondition>,
    logical_expressions: LogicalRecords<MirLogicalExpression>,
    optional_guards: OptionalGuardRegistry,
}

impl MirCallableEdit {
    pub(super) fn from_dense_parts(
        callable: CallableId,
        storage: Vec<MirStorage>,
        values: Vec<MirValue>,
        mut body: MirBody,
    ) -> Result<Self, MirRewriteError> {
        let optional_guards = OptionalGuardRegistry::discover(callable, &mut body)?;
        let MirBody {
            entry,
            blocks,
            path_conditions,
            logical_expressions,
        } = body;

        let block_order = blocks.iter().map(|block| block.id).collect();
        let storage = SparseSlots::from_dense(callable, storage)?;
        let values = SparseSlots::from_dense(callable, values)?;
        let blocks = SparseSlots::from_dense(callable, blocks)?;
        blocks.get(entry)?;
        let block_order = LiveOrder::complete(callable, blocks.live_ids(), block_order)?;
        let path_conditions = SparseSlots::from_dense(callable, path_conditions)?;
        validate_path_parents(&path_conditions)?;

        Ok(Self {
            callable,
            entry,
            storage,
            values,
            blocks,
            block_order,
            path_conditions,
            logical_expressions: LogicalRecords::from_dense(logical_expressions),
            optional_guards,
        })
    }

    pub(super) const fn callable(&self) -> CallableId {
        self.callable
    }

    pub(super) const fn entry(&self) -> BlockId {
        self.entry
    }

    pub(super) fn storage(&self, identity: StorageId) -> Result<&MirStorage, MirRewriteError> {
        self.storage.get(identity)
    }

    pub(super) fn storage_ids(&self) -> impl Iterator<Item = StorageId> + '_ {
        self.storage.live_ids()
    }

    pub(super) fn allocate_storage(
        &mut self,
        build: impl FnOnce(StorageId) -> MirStorage,
    ) -> Result<StorageId, MirRewriteError> {
        self.storage.allocate_with(build)
    }

    pub(super) fn remove_storage(
        &mut self,
        identity: StorageId,
    ) -> Result<MirStorage, MirRewriteError> {
        self.storage.remove(identity)
    }

    pub(super) fn value(&self, identity: ValueId) -> Result<&MirValue, MirRewriteError> {
        self.values.get(identity)
    }

    pub(super) fn value_ids(&self) -> impl Iterator<Item = ValueId> + '_ {
        self.values.live_ids()
    }

    pub(super) fn allocate_value(
        &mut self,
        build: impl FnOnce(ValueId) -> MirValue,
    ) -> Result<ValueId, MirRewriteError> {
        self.values.allocate_with(build)
    }

    pub(super) fn remove_value(&mut self, identity: ValueId) -> Result<MirValue, MirRewriteError> {
        self.values.remove(identity)
    }

    pub(super) fn block(&self, identity: BlockId) -> Result<&MirBasicBlock, MirRewriteError> {
        self.blocks.get(identity)
    }

    pub(super) fn block_order(&self) -> &[BlockId] {
        self.block_order.entries()
    }

    pub(super) fn allocate_block(
        &mut self,
        placement: BlockPlacement,
        build: impl FnOnce(BlockId) -> MirBasicBlock,
    ) -> Result<BlockId, MirRewriteError> {
        validate_block_placement(&self.blocks, placement)?;
        let identity = self.blocks.next_id();
        let block = build(identity);
        self.blocks.validate_next(&block)?;
        self.block_order.insert(identity, placement)?;
        self.blocks.append_prevalidated(identity, block);
        Ok(identity)
    }

    pub(super) fn remove_block(
        &mut self,
        identity: BlockId,
    ) -> Result<MirBasicBlock, MirRewriteError> {
        self.blocks.get(identity)?;
        if !self.block_order.contains(identity) {
            return Err(MirRewriteError::MissingOrderIdentity {
                identity: super::MirLocalIdentity::Block(identity),
            });
        }
        self.block_order.remove(identity)?;
        self.blocks.remove(identity)
    }

    pub(super) fn path_condition(
        &self,
        identity: PathConditionId,
    ) -> Result<&MirPathCondition, MirRewriteError> {
        self.path_conditions.get(identity)
    }

    pub(super) fn path_condition_ids(&self) -> impl Iterator<Item = PathConditionId> + '_ {
        self.path_conditions.live_ids()
    }

    pub(super) fn allocate_path_condition(
        &mut self,
        build: impl FnOnce(PathConditionId) -> MirPathCondition,
    ) -> Result<PathConditionId, MirRewriteError> {
        let identity = self.path_conditions.next_id();
        let condition = build(identity);
        self.path_conditions.validate_next(&condition)?;
        validate_path_parent(&self.path_conditions, &condition)?;
        self.path_conditions.append(condition)
    }

    pub(super) fn remove_path_condition(
        &mut self,
        identity: PathConditionId,
    ) -> Result<MirPathCondition, MirRewriteError> {
        self.path_conditions.remove(identity)
    }

    pub(super) fn logical_record(
        &self,
        index: LogicalRecordIndex,
    ) -> Result<&MirLogicalExpression, MirRewriteError> {
        self.logical_expressions.get(index)
    }

    pub(super) fn logical_order(&self) -> &[LogicalRecordIndex] {
        self.logical_expressions.order()
    }

    pub(super) fn allocate_logical_record(
        &mut self,
        expression: MirLogicalExpression,
    ) -> LogicalRecordIndex {
        self.logical_expressions.allocate(expression)
    }

    pub(super) fn remove_logical_record(
        &mut self,
        index: LogicalRecordIndex,
    ) -> Result<MirLogicalExpression, MirRewriteError> {
        self.logical_expressions.remove(index)
    }

    pub(super) fn optional_guard(&self, identity: OptionalGuardId) -> Result<(), MirRewriteError> {
        self.optional_guards.get(identity)
    }

    pub(super) fn optional_guard_ids(&self) -> impl Iterator<Item = OptionalGuardId> + '_ {
        self.optional_guards.live_ids()
    }

    pub(super) fn allocate_optional_guard(&mut self) -> OptionalGuardId {
        self.optional_guards.allocate()
    }

    pub(super) fn remove_optional_guard(
        &mut self,
        identity: OptionalGuardId,
    ) -> Result<(), MirRewriteError> {
        self.optional_guards.remove(identity)
    }
}

fn validate_block_placement(
    blocks: &SparseSlots<BlockId, MirBasicBlock>,
    placement: BlockPlacement,
) -> Result<(), MirRewriteError> {
    match placement {
        BlockPlacement::Append => Ok(()),
        BlockPlacement::Before(anchor) | BlockPlacement::After(anchor) => {
            blocks.get(anchor).map(|_| ())
        }
    }
}

fn validate_path_parents(
    conditions: &SparseSlots<PathConditionId, MirPathCondition>,
) -> Result<(), MirRewriteError> {
    for condition in conditions.live_entries() {
        validate_path_parent(conditions, condition)?;
    }
    Ok(())
}

fn validate_path_parent(
    conditions: &SparseSlots<PathConditionId, MirPathCondition>,
    condition: &MirPathCondition,
) -> Result<(), MirRewriteError> {
    let Some(parent) = condition.parent else {
        return Ok(());
    };
    if parent.callable() != conditions.owner() {
        conditions.get(parent)?;
    }
    if parent.index() >= condition.id.index() {
        return Err(MirRewriteError::PathParentNotEarlier {
            condition: condition.id,
            parent,
        });
    }
    conditions.get(parent).map(|_| ())
}

#[cfg(test)]
mod tests;
