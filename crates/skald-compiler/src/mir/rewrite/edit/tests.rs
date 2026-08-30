use crate::{
    identity::{CallableId, FunctionId, OptionalTypeId},
    test_support::lower_source_to_mir,
};

use super::{
    logical::LogicalRecordIndex,
    order::{LiveOrder, OrderPlacement},
    *,
};
use crate::mir::{
    MirBasicBlock, MirBody, MirInstruction, MirLogicalExpression, MirLogicalOperation,
    MirOptionalViewBegin, MirOptionalViewEnd, MirPathCondition, MirPlace, MirStorage,
    MirStorageKind, MirTerminator, MirType, MirValue,
};

fn fixture_parts() -> (CallableId, Vec<MirStorage>, Vec<MirValue>, MirBody) {
    let program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let definition = program
        .definitions
        .get(program.entry_function)
        .expect("entry definition");
    let callable = definition.callable();
    let span = definition.span;
    let storage_id = |index| StorageId::new(callable, index);
    let value_id = |index| ValueId::new(callable, index);
    let block_id = |index| BlockId::new(callable, index);
    let path_id = |index| PathConditionId::new(callable, index);
    let guard = OptionalGuardId::new(callable, 2);
    let optional = OptionalTypeId::new(0);

    let storage = (0..3)
        .map(|index| MirStorage {
            id: storage_id(index),
            source: None,
            name: format!("storage{index}"),
            kind: MirStorageKind::Temporary,
            ty: MirType::I64,
            span,
        })
        .collect();
    let values = (0..3)
        .map(|index| MirValue {
            id: value_id(index),
            ty: MirType::I64,
            span,
        })
        .collect();
    let logical = |selected_result| MirLogicalExpression {
        operation: MirLogicalOperation::And,
        condition: path_id(0),
        result: storage_id(0),
        left_result: value_id(0),
        split: block_id(0),
        selection: block_id(0),
        right_entry: block_id(1),
        right_exit: block_id(1),
        right_result: value_id(1),
        short: block_id(1),
        join: block_id(1),
        selected_result,
        span,
    };
    let body = MirBody {
        entry: block_id(0),
        blocks: vec![
            MirBasicBlock {
                id: block_id(0),
                instructions: vec![],
                terminator: Some(MirTerminator::BeginOptionalView {
                    begin: MirOptionalViewBegin {
                        optional,
                        guard,
                        source: MirPlace::base(storage_id(0)),
                        payload: MirType::I64,
                        span,
                    },
                    success_target: block_id(1),
                    absent_target: block_id(1),
                    overflow_target: block_id(1),
                    span,
                }),
                span,
            },
            MirBasicBlock {
                id: block_id(1),
                instructions: vec![MirInstruction::EndOptionalView(MirOptionalViewEnd {
                    optional,
                    guard,
                    source: MirPlace::base(storage_id(0)),
                    payload: MirType::I64,
                    span,
                })],
                terminator: Some(MirTerminator::Return {
                    value: Some(value_id(0)),
                    span,
                }),
                span,
            },
        ],
        path_conditions: vec![
            MirPathCondition {
                id: path_id(0),
                parent: None,
                activation: storage_id(1),
                active_predecessor: block_id(0),
                inactive_predecessor: block_id(0),
                merge: block_id(1),
                span,
            },
            MirPathCondition {
                id: path_id(1),
                parent: Some(path_id(0)),
                activation: storage_id(2),
                active_predecessor: block_id(0),
                inactive_predecessor: block_id(0),
                merge: block_id(1),
                span,
            },
        ],
        logical_expressions: vec![logical(value_id(1)), logical(value_id(2))],
    };
    (callable, storage, values, body)
}

fn edit() -> MirCallableEdit {
    let (callable, storage, values, body) = fixture_parts();
    MirCallableEdit::from_dense_parts(callable, storage, values, body)
        .expect("fixture opens as sparse callable state")
}

fn empty_block(identity: BlockId, span: crate::source::Span) -> MirBasicBlock {
    MirBasicBlock {
        id: identity,
        instructions: vec![],
        terminator: Some(MirTerminator::Return { value: None, span }),
        span,
    }
}

#[test]
fn sparse_slots_preserve_survivors_and_never_reuse_deleted_indices() {
    let mut edit = edit();
    let callable = edit.callable();
    let span = edit.storage(StorageId::new(callable, 0)).unwrap().span;

    let removed = edit.remove_storage(StorageId::new(callable, 0)).unwrap();
    assert_eq!(removed.id.index(), 0);
    assert_eq!(
        edit.remove_storage(StorageId::new(callable, 0)),
        Err(MirRewriteError::DeletedIdentity {
            identity: super::super::MirLocalIdentity::Storage(StorageId::new(callable, 0)),
        })
    );
    assert_eq!(
        edit.storage(StorageId::new(callable, 1))
            .unwrap()
            .id
            .index(),
        1
    );
    let allocated = edit
        .allocate_storage(|id| MirStorage {
            id,
            source: None,
            name: "new".into(),
            kind: MirStorageKind::Temporary,
            ty: MirType::I64,
            span,
        })
        .unwrap();
    assert_eq!(allocated.index(), 3);
    assert_eq!(
        edit.storage_ids().map(StorageId::index).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );

    edit.remove_value(ValueId::new(callable, 0)).unwrap();
    let new_value = edit
        .allocate_value(|id| MirValue {
            id,
            ty: MirType::I64,
            span,
        })
        .unwrap();
    assert_eq!(new_value.index(), 3);
    assert_eq!(edit.value(ValueId::new(callable, 1)).unwrap().id.index(), 1);

    edit.remove_path_condition(PathConditionId::new(callable, 0))
        .unwrap();
    let new_path = edit
        .allocate_path_condition(|id| MirPathCondition {
            id,
            parent: Some(PathConditionId::new(callable, 1)),
            activation: StorageId::new(callable, 1),
            active_predecessor: BlockId::new(callable, 0),
            inactive_predecessor: BlockId::new(callable, 0),
            merge: BlockId::new(callable, 1),
            span,
        })
        .unwrap();
    assert_eq!(new_path.index(), 2);
}

#[test]
fn sparse_slots_reject_foreign_lookups_and_mismatched_new_declarations() {
    let mut edit = edit();
    let callable = edit.callable();
    let foreign = CallableId::Function(FunctionId::new(99));
    let foreign_storage = StorageId::new(foreign, 0);
    assert_eq!(
        edit.storage(foreign_storage),
        Err(MirRewriteError::ForeignIdentity {
            expected: callable,
            identity: super::super::MirLocalIdentity::Storage(foreign_storage),
        })
    );

    let span = edit.storage(StorageId::new(callable, 0)).unwrap().span;
    let error = edit
        .allocate_storage(|expected| MirStorage {
            id: StorageId::new(callable, expected.index() + 1),
            source: None,
            name: "wrong".into(),
            kind: MirStorageKind::Temporary,
            ty: MirType::I64,
            span,
        })
        .unwrap_err();
    assert!(matches!(
        error,
        MirRewriteError::DeclarationIdentityMismatch { .. }
    ));
    let allocated = edit
        .allocate_storage(|id| MirStorage {
            id,
            source: None,
            name: "right".into(),
            kind: MirStorageKind::Temporary,
            ty: MirType::I64,
            span,
        })
        .unwrap();
    assert_eq!(allocated.index(), 3);
}

#[test]
fn block_order_is_explicit_and_independent_from_allocation_order() {
    let mut edit = edit();
    let callable = edit.callable();
    let first = BlockId::new(callable, 0);
    let second = BlockId::new(callable, 1);
    let span = edit.block(first).unwrap().span;

    let before = edit
        .allocate_block(BlockPlacement::Before(second), |id| empty_block(id, span))
        .unwrap();
    let after = edit
        .allocate_block(BlockPlacement::After(first), |id| empty_block(id, span))
        .unwrap();
    let appended = edit
        .allocate_block(BlockPlacement::Append, |id| empty_block(id, span))
        .unwrap();
    assert_eq!(before.index(), 2);
    assert_eq!(after.index(), 3);
    assert_eq!(appended.index(), 4);
    assert_eq!(
        edit.block_order(),
        &[first, after, before, second, appended]
    );

    edit.remove_block(before).unwrap();
    assert_eq!(edit.block_order(), &[first, after, second, appended]);
    assert_eq!(
        edit.block(before),
        Err(MirRewriteError::DeletedIdentity {
            identity: super::super::MirLocalIdentity::Block(before),
        })
    );
}

#[test]
fn block_order_validation_rejects_duplicates_missing_entries_and_foreign_owners() {
    let callable = CallableId::Function(FunctionId::new(0));
    let first = BlockId::new(callable, 0);
    let second = BlockId::new(callable, 1);
    assert!(matches!(
        LiveOrder::complete(callable, [first, second], vec![first, first, second]),
        Err(MirRewriteError::DuplicateOrderIdentity { .. })
    ));
    assert_eq!(
        LiveOrder::complete(callable, [first, second], vec![first]),
        Err(MirRewriteError::MissingOrderIdentity {
            identity: super::super::MirLocalIdentity::Block(second),
        })
    );
    let foreign = BlockId::new(FunctionId::new(1), 0);
    assert!(matches!(
        LiveOrder::complete(callable, [first], vec![foreign]),
        Err(MirRewriteError::ForeignIdentity { .. })
    ));
}

#[test]
fn path_creation_requires_an_existing_earlier_parent() {
    let mut edit = edit();
    let callable = edit.callable();
    let span = edit
        .path_condition(PathConditionId::new(callable, 0))
        .unwrap()
        .span;
    let build = |id, parent| MirPathCondition {
        id,
        parent: Some(parent),
        activation: StorageId::new(callable, 0),
        active_predecessor: BlockId::new(callable, 0),
        inactive_predecessor: BlockId::new(callable, 0),
        merge: BlockId::new(callable, 1),
        span,
    };

    let child = edit
        .allocate_path_condition(|id| build(id, PathConditionId::new(callable, 1)))
        .unwrap();
    assert_eq!(child.index(), 2);
    let error = edit
        .allocate_path_condition(|id| build(id, id))
        .unwrap_err();
    assert_eq!(
        error,
        MirRewriteError::PathParentNotEarlier {
            condition: PathConditionId::new(callable, 3),
            parent: PathConditionId::new(callable, 3),
        }
    );
    assert_eq!(edit.path_condition_ids().count(), 3);
}

#[test]
fn guard_discovery_tombstones_and_allocates_without_filling_old_holes() {
    let mut edit = edit();
    let callable = edit.callable();
    let discovered: Vec<_> = edit.optional_guard_ids().collect();
    assert_eq!(discovered, vec![OptionalGuardId::new(callable, 2)]);
    assert!(matches!(
        edit.optional_guard(OptionalGuardId::new(callable, 0)),
        Err(MirRewriteError::UnknownIdentity { .. })
    ));

    let allocated = edit.allocate_optional_guard();
    assert_eq!(allocated.index(), 3);
    edit.remove_optional_guard(discovered[0]).unwrap();
    assert!(matches!(
        edit.remove_optional_guard(discovered[0]),
        Err(MirRewriteError::DeletedIdentity { .. })
    ));
    assert_eq!(
        edit.optional_guard_ids().collect::<Vec<_>>(),
        vec![allocated]
    );
}

#[test]
fn logical_records_keep_explicit_relative_order_across_deletion_and_allocation() {
    let mut edit = edit();
    let first = LogicalRecordIndex::new(0);
    let second = LogicalRecordIndex::new(1);
    let replacement = edit.logical_record(first).unwrap().clone();
    edit.remove_logical_record(first).unwrap();
    let allocated = edit.allocate_logical_record(replacement);
    assert_eq!(allocated.index(), 2);
    assert_eq!(edit.logical_order(), &[second, allocated]);
    assert!(matches!(
        edit.logical_record(first),
        Err(MirRewriteError::DeletedLogicalRecord { index: 0 })
    ));
}

#[test]
fn equivalent_edit_sequences_produce_identical_private_state() {
    fn apply(mut edit: MirCallableEdit) -> MirCallableEdit {
        let callable = edit.callable();
        let first = BlockId::new(callable, 0);
        let span = edit.block(first).unwrap().span;
        edit.remove_value(ValueId::new(callable, 1)).unwrap();
        edit.allocate_block(OrderPlacement::After(first), |id| empty_block(id, span))
            .unwrap();
        edit.remove_optional_guard(OptionalGuardId::new(callable, 2))
            .unwrap();
        edit.allocate_optional_guard();
        edit
    }

    assert_eq!(apply(edit()), apply(edit()));
}

#[test]
fn block_instructions_remain_block_owned_without_a_persistent_identity() {
    let edit = edit();
    let block = edit.block(BlockId::new(edit.callable(), 1)).unwrap();
    assert_eq!(block.instructions.len(), 1);
    assert!(matches!(
        block.instructions[0],
        MirInstruction::EndOptionalView(_)
    ));
    assert_eq!(edit.entry(), BlockId::new(edit.callable(), 0));
}
