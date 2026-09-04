use crate::{
    identity::{CallableId, ClassId, StaticFieldId, StaticInitializerId},
    mir::{
        BlockId, MirBasicBlock, MirBody, MirFunctionDefinition, MirInstruction,
        MirStaticInitializerBody, MirStaticPublication, MirStorage, MirStorageKind, MirStorageLive,
        MirTerminator, MirType, MirValue, StorageId, ValueId,
    },
    test_support::lower_source_to_mir,
};

use super::*;
use crate::mir::rewrite::{callable::MirCallablePackage, final_cfg_facts_for_definition};

#[derive(Clone, Copy)]
enum TerminatorShape {
    Return,
    Goto(usize),
    Branch(usize, usize),
}

#[test]
fn forwarding_resolves_single_and_transitive_empty_blocks_in_block_order() {
    let definition = function(&[
        TerminatorShape::Goto(1),
        TerminatorShape::Goto(2),
        TerminatorShape::Goto(3),
        TerminatorShape::Return,
    ]);
    let owner = definition.callable();
    let facts = final_cfg_facts_for_definition((&definition).into()).unwrap();

    let analysis = analyze_empty_block_forwarding(&facts);
    assert_eq!(
        candidate_blocks(&analysis),
        vec![block(owner, 1), block(owner, 2)]
    );
    assert_eq!(
        analysis.plan().target_for(block(owner, 1)),
        Some(block(owner, 3))
    );
    assert_eq!(
        analysis.plan().target_for(block(owner, 2)),
        Some(block(owner, 3))
    );
    assert_eq!(analysis.candidates()[0].direct_target(), block(owner, 2));
    assert_eq!(
        analysis.candidates()[0].incoming_edges(),
        facts.block(block(owner, 1)).unwrap().predecessor_edges()
    );
    assert_eq!(analysis.counts().examined_blocks(), 4);
    assert_eq!(analysis.counts().candidates(), 2);
    assert_eq!(analysis.counts().barriers(), 2);
}

#[test]
fn forwarding_keeps_self_loops_cycles_and_chains_entering_them() {
    let definition = function(&[
        TerminatorShape::Return,
        TerminatorShape::Goto(1),
        TerminatorShape::Goto(3),
        TerminatorShape::Goto(2),
        TerminatorShape::Goto(2),
        TerminatorShape::Goto(6),
        TerminatorShape::Return,
    ]);
    let owner = definition.callable();
    let facts = final_cfg_facts_for_definition((&definition).into()).unwrap();

    let analysis = analyze_empty_block_forwarding(&facts);
    assert_eq!(candidate_blocks(&analysis), vec![block(owner, 5)]);
    assert_eq!(
        analysis.plan().target_for(block(owner, 5)),
        Some(block(owner, 6))
    );
    assert_barrier(
        &analysis,
        block(owner, 1),
        MirEmptyBlockForwardingBarrierKind::SelfLoop,
    );
    for index in [2, 3] {
        assert_barrier(
            &analysis,
            block(owner, index),
            MirEmptyBlockForwardingBarrierKind::Cycle,
        );
    }
    assert_barrier(
        &analysis,
        block(owner, 4),
        MirEmptyBlockForwardingBarrierKind::LeadsToCycle,
    );
    assert_eq!(
        analysis
            .counts()
            .barriers_of_kind(MirEmptyBlockForwardingBarrierKind::Cycle),
        2
    );
}

#[test]
fn forwarding_classifies_entry_instructions_non_gotos_and_multiple_predecessors() {
    let mut definition = function(&[
        TerminatorShape::Branch(2, 2),
        TerminatorShape::Goto(2),
        TerminatorShape::Goto(3),
        TerminatorShape::Return,
    ]);
    let owner = definition.callable();
    make_instruction_bearing(&mut definition, 1);
    let before = definition.clone();
    let facts = final_cfg_facts_for_definition((&definition).into()).unwrap();

    let first = analyze_empty_block_forwarding(&facts);
    let second = analyze_empty_block_forwarding(&facts);
    assert_eq!(first, second);
    assert_eq!(definition, before, "candidate analysis must not mutate MIR");
    assert_barrier(
        &first,
        block(owner, 0),
        MirEmptyBlockForwardingBarrierKind::BodyEntry,
    );
    assert_barrier(
        &first,
        block(owner, 1),
        MirEmptyBlockForwardingBarrierKind::InstructionBearing,
    );
    assert_barrier(
        &first,
        block(owner, 3),
        MirEmptyBlockForwardingBarrierKind::NonGotoTerminator,
    );
    assert_eq!(
        first.plan().target_for(block(owner, 2)),
        Some(block(owner, 3))
    );
    assert_eq!(first.candidates()[0].incoming_edges().len(), 3);
}

#[test]
fn forwarding_respects_permanent_endpoints_and_incoming_mutation_barriers() {
    let initializer = static_initializer(&[
        TerminatorShape::Goto(3),
        TerminatorShape::Goto(3),
        TerminatorShape::Return,
        TerminatorShape::Goto(4),
        TerminatorShape::Goto(2),
    ]);
    let owner = initializer.callable();
    let facts = final_cfg_facts_for_definition((&initializer).into()).unwrap();
    let analysis = analyze_empty_block_forwarding(&facts);

    assert_barrier(
        &analysis,
        block(owner, 1),
        MirEmptyBlockForwardingBarrierKind::PermanentAttachment,
    );
    assert_barrier(
        &analysis,
        block(owner, 3),
        MirEmptyBlockForwardingBarrierKind::IncomingPermanentAttachment,
    );
    assert_eq!(
        analysis.plan().target_for(block(owner, 4)),
        Some(block(owner, 2))
    );
}

#[test]
fn merge_candidates_use_total_edge_occurrences_including_unreachable_sources() {
    let definition = function(&[
        TerminatorShape::Goto(1),
        TerminatorShape::Return,
        TerminatorShape::Branch(1, 1),
    ]);
    let owner = definition.callable();
    let facts = final_cfg_facts_for_definition((&definition).into()).unwrap();
    assert_eq!(facts.unreachable(), &[block(owner, 2)]);

    let analysis = analyze_basic_block_merging(&facts);
    assert!(analysis.candidates().is_empty());
    let barrier = merge_barrier_for(&analysis, block(owner, 0));
    assert_eq!(
        barrier.kind(),
        MirBasicBlockMergeBarrierKind::NonUniqueIncomingEdge
    );
    assert_eq!(barrier.incoming_edge_count(), Some(3));
    assert_eq!(
        merge_barrier_for(&analysis, block(owner, 2)).kind(),
        MirBasicBlockMergeBarrierKind::NonGotoPredecessor
    );
}

#[test]
fn merge_analysis_includes_unique_pairs_inside_unreachable_regions() {
    let definition = function(&[
        TerminatorShape::Return,
        TerminatorShape::Return,
        TerminatorShape::Goto(3),
        TerminatorShape::Return,
    ]);
    let owner = definition.callable();
    let facts = final_cfg_facts_for_definition((&definition).into()).unwrap();

    let analysis = analyze_basic_block_merging(&facts);
    assert_eq!(
        analysis.first_candidate(),
        Some(MirBasicBlockMergeCandidate {
            predecessor: block(owner, 2),
            successor: block(owner, 3),
        })
    );
}

#[test]
fn merge_selects_linear_pairs_by_current_block_order() {
    let definition = function(&[
        TerminatorShape::Goto(1),
        TerminatorShape::Goto(2),
        TerminatorShape::Return,
    ]);
    let owner = definition.callable();
    let facts = final_cfg_facts_for_definition((&definition).into()).unwrap();

    let analysis = analyze_basic_block_merging(&facts);
    assert_eq!(
        analysis.candidates(),
        &[
            MirBasicBlockMergeCandidate {
                predecessor: block(owner, 0),
                successor: block(owner, 1),
            },
            MirBasicBlockMergeCandidate {
                predecessor: block(owner, 1),
                successor: block(owner, 2),
            },
        ]
    );
    assert_eq!(
        analysis.first_candidate(),
        analysis.candidates().first().copied()
    );
    assert_eq!(analysis.counts().examined_blocks(), 3);
    assert_eq!(analysis.counts().candidates(), 2);
    assert_eq!(analysis.counts().barriers(), 1);
}

#[test]
fn merge_rejects_entry_successors_self_loops_branches_and_permanent_endpoints() {
    let two_block_loop = function(&[TerminatorShape::Goto(1), TerminatorShape::Goto(0)]);
    let owner = two_block_loop.callable();
    let facts = final_cfg_facts_for_definition((&two_block_loop).into()).unwrap();
    let analysis = analyze_basic_block_merging(&facts);
    assert_eq!(
        analysis.first_candidate(),
        Some(MirBasicBlockMergeCandidate {
            predecessor: block(owner, 0),
            successor: block(owner, 1),
        })
    );
    assert_eq!(
        merge_barrier_for(&analysis, block(owner, 1)).kind(),
        MirBasicBlockMergeBarrierKind::SuccessorIsEntry
    );

    let shapes = [
        TerminatorShape::Return,
        TerminatorShape::Goto(1),
        TerminatorShape::Branch(3, 3),
        TerminatorShape::Return,
    ];
    let definition = function(&shapes);
    let owner = definition.callable();
    let facts = final_cfg_facts_for_definition((&definition).into()).unwrap();
    let analysis = analyze_basic_block_merging(&facts);
    assert_eq!(
        merge_barrier_for(&analysis, block(owner, 1)).kind(),
        MirBasicBlockMergeBarrierKind::SelfLoop
    );
    assert_eq!(
        merge_barrier_for(&analysis, block(owner, 2)).kind(),
        MirBasicBlockMergeBarrierKind::NonGotoPredecessor
    );

    let initializer = static_initializer(&[
        TerminatorShape::Goto(1),
        TerminatorShape::Goto(3),
        TerminatorShape::Return,
        TerminatorShape::Return,
    ]);
    let owner = initializer.callable();
    let facts = final_cfg_facts_for_definition((&initializer).into()).unwrap();
    let analysis = analyze_basic_block_merging(&facts);
    assert_eq!(
        merge_barrier_for(&analysis, block(owner, 0)).kind(),
        MirBasicBlockMergeBarrierKind::SuccessorPermanentAttachment
    );
    assert_eq!(
        merge_barrier_for(&analysis, block(owner, 1)).kind(),
        MirBasicBlockMergeBarrierKind::PredecessorPermanentAttachment
    );
}

#[test]
fn merge_rescan_is_explicit_and_deterministic_after_an_edit() {
    let definition = function(&[
        TerminatorShape::Goto(1),
        TerminatorShape::Goto(2),
        TerminatorShape::Return,
    ]);
    let owner = definition.callable();
    let span = definition.span;
    let mut package = MirCallablePackage::from_function(definition).unwrap();

    let first_facts = package.edit_mut().final_cfg_facts().unwrap();
    assert_eq!(
        analyze_basic_block_merging(&first_facts)
            .first_candidate()
            .unwrap()
            .predecessor(),
        block(owner, 0)
    );

    package
        .edit_mut()
        .rewrite_block_terminator(block(owner, 0), |_| {
            Some(MirTerminator::Return { value: None, span })
        })
        .unwrap();
    let rescanned = analyze_basic_block_merging(&package.edit_mut().final_cfg_facts().unwrap());
    assert_eq!(
        rescanned.first_candidate().unwrap().predecessor(),
        block(owner, 1)
    );
    assert_eq!(
        rescanned.first_candidate().unwrap().successor(),
        block(owner, 2)
    );
}

fn function(shapes: &[TerminatorShape]) -> MirFunctionDefinition {
    let program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let mut definition = program
        .definitions
        .get(program.entry_function)
        .expect("entry definition")
        .clone();
    let owner = definition.callable();
    let span = definition.span;
    definition.return_storage = None;
    definition.parameters.clear();
    definition.storage.clear();
    definition.values = vec![MirValue {
        id: ValueId::new(owner, 0),
        ty: MirType::Bool,
        span,
    }];
    definition.body = MirBody {
        entry: block(owner, 0),
        blocks: shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| basic_block(owner, index, *shape, span))
            .collect(),
        path_conditions: vec![],
        logical_expressions: vec![],
    };
    definition
}

fn static_initializer(shapes: &[TerminatorShape]) -> MirStaticInitializerBody {
    let span = function(&[TerminatorShape::Return]).span;
    let field = StaticFieldId::new(ClassId::new(0), 0);
    let id = StaticInitializerId::from(field);
    let owner = CallableId::StaticInitializer(id);
    MirStaticInitializerBody {
        id,
        field,
        destination_type: MirType::I64,
        publication: MirStaticPublication {
            initialization_exit: block(owner, 1),
            cleanup_entry: block(owner, 2),
            span,
        },
        storage: vec![],
        values: vec![MirValue {
            id: ValueId::new(owner, 0),
            ty: MirType::Bool,
            span,
        }],
        body: MirBody {
            entry: block(owner, 0),
            blocks: shapes
                .iter()
                .enumerate()
                .map(|(index, shape)| basic_block(owner, index, *shape, span))
                .collect(),
            path_conditions: vec![],
            logical_expressions: vec![],
        },
        span,
    }
}

fn basic_block(
    owner: CallableId,
    index: usize,
    shape: TerminatorShape,
    span: crate::source::Span,
) -> MirBasicBlock {
    let terminator = match shape {
        TerminatorShape::Return => MirTerminator::Return { value: None, span },
        TerminatorShape::Goto(target) => MirTerminator::Goto {
            target: block(owner, target),
            span,
        },
        TerminatorShape::Branch(true_target, false_target) => MirTerminator::Branch {
            condition: ValueId::new(owner, 0),
            true_target: block(owner, true_target),
            false_target: block(owner, false_target),
            span,
        },
    };
    MirBasicBlock {
        id: block(owner, index),
        instructions: vec![],
        terminator: Some(terminator),
        span,
    }
}

fn make_instruction_bearing(definition: &mut MirFunctionDefinition, block_index: usize) {
    let owner = definition.callable();
    let storage = StorageId::new(owner, 0);
    definition.storage.push(MirStorage {
        id: storage,
        source: None,
        name: "candidate-barrier".to_owned(),
        kind: MirStorageKind::Temporary,
        ty: MirType::I64,
        span: definition.span,
    });
    definition.body.blocks[block_index]
        .instructions
        .push(MirInstruction::StorageLive(MirStorageLive {
            storage,
            span: definition.span,
        }));
}

fn block(owner: CallableId, index: usize) -> BlockId {
    BlockId::new(owner, index)
}

fn candidate_blocks(analysis: &MirEmptyBlockForwardingAnalysis) -> Vec<BlockId> {
    analysis
        .candidates()
        .iter()
        .map(MirEmptyBlockForwardingCandidate::block)
        .collect()
}

fn assert_barrier(
    analysis: &MirEmptyBlockForwardingAnalysis,
    block: BlockId,
    kind: MirEmptyBlockForwardingBarrierKind,
) {
    assert!(analysis
        .barriers()
        .iter()
        .any(|barrier| barrier.block() == block && barrier.kind() == kind));
}

fn merge_barrier_for(
    analysis: &MirBasicBlockMergeAnalysis,
    predecessor: BlockId,
) -> MirBasicBlockMergeBarrier {
    analysis
        .barriers()
        .iter()
        .copied()
        .find(|barrier| barrier.predecessor() == predecessor)
        .expect("predecessor has a merge barrier")
}
