//! Private sparse state for one isolated callable rewrite transaction.

mod guards;
mod logical;
mod operations;
mod order;
mod slots;

#[cfg(test)]
pub(super) mod test_support;

use crate::identity::CallableId;

use super::super::{
    BlockId, MirBasicBlock, MirBody, MirLogicalExpression, MirPathCondition, MirStorage, MirValue,
    OptionalGuardId, PathConditionId, StorageId, ValueId,
};
use super::error::MirRewriteError;
use super::MirLocalIdentitySite;
pub(in crate::mir::rewrite) use guards::collect_optional_guards;
use guards::OptionalGuardRegistry;
pub(crate) use logical::LogicalRecordIndex;
use logical::LogicalRecords;
use order::{LiveOrder, OrderPlacement};
use slots::SparseSlots;

pub(crate) type BlockPlacement = OrderPlacement<BlockId>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EditIdentityInventory<I> {
    pub(super) original_len: usize,
    pub(super) slots: Vec<Option<bool>>,
    pub(super) order: Vec<I>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EditRecordInventory {
    pub(super) original_len: usize,
    pub(super) live: Vec<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MirCallableEditInventory {
    pub(super) callable: CallableId,
    pub(super) storage: EditIdentityInventory<StorageId>,
    pub(super) values: EditIdentityInventory<ValueId>,
    pub(super) blocks: EditIdentityInventory<BlockId>,
    pub(super) path_conditions: EditIdentityInventory<PathConditionId>,
    pub(super) optional_guards: EditIdentityInventory<OptionalGuardId>,
    pub(super) logical_expressions: EditRecordInventory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MirCallableDenseCandidate {
    pub(super) callable: CallableId,
    pub(super) storage: Vec<MirStorage>,
    pub(super) values: Vec<MirValue>,
    pub(super) body: MirBody,
}

/// Owned sparse common state for one callable under transformation.
///
/// This type deliberately cannot be converted to [`MirBody`] or passed to a
/// verifier. Dense reconstruction is a separate atomic operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirCallableEdit {
    callable: CallableId,
    entry: BlockId,
    /// Callable-header block references which remain outside [`MirBody`].
    ///
    /// These are observations, not another mutable owner. Commit still maps
    /// the authoritative attachments held by `MirCallablePackage`.
    attachment_blocks: Vec<(MirLocalIdentitySite, BlockId)>,
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
        let block_order = LiveOrder::complete(callable, blocks.live_ids(), block_order)?;
        let path_conditions = SparseSlots::from_dense(callable, path_conditions)?;
        validate_path_parents(&path_conditions)?;

        Ok(Self {
            callable,
            entry,
            attachment_blocks: Vec::new(),
            storage,
            values,
            blocks,
            block_order,
            path_conditions,
            logical_expressions: LogicalRecords::from_dense(logical_expressions),
            optional_guards,
        })
    }

    pub(crate) const fn callable(&self) -> CallableId {
        self.callable
    }

    pub(crate) const fn entry(&self) -> BlockId {
        self.entry
    }

    pub(in crate::mir::rewrite) fn with_attachment_blocks(
        mut self,
        attachments: impl IntoIterator<Item = (MirLocalIdentitySite, BlockId)>,
    ) -> Self {
        self.attachment_blocks.extend(attachments);
        self
    }

    pub(crate) fn storage(&self, identity: StorageId) -> Result<&MirStorage, MirRewriteError> {
        self.storage.get(identity)
    }

    pub(crate) fn storage_ids(&self) -> impl Iterator<Item = StorageId> + '_ {
        self.storage.live_ids()
    }

    pub(crate) fn allocate_storage(
        &mut self,
        build: impl FnOnce(StorageId) -> MirStorage,
    ) -> Result<StorageId, MirRewriteError> {
        self.storage.allocate_with(build)
    }

    pub(crate) fn remove_storage(
        &mut self,
        identity: StorageId,
    ) -> Result<MirStorage, MirRewriteError> {
        self.storage.remove(identity)
    }

    pub(crate) fn value(&self, identity: ValueId) -> Result<&MirValue, MirRewriteError> {
        self.values.get(identity)
    }

    pub(crate) fn value_ids(&self) -> impl Iterator<Item = ValueId> + '_ {
        self.values.live_ids()
    }

    pub(in crate::mir::rewrite) fn allocated_value_slots(&self) -> usize {
        self.values.next_id().index()
    }

    pub(crate) fn allocate_value(
        &mut self,
        build: impl FnOnce(ValueId) -> MirValue,
    ) -> Result<ValueId, MirRewriteError> {
        self.values.allocate_with(build)
    }

    pub(crate) fn remove_value(&mut self, identity: ValueId) -> Result<MirValue, MirRewriteError> {
        self.values.remove(identity)
    }

    pub(crate) fn block(&self, identity: BlockId) -> Result<&MirBasicBlock, MirRewriteError> {
        self.blocks.get(identity)
    }

    pub(crate) fn block_order(&self) -> &[BlockId] {
        self.block_order.entries()
    }

    pub(in crate::mir::rewrite) fn block_ids(&self) -> impl Iterator<Item = BlockId> + '_ {
        self.blocks.live_ids()
    }

    pub(crate) fn allocate_block(
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

    pub(crate) fn remove_block(
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

    pub(crate) fn path_condition(
        &self,
        identity: PathConditionId,
    ) -> Result<&MirPathCondition, MirRewriteError> {
        self.path_conditions.get(identity)
    }

    pub(crate) fn path_condition_ids(&self) -> impl Iterator<Item = PathConditionId> + '_ {
        self.path_conditions.live_ids()
    }

    pub(crate) fn allocate_path_condition(
        &mut self,
        build: impl FnOnce(PathConditionId) -> MirPathCondition,
    ) -> Result<PathConditionId, MirRewriteError> {
        let identity = self.path_conditions.next_id();
        let condition = build(identity);
        self.path_conditions.validate_next(&condition)?;
        validate_path_parent(&self.path_conditions, &condition)?;
        self.path_conditions.append(condition)
    }

    pub(crate) fn remove_path_condition(
        &mut self,
        identity: PathConditionId,
    ) -> Result<MirPathCondition, MirRewriteError> {
        self.path_conditions.remove(identity)
    }

    pub(in crate::mir::rewrite) fn replace_imported_path_condition(
        &mut self,
        identity: PathConditionId,
        condition: MirPathCondition,
    ) -> Result<(), MirRewriteError> {
        debug_assert_eq!(condition.id, identity);
        *self.path_conditions.get_mut(identity)? = condition;
        Ok(())
    }

    pub(crate) fn logical_record(
        &self,
        index: LogicalRecordIndex,
    ) -> Result<&MirLogicalExpression, MirRewriteError> {
        self.logical_expressions.get(index)
    }

    pub(crate) fn logical_order(&self) -> &[LogicalRecordIndex] {
        self.logical_expressions.order()
    }

    pub(crate) fn allocate_logical_record(
        &mut self,
        expression: MirLogicalExpression,
    ) -> LogicalRecordIndex {
        self.logical_expressions.allocate(expression)
    }

    pub(crate) fn remove_logical_record(
        &mut self,
        index: LogicalRecordIndex,
    ) -> Result<MirLogicalExpression, MirRewriteError> {
        self.logical_expressions.remove(index)
    }

    pub(crate) fn optional_guard(&self, identity: OptionalGuardId) -> Result<(), MirRewriteError> {
        self.optional_guards.get(identity)
    }

    pub(crate) fn optional_guard_ids(&self) -> impl Iterator<Item = OptionalGuardId> + '_ {
        self.optional_guards.live_ids()
    }

    pub(crate) fn allocate_optional_guard(&mut self) -> OptionalGuardId {
        self.optional_guards.allocate()
    }

    pub(crate) fn remove_optional_guard(
        &mut self,
        identity: OptionalGuardId,
    ) -> Result<(), MirRewriteError> {
        self.optional_guards.remove(identity)
    }

    pub(super) fn commit_inventory(&self) -> Result<MirCallableEditInventory, MirRewriteError> {
        self.block_order.validate_complete(self.blocks.live_ids())?;
        self.logical_expressions.validate_order()?;
        Ok(MirCallableEditInventory {
            callable: self.callable,
            storage: slot_inventory(&self.storage, self.storage.live_ids().collect()),
            values: slot_inventory(&self.values, self.values.live_ids().collect()),
            blocks: slot_inventory(&self.blocks, self.block_order.entries().to_vec()),
            path_conditions: slot_inventory(
                &self.path_conditions,
                self.path_conditions.live_ids().collect(),
            ),
            optional_guards: EditIdentityInventory {
                original_len: self.optional_guards.original_len(),
                slots: self.optional_guards.slot_liveness().collect(),
                order: self.optional_guards.live_ids().collect(),
            },
            logical_expressions: EditRecordInventory {
                original_len: self.logical_expressions.original_len(),
                live: self.logical_expressions.slot_liveness().collect(),
            },
        })
    }

    pub(super) fn into_dense_candidate(self) -> Result<MirCallableDenseCandidate, MirRewriteError> {
        let Self {
            callable,
            entry,
            attachment_blocks: _,
            storage,
            values,
            blocks,
            block_order,
            path_conditions,
            logical_expressions,
            optional_guards: _,
        } = self;
        let blocks = blocks.into_explicit_order(block_order.entries())?;
        let logical_expressions = logical_expressions.into_explicit_order()?;
        Ok(MirCallableDenseCandidate {
            callable,
            storage: storage.into_slot_order(),
            values: values.into_slot_order(),
            body: MirBody {
                entry,
                blocks,
                path_conditions: path_conditions.into_slot_order(),
                logical_expressions,
            },
        })
    }

    #[cfg(test)]
    pub(super) fn replace_block_order_for_test(&mut self, entries: Vec<BlockId>) {
        self.block_order = LiveOrder::unchecked_for_test(self.callable, entries);
    }

    #[cfg(test)]
    pub(super) fn replace_logical_order_for_test(&mut self, entries: Vec<LogicalRecordIndex>) {
        self.logical_expressions.replace_order_for_test(entries);
    }

    #[cfg(test)]
    pub(super) fn forget_optional_guard_for_test(&mut self, identity: OptionalGuardId) {
        self.optional_guards.forget_for_test(identity);
    }
}

fn slot_inventory<I, T>(slots: &SparseSlots<I, T>, order: Vec<I>) -> EditIdentityInventory<I>
where
    I: super::identity::MirLocalId,
    T: slots::EditSlotEntry<I>,
{
    EditIdentityInventory {
        original_len: slots.original_len(),
        slots: slots.slot_liveness().map(Some).collect(),
        order,
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
