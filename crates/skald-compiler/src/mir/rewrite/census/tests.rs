use crate::{
    identity::{CallableId, FunctionId},
    mir::{
        rewrite::tests::representative_function, BlockId, MirAssignment, MirBasicBlock, MirBody,
        MirInstruction, MirRvalue, MirRvalueKind, MirTerminator, MirType, MirValue, ValueId,
    },
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::mir::rewrite::{edit::test_support, rewrite_program};

fn representative_edit() -> MirCallableEdit {
    let definition = representative_function();
    MirCallableEdit::from_dense_parts(
        definition.callable(),
        definition.storage,
        definition.values,
        definition.body,
    )
    .unwrap()
}

fn value(owner: CallableId, index: usize) -> ValueId {
    ValueId::new(owner, index)
}

#[test]
fn census_distinguishes_declarations_definitions_and_actual_uses() {
    let edit = representative_edit();
    let owner = edit.callable();

    let census = edit.value_use_census().unwrap();

    assert_eq!(census.callable(), owner);
    assert_eq!(census.len(), 5);
    assert!(!census.is_empty());
    assert_eq!(
        census.iter().map(|entry| entry.value()).collect::<Vec<_>>(),
        (0..5).map(|index| value(owner, index)).collect::<Vec<_>>()
    );
    assert_eq!(
        census.get(value(owner, 0)).copied(),
        Some(MirValueCensusEntry {
            value: value(owner, 0),
            definition: Some(MirLocalIdentitySite::Instruction {
                block: 0,
                instruction: 1,
            }),
            uses: 2,
        })
    );
    assert_eq!(census.get(value(owner, 1)).unwrap().definition(), None);
    assert_eq!(census.get(value(owner, 1)).unwrap().uses(), 3);
    assert_eq!(
        census.get(value(owner, 2)).unwrap().definition(),
        Some(MirLocalIdentitySite::Instruction {
            block: 0,
            instruction: 2,
        })
    );
    assert_eq!(census.get(value(owner, 2)).unwrap().uses(), 1);
    assert_eq!(census.get(value(owner, 3)).unwrap().uses(), 1);
    assert_eq!(
        census.get(value(owner, 4)).unwrap().definition(),
        Some(MirLocalIdentitySite::Instruction {
            block: 0,
            instruction: 8,
        })
    );
    assert_eq!(census.get(value(owner, 4)).unwrap().uses(), 0);
}

#[test]
fn census_counts_logical_proof_references_without_treating_them_as_definitions() {
    let edit = test_support::edit();
    let owner = edit.callable();

    let census = edit.value_use_census().unwrap();

    assert_eq!(census.get(value(owner, 0)).unwrap().uses(), 3);
    assert_eq!(census.get(value(owner, 1)).unwrap().uses(), 3);
    assert_eq!(census.get(value(owner, 2)).unwrap().uses(), 1);
    assert!(census.iter().all(|entry| entry.definition().is_none()));
}

#[test]
fn values_used_only_by_another_dead_definition_remain_visible_to_fixed_point_passes() {
    let base = representative_function();
    let owner = base.callable();
    let span = base.span;
    let first = value(owner, 0);
    let second = value(owner, 1);
    let block = BlockId::new(owner, 0);
    let values = [first, second]
        .into_iter()
        .map(|id| MirValue {
            id,
            ty: MirType::I64,
            span,
        })
        .collect();
    let body = MirBody {
        entry: block,
        blocks: vec![MirBasicBlock {
            id: block,
            instructions: vec![
                MirInstruction::Assign(MirAssignment {
                    result: first,
                    rvalue: MirRvalue {
                        kind: MirRvalueKind::ConstantI64(1),
                        ty: MirType::I64,
                    },
                    span,
                }),
                MirInstruction::Assign(MirAssignment {
                    result: second,
                    rvalue: MirRvalue {
                        kind: MirRvalueKind::Binary {
                            operation: crate::mir::MirBinaryOperation::AddI64,
                            left: first,
                            right: first,
                        },
                        ty: MirType::I64,
                    },
                    span,
                }),
            ],
            terminator: Some(MirTerminator::Return { value: None, span }),
            span,
        }],
        path_conditions: vec![],
        logical_expressions: vec![],
    };
    let edit = MirCallableEdit::from_dense_parts(owner, vec![], values, body).unwrap();

    let census = edit.value_use_census().unwrap();

    assert_eq!(census.get(first).unwrap().uses(), 2);
    assert_eq!(census.get(second).unwrap().uses(), 0);
}

#[test]
fn census_is_read_only_and_deterministic_for_the_same_snapshot() {
    let edit = representative_edit();
    let expected = edit.clone();

    let first = edit.value_use_census().unwrap();
    let second = edit.value_use_census().unwrap();

    assert_eq!(first, second);
    assert_eq!(edit, expected);
    assert!(first
        .get(value(CallableId::Function(FunctionId::new(99)), 0))
        .is_none());
}

#[test]
fn dense_definition_census_matches_the_callable_edit_analysis() {
    let definition = representative_function();
    let expected = MirCallableEdit::from_dense_parts(
        definition.callable(),
        definition.storage.clone(),
        definition.values.clone(),
        definition.body.clone(),
    )
    .unwrap()
    .value_use_census()
    .unwrap();

    assert_eq!(
        value_use_census_for_definition((&definition).into()).unwrap(),
        expected
    );
}

#[test]
fn census_rejects_foreign_unknown_and_deleted_value_references() {
    for (replacement, failure) in [
        (
            value(CallableId::Function(FunctionId::new(99)), 0),
            MirReferenceFailure::Foreign,
        ),
        (
            value(representative_edit().callable(), 99),
            MirReferenceFailure::Unknown,
        ),
    ] {
        let mut edit = representative_edit();
        let owner = edit.callable();
        edit.rewrite_block_terminator(BlockId::new(owner, 2), |_| {
            Some(MirTerminator::Return {
                value: Some(replacement),
                span: representative_function().span,
            })
        })
        .unwrap();

        assert_eq!(
            edit.value_use_census().unwrap_err(),
            MirRewriteError::InvalidReference {
                expected: owner,
                identity: MirLocalIdentity::Value(replacement),
                site: MirLocalIdentitySite::Terminator(2),
                failure,
            }
        );
    }

    let mut edit = representative_edit();
    let owner = edit.callable();
    let deleted = value(owner, 0);
    edit.remove_value(deleted).unwrap();
    assert_eq!(
        edit.value_use_census().unwrap_err(),
        MirRewriteError::InvalidReference {
            expected: owner,
            identity: MirLocalIdentity::Value(deleted),
            site: MirLocalIdentitySite::Instruction {
                block: 0,
                instruction: 1,
            },
            failure: MirReferenceFailure::Deleted,
        }
    );
}

#[test]
fn census_rejects_foreign_unknown_and_duplicate_value_definitions() {
    let foreign = value(CallableId::Function(FunctionId::new(99)), 0);
    for (replacement, failure) in [
        (foreign, MirReferenceFailure::Foreign),
        (
            value(representative_edit().callable(), 99),
            MirReferenceFailure::Unknown,
        ),
    ] {
        let mut edit = representative_edit();
        let owner = edit.callable();
        edit.rewrite_block_instructions(BlockId::new(owner, 0), |instructions| {
            let mut instructions = instructions.to_vec();
            let MirInstruction::Assign(assignment) = &mut instructions[1] else {
                unreachable!()
            };
            assignment.result = replacement;
            instructions
        })
        .unwrap();

        assert_eq!(
            edit.value_use_census().unwrap_err(),
            MirRewriteError::InvalidReference {
                expected: owner,
                identity: MirLocalIdentity::Value(replacement),
                site: MirLocalIdentitySite::Instruction {
                    block: 0,
                    instruction: 1,
                },
                failure,
            }
        );
    }

    let mut edit = representative_edit();
    let owner = edit.callable();
    let duplicate = value(owner, 0);
    edit.rewrite_block_instructions(BlockId::new(owner, 0), |instructions| {
        let mut instructions = instructions.to_vec();
        instructions.push(instructions[1].clone());
        instructions
    })
    .unwrap();
    assert_eq!(
        edit.value_use_census().unwrap_err(),
        MirRewriteError::DuplicateValueDefinition {
            value: duplicate,
            first: MirLocalIdentitySite::Instruction {
                block: 0,
                instruction: 1,
            },
            duplicate: MirLocalIdentitySite::Instruction {
                block: 0,
                instruction: 9,
            },
        }
    );
}

#[test]
fn recomputed_census_uses_current_sparse_value_indexing() {
    let mut edit = representative_edit();
    let owner = edit.callable();
    let removed = value(owner, 4);
    edit.rewrite_block_instructions(BlockId::new(owner, 0), |instructions| {
        instructions[..8].to_vec()
    })
    .unwrap();
    edit.remove_value(removed).unwrap();

    let census = edit.value_use_census().unwrap();

    assert_eq!(census.len(), 4);
    assert!(census.get(removed).is_none());
    assert_eq!(
        census.iter().map(|entry| entry.value()).collect::<Vec<_>>(),
        (0..4).map(|index| value(owner, index)).collect::<Vec<_>>()
    );
}

#[test]
fn census_composes_with_every_executable_definition_kind() {
    let program = lower_source_to_final_mir(
        "class State {\n\
           static seed: i64 = 1;\n\
           value: i64;\n\
           init(value: i64) { self.value = value; }\n\
           fn read() -> i64 { return self.value; }\n\
         }\n\
         fn main() -> i64 { var state: State = State(2); return state.read() + State.seed; }",
    );
    let mut saw_function = false;
    let mut saw_member = false;
    let mut saw_static_initializer = false;

    rewrite_program(program, |callable, edit| {
        let census = edit.value_use_census()?;
        assert_eq!(census.callable(), callable);
        match callable {
            CallableId::Function(_) => saw_function = true,
            CallableId::StaticInitializer(_) => saw_static_initializer = true,
            CallableId::Initializer(_)
            | CallableId::CopyConstructor(_)
            | CallableId::CopyAssignment(_)
            | CallableId::Destructor(_)
            | CallableId::Method(_) => saw_member = true,
        }
        Ok(())
    })
    .unwrap();

    assert!(saw_function);
    assert!(saw_member);
    assert!(saw_static_initializer);
}
