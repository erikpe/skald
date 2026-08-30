use crate::{
    identity::{CallableId, FunctionId},
    mir::{
        BlockId, MirAssignment, MirInstruction, MirRvalue, MirRvalueKind, MirStorage,
        MirStorageDead, MirStorageKind, MirStorageLive, MirTerminator, MirType, MirValue,
        StorageId, ValueId,
    },
};

use super::super::{test_support::edit, BlockPlacement};
use crate::mir::rewrite::{commit::commit, MirLocalIdentity, MirRewriteError};

#[test]
fn value_use_substitution_preserves_definitions_and_allows_explicit_deletion() {
    let mut edit = edit();
    let owner = edit.callable();
    let block = BlockId::new(owner, 1);
    let from = ValueId::new(owner, 0);
    let to = ValueId::new(owner, 1);
    let span = edit.block(block).unwrap().span;
    edit.rewrite_block_instructions(block, |instructions| {
        let mut rewritten = instructions.to_vec();
        rewritten.push(MirInstruction::Assign(MirAssignment {
            result: from,
            rvalue: MirRvalue {
                kind: MirRvalueKind::ConstantI64(1),
                ty: MirType::I64,
            },
            span,
        }));
        rewritten
    })
    .unwrap();

    assert_eq!(edit.replace_value_uses(from, to).unwrap(), 3);
    let assignment = edit
        .block(block)
        .unwrap()
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) => Some(assignment),
            _ => None,
        })
        .unwrap();
    assert_eq!(assignment.result, from);
    assert!(matches!(
        edit.block(block).unwrap().terminator,
        Some(MirTerminator::Return { value: Some(value), .. }) if value == to
    ));

    edit.rewrite_block_instructions(block, |instructions| {
        instructions
            .iter()
            .filter(|instruction| {
                !matches!(instruction, MirInstruction::Assign(assignment) if assignment.result == from)
            })
            .cloned()
            .collect()
    })
    .unwrap();
    edit.remove_value(from).unwrap();
    commit(edit).expect("explicit use, definition, and declaration edits commit together");
}

#[test]
fn storage_substitution_leaves_liveness_cleanup_explicit() {
    let mut edit = edit();
    let owner = edit.callable();
    let block = BlockId::new(owner, 1);
    let from = StorageId::new(owner, 0);
    let to = StorageId::new(owner, 1);
    let span = edit.block(block).unwrap().span;
    edit.rewrite_block_instructions(block, |instructions| {
        let mut rewritten = vec![
            MirInstruction::StorageLive(MirStorageLive {
                storage: from,
                span,
            }),
            MirInstruction::StorageDead(MirStorageDead {
                storage: from,
                span,
            }),
        ];
        rewritten.extend_from_slice(instructions);
        rewritten
    })
    .unwrap();

    assert!(edit.replace_storage_uses(from, to).unwrap() > 2);
    assert!(matches!(
        edit.block(block).unwrap().instructions[0],
        MirInstruction::StorageLive(MirStorageLive { storage, .. }) if storage == to
    ));
    edit.rewrite_block_instructions(block, |instructions| {
        instructions
            .iter()
            .filter(|instruction| {
                !matches!(
                    instruction,
                    MirInstruction::StorageLive(MirStorageLive { storage, .. })
                        | MirInstruction::StorageDead(MirStorageDead { storage, .. })
                        if *storage == to
                )
            })
            .cloned()
            .collect()
    })
    .unwrap();
    edit.remove_storage(from).unwrap();
    commit(edit).expect("storage proof operations were cleaned up explicitly");
}

#[test]
fn block_insertion_redirection_removal_and_order_are_explicit() {
    let mut edit = edit();
    let owner = edit.callable();
    let target = BlockId::new(owner, 1);
    let span = edit.block(target).unwrap().span;
    let forwarding = edit
        .allocate_block(BlockPlacement::Before(target), |id| {
            crate::mir::MirBasicBlock {
                id,
                instructions: Vec::new(),
                terminator: Some(MirTerminator::Return { value: None, span }),
                span,
            }
        })
        .unwrap();

    assert_eq!(edit.redirect_edges(target, forwarding).unwrap(), 3);
    edit.rewrite_block_terminator(forwarding, |_| Some(MirTerminator::Goto { target, span }))
        .unwrap();
    assert_eq!(edit.block_order()[1], forwarding);

    let disposable = edit
        .allocate_block(BlockPlacement::Append, |id| crate::mir::MirBasicBlock {
            id,
            instructions: Vec::new(),
            terminator: Some(MirTerminator::Return { value: None, span }),
            span,
        })
        .unwrap();
    edit.remove_block(disposable).unwrap();
    commit(edit).expect("explicit ordered CFG edit commits densely");
}

#[test]
fn proof_and_guard_cleanup_does_not_cascade() {
    let mut edit = edit();
    let owner = edit.callable();
    let first = BlockId::new(owner, 0);
    let second = BlockId::new(owner, 1);
    let guard = edit.optional_guard_ids().next().unwrap();
    let span = edit.block(first).unwrap().span;

    edit.rewrite_block_terminator(first, |_| {
        Some(MirTerminator::Goto {
            target: second,
            span,
        })
    })
    .unwrap();
    edit.rewrite_block_instructions(second, |instructions| {
        instructions
            .iter()
            .filter(|instruction| !matches!(instruction, MirInstruction::EndOptionalView(_)))
            .cloned()
            .collect()
    })
    .unwrap();
    edit.remove_optional_guard(guard).unwrap();
    for record in edit.logical_order().to_vec() {
        edit.remove_logical_record(record).unwrap();
    }
    for condition in edit
        .path_condition_ids()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        edit.remove_path_condition(condition).unwrap();
    }

    commit(edit).expect("caller supplied all guard and proof-metadata cleanup");
}

#[test]
fn substitutions_reject_type_and_owner_mistakes() {
    let mut edit = edit();
    let owner = edit.callable();
    let span = edit.value(ValueId::new(owner, 0)).unwrap().span;
    let boolean = edit
        .allocate_value(|id| MirValue {
            id,
            ty: MirType::Bool,
            span,
        })
        .unwrap();
    assert!(matches!(
        edit.replace_value_uses(ValueId::new(owner, 0), boolean),
        Err(MirRewriteError::ValueTypeMismatch { .. })
    ));

    let foreign = CallableId::Function(FunctionId::new(usize::MAX));
    assert!(matches!(
        edit.replace_storage_uses(StorageId::new(owner, 0), StorageId::new(foreign, 0)),
        Err(MirRewriteError::ForeignIdentity {
            identity: MirLocalIdentity::Storage(_),
            ..
        })
    ));

    let storage_span = edit.storage(StorageId::new(owner, 0)).unwrap().span;
    let boolean_storage = edit
        .allocate_storage(|id| MirStorage {
            id,
            source: None,
            name: "boolean".into(),
            kind: MirStorageKind::Temporary,
            ty: MirType::Bool,
            span: storage_span,
        })
        .unwrap();
    assert!(matches!(
        edit.replace_storage_uses(StorageId::new(owner, 0), boolean_storage),
        Err(MirRewriteError::StorageTypeMismatch { .. })
    ));
}
