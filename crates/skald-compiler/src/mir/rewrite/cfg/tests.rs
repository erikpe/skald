use crate::{
    identity::{CallableId, ClassId, FunctionId, MethodId, StaticFieldId, StaticInitializerId},
    mir::{
        test_fixtures::empty_member_definition, BlockId, MirBasicBlock, MirBody,
        MirFunctionDefinition, MirPathCondition, MirStaticInitializerBody, MirStaticPublication,
        MirTerminator, MirType, PathConditionId, ValueId,
    },
    test_support::lower_source_to_mir,
};

use super::*;
use crate::mir::rewrite::{
    callable::MirCallablePackage, edit::MirCallableEdit, tests::representative_function,
    BlockPlacement,
};

#[test]
fn ordinary_function_and_member_use_the_body_entry_as_the_executable_root() {
    let function = simple_function();
    let function_facts = local_cfg_facts_for_definition((&function).into()).unwrap();
    let final_function_facts = final_cfg_facts_for_definition((&function).into()).unwrap();
    assert_eq!(function_facts.entry(), function.body.entry);
    assert!(function_facts.protected_roots().is_empty());
    assert!(function_facts.permanent_roots().is_empty());
    assert_eq!(function_facts.entry_reachable(), &[function.body.entry]);
    assert!(function_facts
        .block(function.body.entry)
        .unwrap()
        .is_entry());
    assert_eq!(final_function_facts, function_facts);

    let span = function.span;
    let class = ClassId::new(7);
    let callable = CallableId::Method(MethodId::new(class, 0));
    let member = empty_member_definition(callable, class, &[], span);
    let member_facts = local_cfg_facts_for_definition((&member).into()).unwrap();
    assert_eq!(member_facts.callable(), callable);
    assert_eq!(member_facts.entry(), member.body.entry);
    assert_eq!(member_facts.reachable(), &[member.body.entry]);
}

#[test]
fn static_publication_roots_protect_initialization_and_shutdown_regions() {
    let initializer = static_initializer();
    let owner = initializer.callable();
    let expected_roots = vec![
        MirProtectedBlockRoot {
            site: MirLocalIdentitySite::StaticPublicationInitializationExit,
            block: BlockId::new(owner, 1),
        },
        MirProtectedBlockRoot {
            site: MirLocalIdentitySite::StaticPublicationCleanupEntry,
            block: BlockId::new(owner, 2),
        },
    ];

    let dense = local_cfg_facts_for_definition((&initializer).into()).unwrap();
    let final_dense = final_cfg_facts_for_definition((&initializer).into()).unwrap();
    assert_eq!(dense.protected_roots(), expected_roots);
    assert_eq!(dense.permanent_roots(), expected_roots);
    assert!(dense.block(BlockId::new(owner, 0)).unwrap().is_entry());
    for attachment in [BlockId::new(owner, 1), BlockId::new(owner, 2)] {
        let facts = dense.block(attachment).unwrap();
        assert!(facts.is_protected_root());
        assert!(facts.is_permanent_attachment());
        assert!(!facts.is_entry());
    }
    assert_eq!(
        dense.protected_but_entry_unreachable(),
        &[BlockId::new(owner, 1), BlockId::new(owner, 2)]
    );
    assert_eq!(dense.unreachable(), &[]);
    assert_eq!(final_dense, dense);

    let mut package = MirCallablePackage::from_static_initializer(initializer).unwrap();
    let sparse = package.edit_mut().local_cfg_facts().unwrap();
    assert_eq!(sparse, dense);

    package
        .edit_mut()
        .remove_block(BlockId::new(owner, 2))
        .unwrap();
    assert_invalid_reference(
        package.edit_mut().local_cfg_facts().unwrap_err(),
        owner,
        BlockId::new(owner, 2),
        MirLocalIdentitySite::StaticPublicationCleanupEntry,
        MirReferenceFailure::Deleted,
    );
}

#[test]
fn every_path_and_logical_block_role_is_a_stable_protected_root() {
    let definition = representative_function();
    let owner = definition.callable();
    let block = |index| BlockId::new(owner, index);
    let facts = local_cfg_facts_for_definition((&definition).into()).unwrap();

    assert_eq!(
        facts.protected_roots(),
        &[
            root(MirLocalIdentitySite::PathCondition(0), block(1)),
            root(MirLocalIdentitySite::PathCondition(0), block(2)),
            root(MirLocalIdentitySite::PathCondition(0), block(2)),
            root(MirLocalIdentitySite::LogicalExpression(0), block(0)),
            root(MirLocalIdentitySite::LogicalExpression(0), block(0)),
            root(MirLocalIdentitySite::LogicalExpression(0), block(1)),
            root(MirLocalIdentitySite::LogicalExpression(0), block(1)),
            root(MirLocalIdentitySite::LogicalExpression(0), block(2)),
            root(MirLocalIdentitySite::LogicalExpression(0), block(2)),
        ]
    );
    assert!(facts.permanent_roots().is_empty());
    assert!(facts.block(block(1)).unwrap().is_protected_root());
    assert!(!facts.block(block(1)).unwrap().is_permanent_attachment());
    assert!(matches!(
        final_cfg_facts_for_definition((&definition).into()),
        Err(MirRewriteError::ConsumedProofRootInFinalCfg {
            site: MirLocalIdentitySite::PathCondition(0),
            block,
        }) if block == BlockId::new(owner, 1)
    ));
}

#[test]
fn zero_one_two_and_three_successor_families_preserve_semantic_order() {
    let mut definition = representative_function();
    let owner = definition.callable();
    let block = |index| BlockId::new(owner, index);
    let span = definition.span;
    definition
        .body
        .blocks
        .extend((3..5).map(|index| return_block(block(index), span)));

    let MirTerminator::BeginOptionalView {
        success_target,
        absent_target,
        overflow_target,
        ..
    } = definition.body.blocks[0]
        .terminator
        .as_mut()
        .expect("fixture has a terminator")
    else {
        panic!("fixture starts with a three-way optional-view terminator")
    };
    *success_target = block(1);
    *absent_target = block(2);
    *overflow_target = block(3);
    definition.body.blocks[1].terminator = Some(MirTerminator::Branch {
        condition: ValueId::new(owner, 0),
        true_target: block(3),
        false_target: block(4),
        span,
    });
    definition.body.blocks[2].terminator = Some(MirTerminator::Goto {
        target: block(4),
        span,
    });

    let facts = local_cfg_facts_for_definition((&definition).into()).unwrap();
    assert_eq!(
        facts.block(block(0)).unwrap().terminator_kind(),
        MirLocalCfgTerminatorKind::BeginOptionalView
    );
    assert_eq!(
        facts.block(block(0)).unwrap().instruction_count(),
        definition.body.blocks[0].instructions.len()
    );
    assert_eq!(
        facts.block(block(0)).unwrap().successors(),
        &[block(1), block(2), block(3)]
    );
    assert_eq!(
        facts.block(block(0)).unwrap().successor_edges(),
        &[
            edge(block(0), block(1), 0),
            edge(block(0), block(2), 1),
            edge(block(0), block(3), 2),
        ]
    );
    assert_eq!(
        facts.block(block(1)).unwrap().successors(),
        &[block(3), block(4)]
    );
    assert_eq!(facts.block(block(2)).unwrap().successors(), &[block(4)]);
    assert_eq!(facts.block(block(4)).unwrap().successors(), &[]);
    assert_eq!(
        facts.block(block(3)).unwrap().predecessor_edges(),
        &[edge(block(0), block(3), 2), edge(block(1), block(3), 0)]
    );
    assert_eq!(
        facts.block(block(4)).unwrap().predecessor_edges(),
        &[edge(block(1), block(4), 1), edge(block(2), block(4), 0)]
    );
}

#[test]
fn duplicate_successors_remain_distinct_edge_occurrences() {
    let mut definition = representative_function();
    let owner = definition.callable();
    let source = BlockId::new(owner, 0);
    let target = BlockId::new(owner, 1);
    let span = definition.span;
    definition.body.blocks[0].terminator = Some(MirTerminator::Branch {
        condition: ValueId::new(owner, 0),
        true_target: target,
        false_target: target,
        span,
    });

    let facts = local_cfg_facts_for_definition((&definition).into()).unwrap();
    let expected = [edge(source, target, 0), edge(source, target, 1)];
    assert_eq!(&facts.edges()[..2], &expected);
    assert_eq!(
        facts
            .edges()
            .iter()
            .filter(|edge| edge.source() == source)
            .copied()
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(facts.block(source).unwrap().successor_edges(), &expected);
    assert_eq!(facts.block(target).unwrap().predecessor_edges(), &expected);
    assert_eq!(expected[1].source(), source);
    assert_eq!(expected[1].target(), target);
    assert_eq!(expected[1].successor_index(), 1);
}

#[test]
fn terminator_shape_vocabulary_covers_every_successor_family() {
    use MirLocalCfgTerminatorKind as Kind;

    let shapes = [
        (Kind::Return, 0),
        (Kind::ReturnShared, 0),
        (Kind::ReturnOptionalShared, 0),
        (Kind::Panic, 0),
        (Kind::Goto, 1),
        (Kind::Branch, 2),
        (Kind::ShiftCountCheck, 2),
        (Kind::IntegerDivisorCheck, 2),
        (Kind::PrimitiveCastRangeCheck, 2),
        (Kind::CheckedCast, 2),
        (Kind::SharedCast, 2),
        (Kind::OptionalUnwrap, 2),
        (Kind::OptionalSharedUnwrap, 2),
        (Kind::BeginOptionalView, 3),
        (Kind::BeginOptionalBoxView, 3),
        (Kind::CheckOptionalMutation, 2),
        (Kind::ArrayPositionCheck, 2),
        (Kind::ArrayOperationCheck, 2),
        (Kind::ArrayLoop, 2),
        (Kind::Terminate, 0),
    ];

    for (kind, expected) in shapes {
        assert_eq!(kind.successor_count(), expected, "{kind:?}");
    }
}

#[test]
fn loops_disconnected_regions_and_protected_closure_are_distinguished() {
    let mut definition = representative_function();
    let owner = definition.callable();
    let block = |index| BlockId::new(owner, index);
    let span = definition.span;
    definition.values.clear();
    definition.body.blocks = vec![
        goto_block(block(0), block(1), span),
        goto_block(block(1), block(0), span),
        goto_block(block(2), block(3), span),
        return_block(block(3), span),
        return_block(block(4), span),
    ];
    definition.body.path_conditions = vec![MirPathCondition {
        id: PathConditionId::new(owner, 0),
        parent: None,
        activation: definition.storage[0].id,
        active_predecessor: block(2),
        inactive_predecessor: block(2),
        merge: block(2),
        span,
    }];
    definition.body.logical_expressions.clear();

    let facts = local_cfg_facts_for_definition((&definition).into()).unwrap();
    assert_eq!(facts.entry_reachable(), &[block(0), block(1)]);
    assert_eq!(facts.reachable(), &[block(0), block(1), block(2), block(3)]);
    assert_eq!(
        facts.protected_but_entry_unreachable(),
        &[block(2), block(3)]
    );
    assert_eq!(facts.unreachable(), &[block(4)]);
}

#[test]
fn block_values_come_from_the_shared_definition_census() {
    let definition = representative_function();
    let owner = definition.callable();
    let facts = local_cfg_facts_for_definition((&definition).into()).unwrap();
    assert_eq!(
        facts
            .block(BlockId::new(owner, 0))
            .unwrap()
            .defined_values(),
        &[
            ValueId::new(owner, 0), // assignment result
            ValueId::new(owner, 2), // call result
            ValueId::new(owner, 4), // I/O result
        ]
    );
    assert!(facts
        .block(BlockId::new(owner, 1))
        .unwrap()
        .defined_values()
        .is_empty());
}

#[test]
fn dense_and_sparse_edit_snapshots_produce_identical_facts() {
    let definition = representative_function();
    let dense = local_cfg_facts_for_definition((&definition).into()).unwrap();
    let edit = MirCallableEdit::from_dense_parts(
        definition.callable(),
        definition.storage.clone(),
        definition.values.clone(),
        definition.body.clone(),
    )
    .unwrap();
    let first = edit.local_cfg_facts().unwrap();
    let second = edit.local_cfg_facts().unwrap();
    assert_eq!(first, dense);
    assert_eq!(second, first);
}

#[test]
fn sparse_predecessors_follow_explicit_block_order_then_successor_order() {
    let mut definition = simple_function();
    let owner = definition.callable();
    let block = |index| BlockId::new(owner, index);
    let span = definition.span;
    definition
        .body
        .blocks
        .extend([return_block(block(1), span), return_block(block(2), span)]);
    let mut edit = MirCallableEdit::from_dense_parts(
        owner,
        definition.storage,
        definition.values,
        definition.body,
    )
    .unwrap();
    let inserted = edit
        .allocate_block(BlockPlacement::Before(block(1)), |id| {
            return_block(id, span)
        })
        .unwrap();
    for source in [block(0), inserted, block(1)] {
        edit.rewrite_block_terminator(source, |_| {
            Some(MirTerminator::Goto {
                target: block(2),
                span,
            })
        })
        .unwrap();
    }

    let facts = edit.local_cfg_facts().unwrap();
    assert_eq!(
        facts
            .blocks()
            .iter()
            .map(|facts| facts.block())
            .collect::<Vec<_>>(),
        vec![block(0), inserted, block(1), block(2)]
    );
    assert_eq!(
        facts.block(block(2)).unwrap().predecessor_edges(),
        &[
            edge(block(0), block(2), 0),
            edge(inserted, block(2), 0),
            edge(block(1), block(2), 0),
        ]
    );
}

#[test]
fn malformed_dense_declarations_roots_successors_and_terminators_are_structured() {
    let mut definition = simple_function();
    let owner = definition.callable();
    let span = definition.span;

    definition.body.blocks[0].id = BlockId::new(owner, 1);
    assert_eq!(
        local_cfg_facts_for_definition((&definition).into()),
        Err(MirRewriteError::DeclarationIdentityMismatch {
            expected: MirLocalIdentity::Block(BlockId::new(owner, 0)),
            actual: MirLocalIdentity::Block(BlockId::new(owner, 1)),
        })
    );

    let mut definition = simple_function();
    definition.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: BlockId::new(owner, 99),
        span,
    });
    assert_invalid_reference(
        local_cfg_facts_for_definition((&definition).into()).unwrap_err(),
        owner,
        BlockId::new(owner, 99),
        MirLocalIdentitySite::Terminator(0),
        MirReferenceFailure::Unknown,
    );

    let mut definition = simple_function();
    definition.body.entry = BlockId::new(CallableId::Function(FunctionId::new(99)), 0);
    assert_invalid_reference(
        local_cfg_facts_for_definition((&definition).into()).unwrap_err(),
        owner,
        definition.body.entry,
        MirLocalIdentitySite::BodyEntry,
        MirReferenceFailure::Foreign,
    );

    let mut definition = simple_function();
    definition.body.blocks[0].terminator = None;
    assert_eq!(
        local_cfg_facts_for_definition((&definition).into()),
        Err(MirRewriteError::MissingBlockTerminator {
            block: BlockId::new(owner, 0),
        })
    );

    let mut initializer = static_initializer();
    let static_owner = initializer.callable();
    initializer.publication.cleanup_entry = BlockId::new(static_owner, 99);
    assert_invalid_reference(
        local_cfg_facts_for_definition((&initializer).into()).unwrap_err(),
        static_owner,
        BlockId::new(static_owner, 99),
        MirLocalIdentitySite::StaticPublicationCleanupEntry,
        MirReferenceFailure::Unknown,
    );
}

#[test]
fn sparse_roots_distinguish_deleted_blocks_and_census_rejects_bad_values() {
    let mut definition = representative_function();
    let owner = definition.callable();
    let block = |index| BlockId::new(owner, index);
    let span = definition.span;
    definition.body.blocks[0].terminator = Some(MirTerminator::Return { value: None, span });
    definition.body.blocks[2].terminator = Some(MirTerminator::Return { value: None, span });
    let mut edit = MirCallableEdit::from_dense_parts(
        owner,
        definition.storage.clone(),
        definition.values.clone(),
        definition.body.clone(),
    )
    .unwrap();
    edit.remove_block(block(1)).unwrap();
    assert_invalid_reference(
        edit.local_cfg_facts().unwrap_err(),
        owner,
        block(1),
        MirLocalIdentitySite::PathCondition(0),
        MirReferenceFailure::Deleted,
    );

    let mut definition = representative_function();
    let duplicate = ValueId::new(owner, 0);
    let crate::mir::MirInstruction::Call(call) = &mut definition.body.blocks[0].instructions[2]
    else {
        panic!("fixture instruction is a call")
    };
    call.result = Some(duplicate);
    assert!(matches!(
        local_cfg_facts_for_definition((&definition).into()),
        Err(MirRewriteError::DuplicateValueDefinition { value, .. }) if value == duplicate
    ));

    let mut definition = representative_function();
    let unknown = ValueId::new(owner, 99);
    let MirTerminator::Return { value, .. } = definition.body.blocks[2]
        .terminator
        .as_mut()
        .expect("fixture return")
    else {
        panic!("fixture ends in a return")
    };
    *value = Some(unknown);
    assert!(matches!(
        local_cfg_facts_for_definition((&definition).into()),
        Err(MirRewriteError::InvalidReference {
            identity: MirLocalIdentity::Value(value),
            failure: MirReferenceFailure::Unknown,
            ..
        }) if value == unknown
    ));

    let mut definition = simple_function();
    definition.body.blocks.push(return_block(block(1), span));
    definition.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: block(1),
        span,
    });
    let mut edit = MirCallableEdit::from_dense_parts(
        owner,
        definition.storage,
        definition.values,
        definition.body,
    )
    .unwrap();
    edit.remove_block(block(1)).unwrap();
    assert_invalid_reference(
        edit.local_cfg_facts().unwrap_err(),
        owner,
        block(1),
        MirLocalIdentitySite::Terminator(0),
        MirReferenceFailure::Deleted,
    );

    let definition = representative_function();
    let mut edit = MirCallableEdit::from_dense_parts(
        owner,
        definition.storage,
        definition.values,
        definition.body,
    )
    .unwrap();
    edit.remove_value(ValueId::new(owner, 0)).unwrap();
    assert!(matches!(
        edit.local_cfg_facts(),
        Err(MirRewriteError::InvalidReference {
            identity: MirLocalIdentity::Value(value),
            failure: MirReferenceFailure::Deleted,
            ..
        }) if value == ValueId::new(owner, 0)
    ));
}

fn root(site: MirLocalIdentitySite, block: BlockId) -> MirProtectedBlockRoot {
    MirProtectedBlockRoot { site, block }
}

fn edge(source: BlockId, target: BlockId, successor_index: usize) -> MirLocalCfgEdge {
    MirLocalCfgEdge {
        source,
        target,
        successor_index,
    }
}

fn simple_function() -> MirFunctionDefinition {
    let program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let mut definition = program
        .definitions
        .get(program.entry_function)
        .expect("entry definition")
        .clone();
    definition.return_storage = None;
    definition.parameters.clear();
    definition.storage.clear();
    definition.values.clear();
    definition.body = MirBody {
        entry: BlockId::new(definition.callable(), 0),
        blocks: vec![return_block(
            BlockId::new(definition.callable(), 0),
            definition.span,
        )],
        path_conditions: vec![],
        logical_expressions: vec![],
    };
    definition
}

fn static_initializer() -> MirStaticInitializerBody {
    let span = simple_function().span;
    let field = StaticFieldId::new(ClassId::new(0), 0);
    let id = StaticInitializerId::from(field);
    let owner = CallableId::StaticInitializer(id);
    MirStaticInitializerBody {
        id,
        field,
        destination_type: MirType::I64,
        publication: MirStaticPublication {
            initialization_exit: BlockId::new(owner, 1),
            cleanup_entry: BlockId::new(owner, 2),
            span,
        },
        storage: vec![],
        values: vec![],
        body: MirBody {
            entry: BlockId::new(owner, 0),
            blocks: (0..3)
                .map(|index| return_block(BlockId::new(owner, index), span))
                .collect(),
            path_conditions: vec![],
            logical_expressions: vec![],
        },
        span,
    }
}

fn goto_block(id: BlockId, target: BlockId, span: crate::source::Span) -> MirBasicBlock {
    MirBasicBlock {
        id,
        instructions: vec![],
        terminator: Some(MirTerminator::Goto { target, span }),
        span,
    }
}

fn return_block(id: BlockId, span: crate::source::Span) -> MirBasicBlock {
    MirBasicBlock {
        id,
        instructions: vec![],
        terminator: Some(MirTerminator::Return { value: None, span }),
        span,
    }
}

fn assert_invalid_reference(
    error: MirRewriteError,
    expected: CallableId,
    block: BlockId,
    site: MirLocalIdentitySite,
    failure: MirReferenceFailure,
) {
    assert_eq!(
        error,
        MirRewriteError::InvalidReference {
            expected,
            identity: MirLocalIdentity::Block(block),
            site,
            failure,
        }
    );
}
