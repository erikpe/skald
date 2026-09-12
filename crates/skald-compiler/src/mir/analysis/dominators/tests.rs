use crate::{
    identity::{CallableId, FunctionId},
    mir::BlockId,
    test_support::lower_source_to_mir,
};

use super::{MirCfgTopology, MirDominators};

#[test]
fn diamonds_and_multiple_paths_have_exact_dominators() {
    let owner = owner(0);
    let block = |index| BlockId::new(owner, index);
    let topology = topology(
        owner,
        block(0),
        [
            (block(0), vec![block(1), block(2)]),
            (block(1), vec![block(3)]),
            (block(2), vec![block(3)]),
            (block(3), vec![block(4)]),
            (block(4), vec![]),
        ],
    );
    let dominators = MirDominators::for_topology(&topology);

    for target in 0..5 {
        assert!(dominators.dominates(block(0), block(target)));
        assert!(dominators.dominates(block(target), block(target)));
    }
    assert!(!dominators.dominates(block(1), block(3)));
    assert!(!dominators.dominates(block(2), block(3)));
    assert!(dominators.dominates(block(3), block(4)));
}

#[test]
fn loops_self_edges_duplicate_edges_and_disconnected_blocks_are_stable() {
    let owner = owner(0);
    let block = |index| BlockId::new(owner, index);
    let topology = topology(
        owner,
        block(0),
        [
            (block(0), vec![block(1), block(1)]),
            (block(1), vec![block(2)]),
            (block(2), vec![block(1), block(2), block(3)]),
            (block(3), vec![]),
            (block(4), vec![block(4)]),
        ],
    );
    let first = MirDominators::for_topology(&topology);
    let second = MirDominators::for_topology(&topology);

    assert_eq!(first, second);
    assert!(first.dominates(block(0), block(3)));
    assert!(first.dominates(block(1), block(3)));
    assert!(first.dominates(block(2), block(3)));
    assert!(first.dominates(block(4), block(4)));
    assert!(!first.dominates(block(0), block(4)));
    assert!(!first.dominates(block(4), block(0)));
}

#[test]
fn malformed_identities_fail_closed() {
    let callable = owner(0);
    let foreign_owner = owner(1);
    let block = |index| BlockId::new(callable, index);
    let foreign = BlockId::new(foreign_owner, 0);
    let unknown = block(99);
    let topology = topology(
        callable,
        block(0),
        [
            (block(0), vec![block(2), unknown, foreign]),
            (block(1), vec![]),
            (block(1), vec![]),
            (block(2), vec![]),
            (foreign, vec![block(2)]),
        ],
    );
    let dominators = MirDominators::for_topology(&topology);

    assert!(dominators.dominates(block(0), block(2)));
    for invalid in [block(1), unknown, foreign] {
        assert!(!dominators.dominates(invalid, invalid));
        assert!(!dominators.dominates(invalid, block(2)));
        assert!(!dominators.dominates(block(0), invalid));
    }
}

#[test]
fn explicit_entry_and_dense_identity_validation_do_not_trust_block_indices() {
    let callable = owner(0);
    let block = |index| BlockId::new(callable, index);
    let topology = topology(
        callable,
        block(1),
        [
            (block(0), vec![]),
            (block(1), vec![block(2)]),
            (block(2), vec![]),
        ],
    );
    let dominators = MirDominators::for_topology(&topology);
    assert!(dominators.dominates(block(1), block(2)));
    assert!(!dominators.dominates(block(1), block(0)));
    assert!(dominators.dominates(block(0), block(0)));

    let program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let mut definition = program
        .definitions
        .get(program.entry_function)
        .expect("entry definition")
        .clone();
    let original = definition.body.entry;
    let misindexed = BlockId::new(definition.callable(), 1);
    definition.body.blocks[0].id = misindexed;
    let topology = MirCfgTopology::for_definition((&definition).into());
    let dominators = MirDominators::for_topology(&topology);
    assert!(!dominators.dominates(original, original));
    assert!(!dominators.dominates(misindexed, misindexed));
}

fn topology(
    owner: CallableId,
    entry: BlockId,
    blocks: impl IntoIterator<Item = (BlockId, Vec<BlockId>)>,
) -> MirCfgTopology {
    MirCfgTopology::from_ordered_successors(owner, entry, blocks)
}

fn owner(index: usize) -> CallableId {
    CallableId::Function(FunctionId::new(index))
}
