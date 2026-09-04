use crate::{
    identity::CallableId,
    mir::{
        rewrite::{
            analyze_basic_block_merging, analyze_empty_block_forwarding,
            final_cfg_facts_for_definition, rewrite_program, MirBasicBlockMergeBarrierKind,
            MirBasicBlockMergeCandidate, MirLocalIdentity, MirLocalIdentitySite,
            MirReferenceFailure, MirRewriteError,
        },
        BlockId, MirAssignment, MirBasicBlock, MirFunctionDefinition, MirInstruction, MirProgram,
        MirRvalue, MirRvalueKind, MirTerminator, MirType, MirValue, ValueId,
    },
    passes::verify_final_mir,
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::passes::pipeline::execution::{MirFinalPassCapability, MirPassFailure};

#[test]
fn complete_forwarding_plan_redirects_chains_and_compacts_densely() {
    let verified = verify_final_mir(forwarding_chain_program()).unwrap();
    let owner = CallableId::Function(verified.entry_function);
    let definition = verified
        .program()
        .definitions
        .get(verified.entry_function)
        .unwrap();
    let expected_target_instructions = definition.body.blocks[3].instructions.clone();
    let expected_target_terminator = definition.body.blocks[3].terminator.clone();
    let facts = final_cfg_facts_for_definition(definition.into()).unwrap();
    let plan = analyze_empty_block_forwarding(&facts).plan().clone();
    let mut observed = None;

    let rewritten = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            observed =
                Some(MirFinalCfgEdit::new(edit).apply_empty_block_forwarding(&facts, &plan)?);
        }
        Ok(())
    })
    .unwrap();

    assert_eq!(observed.unwrap().removed_blocks(), 2);
    assert_eq!(observed.unwrap().redirected_edges(), 2);
    let result = rewritten
        .program
        .definitions
        .get(verified.entry_function)
        .unwrap();
    assert_eq!(result.body.blocks.len(), 2);
    assert!(matches!(
        result.body.blocks[0].terminator,
        Some(MirTerminator::Goto { target, .. }) if target == block(owner, 1)
    ));
    assert_eq!(
        result.body.blocks[1].instructions,
        expected_target_instructions
    );
    assert_eq!(result.body.blocks[1].terminator, expected_target_terminator);

    let report = rewritten
        .callables
        .iter()
        .find(|report| report.callable == owner)
        .unwrap();
    assert_eq!(
        report.maps.blocks.committed(block(owner, 0)).unwrap(),
        block(owner, 0)
    );
    assert!(matches!(
        report.maps.blocks.committed(block(owner, 1)),
        Err(MirRewriteError::DeletedIdentity { .. })
    ));
    assert!(matches!(
        report.maps.blocks.committed(block(owner, 2)),
        Err(MirRewriteError::DeletedIdentity { .. })
    ));
    assert_eq!(
        report.maps.blocks.committed(block(owner, 3)).unwrap(),
        block(owner, 1)
    );
    verify_final_mir(rewritten.program).expect("forwarded MIR remains valid and normalized");
}

#[test]
fn forwarding_preserves_branch_operand_roles_and_span() {
    let verified = verify_final_mir(branch_forwarding_program()).unwrap();
    let owner = CallableId::Function(verified.entry_function);
    let definition = verified
        .program()
        .definitions
        .get(verified.entry_function)
        .unwrap();
    let facts = final_cfg_facts_for_definition(definition.into()).unwrap();
    let plan = analyze_empty_block_forwarding(&facts).plan().clone();
    let (expected_condition, expected_span) = match definition.body.blocks[0]
        .terminator
        .as_ref()
        .expect("fixture entry has a terminator")
    {
        MirTerminator::Branch {
            condition, span, ..
        } => (*condition, *span),
        _ => panic!("fixture entry is a branch"),
    };
    let mut observed = None;

    let rewritten = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            observed =
                Some(MirFinalCfgEdit::new(edit).apply_empty_block_forwarding(&facts, &plan)?);
        }
        Ok(())
    })
    .unwrap();

    assert_eq!(observed.unwrap().removed_blocks(), 1);
    assert_eq!(observed.unwrap().redirected_edges(), 2);
    let entry = &rewritten
        .program
        .definitions
        .get(verified.entry_function)
        .unwrap()
        .body
        .blocks[0];
    assert!(matches!(
        entry.terminator,
        Some(MirTerminator::Branch {
            condition,
            true_target,
            false_target,
            span,
        }) if condition == expected_condition
            && true_target == block(owner, 1)
            && false_target == block(owner, 1)
            && span == expected_span
    ));
    verify_final_mir(rewritten.program).expect("redirected branch remains valid");
}

#[test]
fn forwarding_rejects_stale_facts_and_cycle_derived_incomplete_plans() {
    let verified = verify_final_mir(forwarding_chain_program()).unwrap();
    let owner = CallableId::Function(verified.entry_function);
    let cycle_program = forwarding_cycle_program();
    let cycle_definition = cycle_program
        .definitions
        .get(cycle_program.entry_function)
        .unwrap();
    let cycle_facts = final_cfg_facts_for_definition(cycle_definition.into()).unwrap();
    let cycle_plan = analyze_empty_block_forwarding(&cycle_facts).plan().clone();
    assert!(cycle_plan.is_empty());

    let incomplete_error = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let facts = edit.final_cfg_facts()?;
            MirFinalCfgEdit::new(edit).apply_empty_block_forwarding(&facts, &cycle_plan)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(
        incomplete_error,
        MirRewriteError::StaleCallableSnapshot {
            callable: owner,
            subject: "empty-block forwarding plan",
        }
    );

    let stale_error = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let facts = edit.final_cfg_facts()?;
            let plan = analyze_empty_block_forwarding(&facts).plan().clone();
            let forwarding = plan.resolutions()[0].block();
            let span = edit.block(forwarding)?.span;
            edit.rewrite_block_terminator(forwarding, |_| {
                Some(MirTerminator::Return { value: None, span })
            })?;
            MirFinalCfgEdit::new(edit).apply_empty_block_forwarding(&facts, &plan)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(
        stale_error,
        MirRewriteError::StaleCallableSnapshot {
            callable: owner,
            subject: "normalized CFG facts",
        }
    );
}

#[test]
fn forwarding_rejects_foreign_and_deleted_plan_identities_before_mutation() {
    let verified = verify_final_mir(two_forwarding_functions()).unwrap();
    let owner = CallableId::Function(verified.entry_function);
    let foreign_definition = verified
        .program()
        .definitions
        .iter()
        .find(|definition| definition.callable() != owner)
        .unwrap();
    let foreign_facts = final_cfg_facts_for_definition(foreign_definition.into()).unwrap();
    let foreign_plan = analyze_empty_block_forwarding(&foreign_facts)
        .plan()
        .clone();
    let foreign_block = foreign_plan.resolutions()[0].block();

    let foreign_error = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let facts = edit.final_cfg_facts()?;
            MirFinalCfgEdit::new(edit).apply_empty_block_forwarding(&facts, &foreign_plan)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(
        foreign_error,
        MirRewriteError::ForeignIdentity {
            expected: owner,
            identity: MirLocalIdentity::Block(foreign_block),
        }
    );

    let deleted_error = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let facts = edit.final_cfg_facts()?;
            let plan = analyze_empty_block_forwarding(&facts).plan().clone();
            let deleted = plan.resolutions()[0].block();
            edit.remove_block(deleted)?;
            MirFinalCfgEdit::new(edit).apply_empty_block_forwarding(&facts, &plan)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(
        deleted_error,
        MirRewriteError::DeletedIdentity {
            identity: MirLocalIdentity::Block(_),
        }
    ));
}

#[test]
fn merge_moves_complete_contents_and_preserves_values_storage_order_and_spans() {
    let verified = verify_final_mir(merge_program()).unwrap();
    let owner = CallableId::Function(verified.entry_function);
    let definition = verified
        .program()
        .definitions
        .get(verified.entry_function)
        .unwrap();
    let facts = final_cfg_facts_for_definition(definition.into()).unwrap();
    let candidate = analyze_basic_block_merging(&facts)
        .first_candidate()
        .unwrap();
    let expected_storage = definition.storage.clone();
    let expected_values = definition.values.clone();
    let predecessor_span = definition.body.blocks[0].span;
    let mut expected_instructions = definition.body.blocks[0].instructions.clone();
    expected_instructions.extend(definition.body.blocks[1].instructions.clone());
    let expected_terminator = definition.body.blocks[1].terminator.clone();
    let expected_moved = definition.body.blocks[1].instructions.len();
    let mut observed = None;

    let rewritten = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            observed = Some(MirFinalCfgEdit::new(edit).merge_basic_blocks(&facts, candidate)?);
        }
        Ok(())
    })
    .unwrap();

    assert_eq!(observed.unwrap().moved_instructions(), expected_moved);
    let result = rewritten
        .program
        .definitions
        .get(verified.entry_function)
        .unwrap();
    assert_eq!(result.body.blocks.len(), 1);
    assert_eq!(result.body.entry, block(owner, 0));
    assert_eq!(result.body.blocks[0].span, predecessor_span);
    assert_eq!(result.body.blocks[0].instructions, expected_instructions);
    assert_eq!(result.body.blocks[0].terminator, expected_terminator);
    assert_eq!(result.storage, expected_storage);
    assert_eq!(result.values, expected_values);

    let report = rewritten
        .callables
        .iter()
        .find(|report| report.callable == owner)
        .unwrap();
    assert!(matches!(
        report.maps.blocks.committed(block(owner, 1)),
        Err(MirRewriteError::DeletedIdentity { .. })
    ));
    verify_final_mir(rewritten.program).expect("merged MIR remains valid and normalized");
}

#[test]
fn merge_rejects_stale_unauthorized_and_foreign_pairs() {
    let verified = verify_final_mir(merge_program()).unwrap();
    let owner = CallableId::Function(verified.entry_function);

    let stale_error = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let facts = edit.final_cfg_facts()?;
            let candidate = analyze_basic_block_merging(&facts)
                .first_candidate()
                .unwrap();
            let span = edit.block(candidate.predecessor())?.span;
            edit.rewrite_block_terminator(candidate.predecessor(), |_| {
                Some(MirTerminator::Return { value: None, span })
            })?;
            MirFinalCfgEdit::new(edit).merge_basic_blocks(&facts, candidate)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(
        stale_error,
        MirRewriteError::StaleCallableSnapshot {
            callable: owner,
            subject: "normalized CFG facts",
        }
    );

    let unauthorized_error = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let facts = edit.final_cfg_facts()?;
            let candidate =
                MirBasicBlockMergeCandidate::unchecked(block(owner, 1), block(owner, 0));
            MirFinalCfgEdit::new(edit).merge_basic_blocks(&facts, candidate)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(
        unauthorized_error,
        MirRewriteError::StaleCallableSnapshot {
            callable: owner,
            subject: "basic-block merge candidate",
        }
    );

    let foreign = CallableId::Function(crate::identity::FunctionId::new(usize::MAX));
    let foreign_block = block(foreign, 0);
    let foreign_error = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let facts = edit.final_cfg_facts()?;
            let candidate = MirBasicBlockMergeCandidate::unchecked(foreign_block, block(owner, 1));
            MirFinalCfgEdit::new(edit).merge_basic_blocks(&facts, candidate)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(
        foreign_error,
        MirRewriteError::ForeignIdentity {
            expected: owner,
            identity: MirLocalIdentity::Block(foreign_block),
        }
    );
}

#[test]
fn merge_capability_rechecks_static_publication_barriers() {
    let mut program = lower_source_to_final_mir(
        "class Globals { static value: i64 = 7; init() {} } fn main() -> i64 { return Globals.value; }",
    );
    let initializer = program
        .static_lifecycle
        .as_mut()
        .unwrap()
        .initializers_mut_for_test()
        .first_mut()
        .unwrap();
    let owner = initializer.callable();
    let permanent = initializer.publication.cleanup_entry;
    let predecessor = block(owner, initializer.body.blocks.len());
    initializer.body.blocks.push(MirBasicBlock {
        id: predecessor,
        instructions: vec![],
        terminator: Some(MirTerminator::Goto {
            target: permanent,
            span: initializer.span,
        }),
        span: initializer.span,
    });

    let verified = verify_final_mir(program).unwrap();
    let initializer = verified
        .program()
        .static_lifecycle
        .as_ref()
        .unwrap()
        .initializers()
        .first()
        .unwrap();
    let facts = final_cfg_facts_for_definition(initializer.into()).unwrap();
    let analysis = analyze_basic_block_merging(&facts);
    let barrier = analysis
        .barriers()
        .iter()
        .find(|barrier| barrier.predecessor() == predecessor)
        .unwrap();
    assert_eq!(
        barrier.kind(),
        MirBasicBlockMergeBarrierKind::SuccessorPermanentAttachment
    );
    let candidate = MirBasicBlockMergeCandidate::unchecked(predecessor, permanent);

    let error = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let current = edit.final_cfg_facts()?;
            MirFinalCfgEdit::new(edit).merge_basic_blocks(&current, candidate)?;
        }
        Ok(())
    })
    .unwrap_err();

    assert_eq!(
        error,
        MirRewriteError::StaleCallableSnapshot {
            callable: owner,
            subject: "basic-block merge candidate",
        }
    );
}

#[test]
fn capability_commit_failure_publishes_no_partially_forwarded_program() {
    let verified = verify_final_mir(forwarding_chain_program()).unwrap();
    let original = verified.program().clone();
    let owner = CallableId::Function(verified.entry_function);
    let capability = MirFinalPassCapability::new(verified);

    let error = match capability.rewrite_cfg(|callable, edit| {
        if callable == owner {
            let facts = edit.facts()?;
            let plan = analyze_empty_block_forwarding(&facts).plan().clone();
            edit.apply_empty_block_forwarding(&facts, &plan)?;
            edit.edit.remove_block(block(owner, 0))?;
        }
        Ok(())
    }) {
        Ok(_) => panic!("invalid body entry unexpectedly committed"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        MirPassFailure::Rewrite(MirRewriteError::InvalidReference {
            expected,
            identity: MirLocalIdentity::Block(_),
            site: MirLocalIdentitySite::BodyEntry,
            failure: MirReferenceFailure::Deleted,
        }) if expected == owner
    ));
    assert_eq!(original, forwarding_chain_program());
}

fn forwarding_chain_program() -> MirProgram {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 7; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    install_forwarding_chain(definition);
    program
}

fn forwarding_cycle_program() -> MirProgram {
    let mut program = forwarding_chain_program();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let owner = definition.callable();
    let span = definition.span;
    definition.body.blocks[2].terminator = Some(MirTerminator::Goto {
        target: block(owner, 1),
        span,
    });
    program
}

fn branch_forwarding_program() -> MirProgram {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 7; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let owner = definition.callable();
    let span = definition.span;
    let mut result = take_single_entry(definition);
    result.id = block(owner, 2);

    let condition = ValueId::new(owner, definition.values.len());
    definition.values.push(MirValue {
        id: condition,
        ty: MirType::Bool,
        span,
    });
    definition.body.entry = block(owner, 0);
    definition.body.blocks = vec![
        MirBasicBlock {
            id: block(owner, 0),
            instructions: vec![MirInstruction::Assign(MirAssignment {
                result: condition,
                rvalue: MirRvalue {
                    kind: MirRvalueKind::ConstantBool(true),
                    ty: MirType::Bool,
                },
                span,
            })],
            terminator: Some(MirTerminator::Branch {
                condition,
                true_target: block(owner, 1),
                false_target: block(owner, 1),
                span,
            }),
            span,
        },
        goto_block(owner, 1, 2, span),
        result,
    ];
    program
}

fn merge_program() -> MirProgram {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 7; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    install_merge_pair(definition);
    program
}

fn two_forwarding_functions() -> MirProgram {
    let mut program = lower_source_to_final_mir(
        "fn helper() -> i64 { return 8; } fn main() -> i64 { return 7; }",
    );
    let functions = program
        .definitions
        .iter()
        .map(|definition| definition.function)
        .collect::<Vec<_>>();
    for function in functions {
        install_forwarding_chain(program.definitions.get_mut_for_test(function).unwrap());
    }
    program
}

fn install_forwarding_chain(definition: &mut MirFunctionDefinition) {
    let owner = definition.callable();
    let span = definition.span;
    let mut result = take_single_entry(definition);
    result.id = block(owner, 3);
    definition.body.entry = block(owner, 0);
    definition.body.blocks = vec![
        goto_block(owner, 0, 1, span),
        goto_block(owner, 1, 2, span),
        goto_block(owner, 2, 3, span),
        result,
    ];
}

fn install_merge_pair(definition: &mut MirFunctionDefinition) {
    let owner = definition.callable();
    let span = definition.span;
    let mut successor = take_single_entry(definition);
    successor.id = block(owner, 1);
    let predecessor_value = ValueId::new(owner, definition.values.len());
    definition.values.push(MirValue {
        id: predecessor_value,
        ty: MirType::I64,
        span,
    });
    definition.body.entry = block(owner, 0);
    definition.body.blocks = vec![
        MirBasicBlock {
            id: block(owner, 0),
            instructions: vec![MirInstruction::Assign(MirAssignment {
                result: predecessor_value,
                rvalue: MirRvalue {
                    kind: MirRvalueKind::ConstantI64(99),
                    ty: MirType::I64,
                },
                span,
            })],
            terminator: Some(MirTerminator::Goto {
                target: block(owner, 1),
                span,
            }),
            span,
        },
        successor,
    ];
}

fn take_single_entry(definition: &mut MirFunctionDefinition) -> MirBasicBlock {
    assert_eq!(definition.body.blocks.len(), 1);
    definition.body.blocks.pop().unwrap()
}

fn goto_block(
    owner: CallableId,
    index: usize,
    target: usize,
    span: crate::source::Span,
) -> MirBasicBlock {
    MirBasicBlock {
        id: block(owner, index),
        instructions: vec![],
        terminator: Some(MirTerminator::Goto {
            target: block(owner, target),
            span,
        }),
        span,
    }
}

fn block(owner: CallableId, index: usize) -> BlockId {
    BlockId::new(owner, index)
}
