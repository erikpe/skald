//! Bounded production/round-oracle comparison over genuine selected graphs.
use super::{
    super::requirements::Requirements, super::*, check_with_round_oracle, fixtures::*,
    oracle::observe_round_solver,
};
use crate::backend::selected::*;

#[test]
fn empty_token_graph_reaches_successors_when_first_propagation_is_top() {
    fixture(false, |context, lower, target| {
        let (mut builder, entry) = begin(context, lower);
        let exit = builder.block(&[], None).unwrap();
        builder
            .terminate(entry, Node::new(Op::Jump(1), target), &[(exit, vec![])])
            .unwrap();
        builder
            .terminate(exit, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(builder.finish(), target).ok().unwrap();

        for _ in 0..2 {
            assert!(check_with_round_oracle(PlacementDraft::new(&selected), target).is_ok());
        }
    });
}

#[test]
fn entry_backedge_keeps_the_fixed_seed_and_unreachable_blocks_grant_no_facts() {
    fixture(false, |context, lower, target| {
        let (mut builder, entry) = begin(context, lower);
        let exit = builder.block(&[], None).unwrap();
        let unreachable = builder.block(&[], None).unwrap();
        let value = builder.value(bits(), None).unwrap();
        let dead = builder.value(bits(), None).unwrap();
        builder
            .append(entry, Node::new(Op::Constant(val(value, bits())), target))
            .unwrap();
        builder
            .terminate(entry, Node::new(Op::Jump(1), target), &[(exit, vec![])])
            .unwrap();
        builder
            .append(exit, Node::new(Op::Read(val(value, bits())), target))
            .unwrap();
        builder
            .terminate(exit, Node::new(Op::Return, target), &[])
            .unwrap();
        builder
            .append(
                unreachable,
                Node::new(Op::Constant(val(dead, bits())), target),
            )
            .unwrap();
        builder
            .append(unreachable, Node::new(Op::Read(val(dead, bits())), target))
            .unwrap();
        builder
            .terminate(unreachable, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(builder.finish(), target).ok().unwrap();
        let placement = || {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.ints[0]);
            operand(&mut draft, exit, 0, 0, target.ints[0]);
            operand(&mut draft, unreachable, 0, 0, target.ints[1]);
            operand(&mut draft, unreachable, 1, 0, target.ints[1]);
            draft
        };
        assert!(check_with_round_oracle(placement(), target).is_ok());

        // Verified selected graphs reject entry predecessors. Inject this
        // solver counterexample after ordinary legality so both schedules keep
        // protecting the fixed entry seed if that upstream invariant changes.
        let draft = placement();
        let mut requirements = Requirements::collect(&draft, target).unwrap();
        requirements.validate(&draft, target).unwrap();
        requirements.seed(&draft).unwrap();
        requirements
            .blocks
            .get_mut(&entry.id())
            .unwrap()
            .edges
            .push((entry.id(), vec![]));
        let production = requirements.observe_solver(&draft);
        let oracle = observe_round_solver(&requirements, &draft);
        assert_eq!(production, oracle);
        assert!(production.outcome.is_ok());
    });
}

#[test]
fn bounded_generated_diamonds_match_fixed_points_and_failures() {
    fixture(false, |context, lower, target| {
        for seed in 0..32usize {
            let (mut builder, entry) = begin(context, lower);
            let left = builder.block(&[], None).unwrap();
            let right = builder.block(&[], None).unwrap();
            let join = builder.block(&[], None).unwrap();
            let value = builder.value(bits(), None).unwrap();
            let branch_value = builder.value(bits(), None).unwrap();
            builder
                .append(entry, Node::new(Op::Constant(val(value, bits())), target))
                .unwrap();
            builder
                .terminate(
                    entry,
                    Node::new(Op::Jump(2), target),
                    &[(left, vec![]), (right, vec![])],
                )
                .unwrap();
            builder
                .append(
                    left,
                    Node::new(Op::Constant(val(branch_value, bits())), target),
                )
                .unwrap();
            builder
                .terminate(left, Node::new(Op::Jump(1), target), &[(join, vec![])])
                .unwrap();
            builder
                .terminate(right, Node::new(Op::Jump(1), target), &[(join, vec![])])
                .unwrap();
            builder
                .append(join, Node::new(Op::Read(val(value, bits())), target))
                .unwrap();
            builder
                .terminate(join, Node::new(Op::Return, target), &[])
                .unwrap();
            let selected = verify_selected(builder.finish(), target).ok().unwrap();
            let clobbers_value = seed % 3 == 0;

            let placement = || {
                let mut draft = PlacementDraft::new(&selected);
                operand(&mut draft, entry, 0, 0, target.ints[0]);
                operand(
                    &mut draft,
                    left,
                    0,
                    0,
                    target.ints[usize::from(!clobbers_value)],
                );
                operand(&mut draft, join, 0, 0, target.ints[0]);
                draft
            };
            let first = check_with_round_oracle(placement(), target).map(|_| ());
            let second = check_with_round_oracle(placement(), target).map(|_| ());
            assert_eq!(first, second, "seed {seed} was nondeterministic");
            assert_eq!(first.is_err(), clobbers_value, "seed {seed}");
        }
    });
}

#[test]
fn oracle_and_production_capacity_dispositions_match() {
    let cases = [
        (0, 0, 0),
        (1, 0, 0),
        (3, 4, 5),
        (usize::MAX, 2, 1),
        (1, usize::MAX, 2),
        (1, 1, usize::MAX),
    ];
    for (blocks, locations, tokens) in cases {
        assert_eq!(
            super::super::check::iteration_bound(blocks, locations, tokens),
            super::oracle::oracle_iteration_bound(blocks, locations, tokens),
        );
    }
}
