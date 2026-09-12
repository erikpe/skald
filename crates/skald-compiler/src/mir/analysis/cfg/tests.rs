use std::collections::BTreeSet;

use crate::{
    identity::{CallableId, FunctionId},
    mir::{BlockId, MirTerminator},
    test_support::lower_source_to_mir,
};

use super::{MirCfgEdge, MirCfgTopology};

#[test]
fn edge_occurrences_and_predecessor_blocks_have_distinct_semantics() {
    let owner = owner(0);
    let block = |index| BlockId::new(owner, index);
    let topology = sparse_topology(
        owner,
        block(0),
        [
            (block(0), vec![block(1), block(1)]),
            (block(1), vec![block(2)]),
            (block(2), vec![]),
        ],
    );

    assert_eq!(
        topology.edges(),
        &[
            edge(block(0), block(1), 0),
            edge(block(0), block(1), 1),
            edge(block(1), block(2), 0),
        ]
    );
    assert_eq!(
        topology.block(block(0)).unwrap().successor_edges(),
        &[edge(block(0), block(1), 0), edge(block(0), block(1), 1)]
    );
    assert_eq!(
        topology.block(block(1)).unwrap().predecessor_edges(),
        &[edge(block(0), block(1), 0), edge(block(0), block(1), 1)]
    );
    assert_eq!(
        topology.block(block(1)).unwrap().predecessor_blocks(),
        &BTreeSet::from([block(0)])
    );
}

#[test]
fn reachability_handles_loops_self_edges_and_disconnected_components() {
    let owner = owner(0);
    let block = |index| BlockId::new(owner, index);
    let topology = sparse_topology(
        owner,
        block(0),
        [
            (block(0), vec![block(1)]),
            (block(1), vec![block(0)]),
            (block(2), vec![block(2)]),
            (block(3), vec![]),
        ],
    );

    assert_eq!(
        topology.entry_reachable(),
        &BTreeSet::from([block(0), block(1)])
    );
    assert_eq!(
        topology.reachable_from([block(2)]),
        BTreeSet::from([block(2)])
    );
    assert_eq!(
        topology.reachable_from([block(1), block(2)]),
        BTreeSet::from([block(0), block(1), block(2)])
    );
}

#[test]
fn malformed_sparse_nodes_and_targets_remain_tolerant() {
    let callable = owner(0);
    let foreign_owner = owner(1);
    let block = |index| BlockId::new(callable, index);
    let foreign = BlockId::new(foreign_owner, 0);
    let unknown = block(99);
    let topology = sparse_topology(
        callable,
        block(0),
        [
            (block(0), vec![block(2), unknown, foreign]),
            (foreign, vec![block(0)]),
            (block(1), vec![]),
            (block(1), vec![]),
            (block(2), vec![]),
        ],
    );

    assert_eq!(topology.edges().len(), 4);
    assert_eq!(topology.edges()[0].target(), block(2));
    assert_eq!(topology.edges()[1].target(), unknown);
    assert_eq!(topology.edges()[2].target(), foreign);
    assert!(topology.block(foreign).is_none());
    assert!(topology.block(block(1)).is_none());
    assert_eq!(
        topology.entry_reachable(),
        &BTreeSet::from([block(0), block(2)])
    );
    assert_eq!(
        topology.predecessor_blocks(block(2)),
        Some(&BTreeSet::from([block(0)]))
    );
    assert_eq!(
        topology.block(block(0)).unwrap().predecessor_edges(),
        &[edge(foreign, block(0), 0)]
    );
    assert_eq!(
        topology.block(block(0)).unwrap().predecessor_blocks(),
        &BTreeSet::new()
    );
    assert_eq!(
        topology.predecessor_blocks(block(0)),
        Some(&BTreeSet::from([foreign]))
    );
    assert_eq!(
        topology.predecessor_blocks(unknown),
        Some(&BTreeSet::from([block(0)]))
    );
    assert!(topology.reachable_from([foreign, unknown]).is_empty());

    let invalid_entry = sparse_topology(callable, unknown, [(block(0), vec![])]);
    assert!(invalid_entry.entry_reachable().is_empty());
}

#[test]
fn dense_definition_construction_tolerates_missing_and_invalid_structure() {
    let program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let mut definition = program
        .definitions
        .get(program.entry_function)
        .expect("entry definition")
        .clone();
    let owner = definition.callable();
    let entry = definition.body.entry;

    definition.body.blocks[0].terminator = None;
    let topology = MirCfgTopology::for_definition((&definition).into());
    assert_eq!(topology.blocks().len(), 1);
    assert!(topology.edges().is_empty());
    assert_eq!(topology.entry_reachable(), &BTreeSet::from([entry]));

    definition.body.blocks[0].id = BlockId::new(owner, 1);
    let topology = MirCfgTopology::for_definition((&definition).into());
    assert!(topology.block(BlockId::new(owner, 1)).is_none());
    assert!(topology.entry_reachable().is_empty());

    definition.body.blocks[0].id = entry;
    definition.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: BlockId::new(owner, 99),
        span: definition.span,
    });
    let first = MirCfgTopology::for_definition((&definition).into());
    let second = MirCfgTopology::for_definition((&definition).into());
    assert_eq!(first, second);
    assert_eq!(first.edges(), &[edge(entry, BlockId::new(owner, 99), 0)]);
    assert_eq!(first.entry_reachable(), &BTreeSet::from([entry]));
}

fn sparse_topology(
    owner: CallableId,
    entry: BlockId,
    blocks: impl IntoIterator<Item = (BlockId, Vec<BlockId>)>,
) -> MirCfgTopology {
    MirCfgTopology::from_ordered_successors(owner, entry, blocks)
}

fn owner(index: usize) -> CallableId {
    CallableId::Function(FunctionId::new(index))
}

fn edge(source: BlockId, target: BlockId, successor_index: usize) -> MirCfgEdge {
    MirCfgEdge::new(source, target, successor_index)
}
