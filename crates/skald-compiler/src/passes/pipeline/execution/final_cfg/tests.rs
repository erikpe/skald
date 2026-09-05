use crate::{
    identity::CallableId,
    mir::{
        rewrite::{
            rewrite_program, MirLocalIdentity, MirLocalIdentitySite, MirReferenceFailure,
            MirRewriteError,
        },
        BlockId, MirAssignment, MirBasicBlock, MirInstruction, MirRvalue, MirRvalueKind,
        MirStorageKind, MirTerminator, MirType, MirValue, ValueId,
    },
    passes::verify_final_mir,
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::passes::pipeline::execution::{MirFinalPassCapability, MirPassFailure};

#[test]
fn final_cfg_capability_rejects_storage_reclassification_before_commit() {
    let source = "fn main() -> i64 { if (true && false) { return 1; } return 0; }";
    let verified = verify_final_mir(lower_source_to_final_mir(source)).unwrap();
    let original = verified.program().clone();
    let owner = CallableId::Function(verified.entry_function);
    let activation = verified
        .definitions
        .get(verified.entry_function)
        .unwrap()
        .storage
        .iter()
        .find(|storage| storage.kind.is_normalized_path_activation())
        .unwrap()
        .id;

    let error = match MirFinalPassCapability::new(verified).rewrite_cfg(|callable, edit| {
        if callable == owner {
            edit.edit.replace_storage_kind(
                activation,
                MirStorageKind::NormalizedPathActivation,
                MirStorageKind::ScalarSpill,
            )?;
        }
        Ok(())
    }) {
        Ok(_) => panic!("final CFG capability unexpectedly published a storage mutation"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        MirPassFailure::Rewrite(MirRewriteError::UnsupportedFinalCfgStorageMutation {
            callable
        }) if callable == owner
    ));
    assert_eq!(
        original,
        *verify_final_mir(lower_source_to_final_mir(source))
            .unwrap()
            .program()
    );
}

#[test]
fn exact_normalized_snapshot_removes_only_disconnected_blocks_and_values() {
    let program = disconnected_program();
    let verified = verify_final_mir(program).unwrap();
    let owner = CallableId::Function(verified.entry_function);
    let mut observed = None;

    let rewritten = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let mut edit = MirFinalCfgEdit::new(edit);
            let facts = edit.facts()?;
            observed = Some(edit.remove_unreachable_blocks(&facts)?);
        }
        Ok(())
    })
    .unwrap();

    assert_eq!(
        observed,
        Some(MirFinalCfgRemoval {
            blocks: 1,
            values: 1,
        })
    );
    let definition = rewritten
        .program
        .definitions
        .get(verified.entry_function)
        .unwrap();
    assert_eq!(definition.body.blocks.len(), 1);
    assert_eq!(definition.values.len(), 1);
}

#[test]
fn stale_normalized_snapshot_is_rejected_before_further_deletion() {
    let program = disconnected_program();
    let verified = verify_final_mir(program).unwrap();
    let owner = CallableId::Function(verified.entry_function);
    let disconnected = BlockId::new(owner, 1);

    let error = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let facts = edit.final_cfg_facts()?;
            edit.remove_block(disconnected)?;
            MirFinalCfgEdit::new(edit).remove_unreachable_blocks(&facts)?;
        }
        Ok(())
    })
    .unwrap_err();

    assert_eq!(
        error,
        MirRewriteError::StaleCallableSnapshot {
            callable: owner,
            subject: "normalized CFG facts",
        }
    );
}

#[test]
fn dense_commit_rejects_a_reachable_use_of_a_deleted_block_value() {
    let program = disconnected_program();
    let verified = verify_final_mir(program).unwrap();
    let owner = CallableId::Function(verified.entry_function);
    let entry = BlockId::new(owner, 0);
    let disconnected_value = ValueId::new(owner, 1);
    let span = verified
        .program()
        .definitions
        .get(verified.entry_function)
        .unwrap()
        .span;

    let error = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let facts = edit.final_cfg_facts()?;
            edit.rewrite_block_terminator(entry, |_| {
                Some(MirTerminator::Return {
                    value: Some(disconnected_value),
                    span,
                })
            })?;
            MirFinalCfgEdit::new(edit).remove_unreachable_blocks(&facts)?;
        }
        Ok(())
    })
    .unwrap_err();

    assert_eq!(
        error,
        MirRewriteError::InvalidReference {
            expected: owner,
            identity: MirLocalIdentity::Value(disconnected_value),
            site: MirLocalIdentitySite::Terminator(entry.index()),
            failure: MirReferenceFailure::Deleted,
        }
    );
}

fn disconnected_program() -> crate::mir::MirProgram {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let owner = definition.callable();
    let span = definition.span;
    let value = ValueId::new(owner, definition.values.len());
    definition.values.push(MirValue {
        id: value,
        ty: MirType::I64,
        span,
    });
    definition.body.blocks.push(MirBasicBlock {
        id: BlockId::new(owner, definition.body.blocks.len()),
        instructions: vec![MirInstruction::Assign(MirAssignment {
            result: value,
            rvalue: MirRvalue {
                kind: MirRvalueKind::ConstantI64(99),
                ty: MirType::I64,
            },
            span,
        })],
        terminator: Some(MirTerminator::Return {
            value: Some(value),
            span,
        }),
        span,
    });
    program
}
