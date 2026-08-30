use crate::{
    identity::{CallableId, FunctionId},
    mir::{
        dump_mir, BlockId, MirBasicBlock, MirBody, MirPathCondition, MirPlaceBase, MirStorage,
        MirStorageKind, MirStorageLive, MirTerminator, MirType, MirValue, OptionalGuardId,
        PathConditionId, StorageId, ValueId,
    },
    test_support::lower_source_to_mir,
};

use super::*;
use crate::mir::rewrite::{
    edit::{
        test_support::{edit, fixture_parts},
        BlockPlacement, LogicalRecordIndex, MirCallableEdit,
    },
    error::{MirReferenceFailure, MirRewriteError},
    MirLocalIdentity, MirLocalIdentitySite,
};

#[test]
fn no_op_commit_preserves_common_state_and_exact_dump() {
    let mut original = lower_source_to_mir("fn main() -> i64 { return 7; }");
    let function = original.entry_function;
    let definition = original
        .definitions
        .get(function)
        .expect("entry definition")
        .clone();
    let expected_storage = definition.storage.clone();
    let expected_values = definition.values.clone();
    let expected_body = definition.body.clone();
    let callable = definition.callable();
    let expected_dump = dump_mir(&original);

    let result = commit(
        MirCallableEdit::from_dense_parts(
            callable,
            definition.storage,
            definition.values,
            definition.body,
        )
        .expect("verified common state opens"),
    )
    .expect("no-op common state commits");

    assert_eq!(result.callable.storage, expected_storage);
    assert_eq!(result.callable.values, expected_values);
    assert_eq!(result.callable.body, expected_body);
    assert_eq!(result.callable.callable, callable);
    let rebuilt = original
        .definitions
        .get_mut_for_test(function)
        .expect("entry definition remains present");
    rebuilt.storage = result.callable.storage;
    rebuilt.values = result.callable.values;
    rebuilt.body = result.callable.body;
    assert_eq!(dump_mir(&original), expected_dump);
}

#[test]
fn gaps_compact_in_each_canonical_order_and_report_known_changes() {
    let mut edit = gap_edit();
    let callable = edit.callable();
    let span = edit.storage(StorageId::new(callable, 0)).unwrap().span;

    edit.remove_storage(StorageId::new(callable, 1)).unwrap();
    let new_storage = edit
        .allocate_storage(|id| MirStorage {
            id,
            source: None,
            name: "new storage".into(),
            kind: MirStorageKind::Temporary,
            ty: MirType::I64,
            span,
        })
        .unwrap();
    edit.remove_value(ValueId::new(callable, 1)).unwrap();
    let new_value = edit
        .allocate_value(|id| MirValue {
            id,
            ty: MirType::I64,
            span,
        })
        .unwrap();
    edit.remove_block(BlockId::new(callable, 1)).unwrap();
    let new_block = edit
        .allocate_block(BlockPlacement::Before(BlockId::new(callable, 2)), |id| {
            empty_return_block(id, span)
        })
        .unwrap();
    edit.remove_path_condition(PathConditionId::new(callable, 1))
        .unwrap();
    let new_path = edit
        .allocate_path_condition(|id| MirPathCondition {
            id,
            parent: Some(PathConditionId::new(callable, 2)),
            activation: StorageId::new(callable, 2),
            active_predecessor: BlockId::new(callable, 0),
            inactive_predecessor: BlockId::new(callable, 0),
            merge: BlockId::new(callable, 2),
            span,
        })
        .unwrap();
    let first_logical = edit
        .logical_record(LogicalRecordIndex::new(0))
        .unwrap()
        .clone();
    edit.remove_logical_record(LogicalRecordIndex::new(1))
        .unwrap();
    edit.allocate_logical_record(first_logical);
    let deleted_guard = edit.allocate_optional_guard();
    let retained_guard = edit.allocate_optional_guard();
    edit.remove_optional_guard(deleted_guard).unwrap();

    let result = commit(edit).expect("structurally complete sparse state commits");

    assert_map(&result.maps.storage, callable, [0, 2, 3], [0, 1, 2]);
    assert_map(&result.maps.values, callable, [0, 2, 3], [0, 1, 2]);
    assert_map(&result.maps.blocks, callable, [0, 3, 2], [0, 1, 2]);
    assert_map(&result.maps.path_conditions, callable, [0, 2, 3], [0, 1, 2]);
    assert_eq!(
        result.maps.optional_guards.committed(retained_guard),
        Ok(OptionalGuardId::new(callable, 0))
    );
    assert!(matches!(
        result.maps.optional_guards.committed(deleted_guard),
        Err(MirRewriteError::DeletedIdentity { .. })
    ));
    assert_eq!(new_storage.index(), 3);
    assert_eq!(new_value.index(), 3);
    assert_eq!(new_block.index(), 3);
    assert_eq!(new_path.index(), 3);

    assert_dense_declarations(&result.callable);
    let expected = MirEntityChangeCount {
        retained: 2,
        inserted: 1,
        removed: 1,
    };
    assert_eq!(result.changes.storage, expected);
    assert_eq!(result.changes.values, expected);
    assert_eq!(result.changes.blocks, expected);
    assert_eq!(result.changes.path_conditions, expected);
    assert_eq!(result.changes.logical_expressions, expected);
    assert_eq!(
        result.changes.optional_guards,
        MirEntityChangeCount {
            retained: 0,
            inserted: 1,
            removed: 0,
        }
    );
}

#[test]
fn tombstoned_references_fail_at_the_first_deterministic_site_for_every_kind() {
    let cases = [
        (
            MirLocalIdentity::Storage(StorageId::new(edit().callable(), 0)),
            MirLocalIdentitySite::Terminator(0),
        ),
        (
            MirLocalIdentity::Value(ValueId::new(edit().callable(), 0)),
            MirLocalIdentitySite::Terminator(1),
        ),
        (
            MirLocalIdentity::Block(BlockId::new(edit().callable(), 1)),
            MirLocalIdentitySite::Terminator(0),
        ),
        (
            MirLocalIdentity::PathCondition(PathConditionId::new(edit().callable(), 0)),
            MirLocalIdentitySite::PathCondition(0),
        ),
        (
            MirLocalIdentity::OptionalGuard(OptionalGuardId::new(edit().callable(), 2)),
            MirLocalIdentitySite::Terminator(0),
        ),
    ];

    for (identity, site) in cases {
        let mut transaction = edit();
        remove_identity(&mut transaction, identity);
        assert_eq!(
            commit(transaction),
            Err(MirRewriteError::InvalidReference {
                expected: identity.callable(),
                identity,
                site,
                failure: MirReferenceFailure::Deleted,
            })
        );
    }
}

#[test]
fn foreign_references_fail_with_identity_kind_and_structural_site() {
    let foreign = CallableId::Function(FunctionId::new(99));
    let mutations: [fn(&mut MirBody, CallableId); 5] = [
        |body, owner| match body.blocks[0].terminator.as_mut().unwrap() {
            MirTerminator::BeginOptionalView { begin, .. } => {
                begin.source.base = MirPlaceBase::Storage(StorageId::new(owner, 0))
            }
            _ => unreachable!(),
        },
        |body, owner| match body.blocks[1].terminator.as_mut().unwrap() {
            MirTerminator::Return { value, .. } => *value = Some(ValueId::new(owner, 0)),
            _ => unreachable!(),
        },
        |body, owner| match body.blocks[0].terminator.as_mut().unwrap() {
            MirTerminator::BeginOptionalView { success_target, .. } => {
                *success_target = BlockId::new(owner, 0)
            }
            _ => unreachable!(),
        },
        |body, owner| {
            body.logical_expressions[0].condition = PathConditionId::new(owner, 0);
        },
        |body, owner| match body.blocks[0].terminator.as_mut().unwrap() {
            MirTerminator::BeginOptionalView { begin, .. } => {
                begin.guard = OptionalGuardId::new(owner, 0)
            }
            _ => unreachable!(),
        },
    ];
    let expected_sites = [
        MirLocalIdentitySite::Terminator(0),
        MirLocalIdentitySite::Terminator(1),
        MirLocalIdentitySite::Terminator(0),
        MirLocalIdentitySite::LogicalExpression(0),
        MirLocalIdentitySite::Terminator(0),
    ];

    for (mutate, expected_site) in mutations.into_iter().zip(expected_sites) {
        let (callable, storage, values, mut body) = fixture_parts();
        mutate(&mut body, foreign);
        let error = commit(
            MirCallableEdit::from_dense_parts(callable, storage, values, body)
                .expect("foreign references remain transaction data until commit"),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            MirRewriteError::InvalidReference {
                expected,
                site,
                failure: MirReferenceFailure::Foreign,
                ..
            } if expected == callable && site == expected_site
        ));
    }
}

#[test]
fn unknown_references_and_body_entry_are_rejected_at_their_structural_sites() {
    let mutations: [fn(&mut MirBody, CallableId); 4] = [
        |body, owner| match body.blocks[0].terminator.as_mut().unwrap() {
            MirTerminator::BeginOptionalView { begin, .. } => {
                begin.source.base = MirPlaceBase::Storage(StorageId::new(owner, 99))
            }
            _ => unreachable!(),
        },
        |body, owner| match body.blocks[1].terminator.as_mut().unwrap() {
            MirTerminator::Return { value, .. } => *value = Some(ValueId::new(owner, 99)),
            _ => unreachable!(),
        },
        |body, owner| body.entry = BlockId::new(owner, 99),
        |body, owner| {
            body.logical_expressions[0].condition = PathConditionId::new(owner, 99);
        },
    ];
    let sites = [
        MirLocalIdentitySite::Terminator(0),
        MirLocalIdentitySite::Terminator(1),
        MirLocalIdentitySite::BodyEntry,
        MirLocalIdentitySite::LogicalExpression(0),
    ];

    for (mutate, site) in mutations.into_iter().zip(sites) {
        let (callable, storage, values, mut body) = fixture_parts();
        mutate(&mut body, callable);
        assert!(matches!(
            commit(MirCallableEdit::from_dense_parts(callable, storage, values, body).unwrap()),
            Err(MirRewriteError::InvalidReference {
                expected,
                site: actual_site,
                failure: MirReferenceFailure::Unknown,
                ..
            }) if expected == callable && actual_site == site
        ));
    }

    let mut guard = edit();
    let callable = guard.callable();
    let identity = OptionalGuardId::new(callable, 2);
    guard.forget_optional_guard_for_test(identity);
    assert_eq!(
        commit(guard),
        Err(MirRewriteError::InvalidReference {
            expected: callable,
            identity: MirLocalIdentity::OptionalGuard(identity),
            site: MirLocalIdentitySite::Terminator(0),
            failure: MirReferenceFailure::Unknown,
        })
    );
}

#[test]
fn instruction_references_distinguish_deleted_unknown_and_foreign_slots() {
    let local = edit().callable();
    let foreign = CallableId::Function(FunctionId::new(99));
    let cases = [
        (StorageId::new(local, 2), MirReferenceFailure::Deleted, true),
        (
            StorageId::new(local, 99),
            MirReferenceFailure::Unknown,
            false,
        ),
        (
            StorageId::new(foreign, 0),
            MirReferenceFailure::Foreign,
            false,
        ),
    ];

    for (identity, failure, remove) in cases {
        let mut transaction = edit();
        let block = BlockId::new(local, 0);
        let span = transaction.block(block).unwrap().span;
        transaction
            .rewrite_block_instructions(block, |instructions| {
                let mut rewritten = vec![crate::mir::MirInstruction::StorageLive(MirStorageLive {
                    storage: identity,
                    span,
                })];
                rewritten.extend_from_slice(instructions);
                rewritten
            })
            .unwrap();
        if remove {
            transaction.remove_storage(identity).unwrap();
        }

        assert_eq!(
            commit(transaction),
            Err(MirRewriteError::InvalidReference {
                expected: local,
                identity: MirLocalIdentity::Storage(identity),
                site: MirLocalIdentitySite::Instruction {
                    block: 0,
                    instruction: 0,
                },
                failure,
            })
        );
    }
}

#[test]
fn missing_duplicate_and_logical_orders_are_rejected_before_compaction() {
    let mut missing = edit();
    let callable = missing.callable();
    missing.replace_block_order_for_test(vec![BlockId::new(callable, 0)]);
    assert_eq!(
        commit(missing),
        Err(MirRewriteError::MissingOrderIdentity {
            identity: MirLocalIdentity::Block(BlockId::new(callable, 1)),
        })
    );

    let mut duplicate = edit();
    duplicate.replace_block_order_for_test(vec![
        BlockId::new(callable, 0),
        BlockId::new(callable, 0),
        BlockId::new(callable, 1),
    ]);
    assert!(matches!(
        commit(duplicate),
        Err(MirRewriteError::DuplicateOrderIdentity { .. })
    ));

    let mut logical = edit();
    logical.replace_logical_order_for_test(vec![
        LogicalRecordIndex::new(0),
        LogicalRecordIndex::new(0),
        LogicalRecordIndex::new(1),
    ]);
    assert_eq!(
        commit(logical),
        Err(MirRewriteError::DuplicateLogicalOrder { index: 0 })
    );

    let mut missing_logical = edit();
    missing_logical.replace_logical_order_for_test(vec![LogicalRecordIndex::new(0)]);
    assert_eq!(
        commit(missing_logical),
        Err(MirRewriteError::MissingLogicalOrder { index: 1 })
    );
}

#[test]
fn the_first_error_and_change_summary_are_deterministic() {
    fn failing_transaction() -> MirCallableEdit {
        let mut transaction = edit();
        let callable = transaction.callable();
        transaction
            .remove_storage(StorageId::new(callable, 0))
            .unwrap();
        transaction.remove_value(ValueId::new(callable, 0)).unwrap();
        transaction
    }
    assert_eq!(commit(failing_transaction()), commit(failing_transaction()));

    let first = commit(gap_edit()).unwrap().changes;
    let second = commit(gap_edit()).unwrap().changes;
    assert_eq!(first, second);
}

fn gap_edit() -> MirCallableEdit {
    let (callable, storage, values, fixture) = fixture_parts();
    let span = fixture.blocks[0].span;
    let block = |index| BlockId::new(callable, index);
    let storage_id = |index| StorageId::new(callable, index);
    let value = |index| ValueId::new(callable, index);
    let path = |index| PathConditionId::new(callable, index);
    let mut logical = fixture.logical_expressions[0].clone();
    logical.condition = path(0);
    logical.result = storage_id(0);
    logical.left_result = value(0);
    logical.right_result = value(2);
    logical.selected_result = value(2);
    logical.split = block(0);
    logical.selection = block(0);
    logical.right_entry = block(2);
    logical.right_exit = block(2);
    logical.short = block(2);
    logical.join = block(2);
    let path_condition = |index, parent, activation| MirPathCondition {
        id: path(index),
        parent,
        activation,
        active_predecessor: block(0),
        inactive_predecessor: block(0),
        merge: block(2),
        span,
    };
    let body = MirBody {
        entry: block(0),
        blocks: vec![
            MirBasicBlock {
                id: block(0),
                instructions: vec![],
                terminator: Some(MirTerminator::Goto {
                    target: block(2),
                    span,
                }),
                span,
            },
            empty_return_block(block(1), span),
            MirBasicBlock {
                id: block(2),
                instructions: vec![],
                terminator: Some(MirTerminator::Return {
                    value: Some(value(2)),
                    span,
                }),
                span,
            },
        ],
        path_conditions: vec![
            path_condition(0, None, storage_id(0)),
            path_condition(1, None, storage_id(1)),
            path_condition(2, Some(path(0)), storage_id(2)),
        ],
        logical_expressions: vec![logical.clone(), logical.clone(), logical],
    };
    MirCallableEdit::from_dense_parts(callable, storage, values, body).unwrap()
}

fn empty_return_block(identity: BlockId, span: crate::source::Span) -> MirBasicBlock {
    MirBasicBlock {
        id: identity,
        instructions: vec![],
        terminator: Some(MirTerminator::Return { value: None, span }),
        span,
    }
}

fn remove_identity(edit: &mut MirCallableEdit, identity: MirLocalIdentity) {
    match identity {
        MirLocalIdentity::Storage(identity) => {
            edit.remove_storage(identity).unwrap();
        }
        MirLocalIdentity::Value(identity) => {
            edit.remove_value(identity).unwrap();
        }
        MirLocalIdentity::Block(identity) => {
            edit.remove_block(identity).unwrap();
        }
        MirLocalIdentity::PathCondition(identity) => {
            edit.remove_path_condition(identity).unwrap();
        }
        MirLocalIdentity::OptionalGuard(identity) => {
            edit.remove_optional_guard(identity).unwrap();
        }
    }
}

fn assert_map<I: MirLocalId + std::fmt::Debug, const N: usize>(
    map: &MirCommitMap<I>,
    callable: CallableId,
    old: [usize; N],
    new: [usize; N],
) {
    for (old, new) in old.into_iter().zip(new) {
        assert_eq!(
            map.committed(I::new(callable, old)),
            Ok(I::new(callable, new))
        );
    }
}

fn assert_dense_declarations(callable: &MirCommittedCallable) {
    assert!(callable
        .storage
        .iter()
        .enumerate()
        .all(|(index, declaration)| declaration.id == StorageId::new(callable.callable, index)));
    assert!(callable
        .values
        .iter()
        .enumerate()
        .all(|(index, declaration)| declaration.id == ValueId::new(callable.callable, index)));
    assert!(callable
        .body
        .blocks
        .iter()
        .enumerate()
        .all(|(index, declaration)| declaration.id == BlockId::new(callable.callable, index)));
    assert!(callable
        .body
        .path_conditions
        .iter()
        .enumerate()
        .all(|(index, declaration)| {
            declaration.id == PathConditionId::new(callable.callable, index)
        }));
}
