use super::*;

#[test]
fn duplicate_edges_are_all_checked_and_joins_require_every_predecessor() {
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let v = b.value(bits(), None).unwrap();
        let p = b.value(bits(), None).unwrap();
        let exit = b.block(&[p], None).unwrap();
        b.append(entry, Node::new(Op::Constant(val(v, bits())), target))
            .unwrap();
        b.terminate(
            entry,
            Node::new(Op::Jump(2), target),
            &[(exit, vec![v]), (exit, vec![v])],
        )
        .unwrap();
        b.append(exit, Node::new(Op::Read(val(p, bits())), target))
            .unwrap();
        b.terminate(exit, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for count in [1, 2] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.ints[0]);
            operand(&mut draft, exit, 0, 0, target.ints[1]);
            draft
                .assign(
                    Assignment::Parameter {
                        block: exit.id(),
                        slot: 0,
                    },
                    Location::Resource(target.ints[1]),
                )
                .unwrap();
            for edge in 0..2 {
                draft
                    .assign(
                        Assignment::EdgeArgument {
                            block: entry.id(),
                            edge,
                            slot: 0,
                        },
                        Location::Resource(target.ints[0]),
                    )
                    .unwrap();
            }
            for slot in 0..count {
                draft.transfer(
                    TransferPoint::Edge {
                        block: entry.id(),
                        slot,
                    },
                    copy(
                        v.id(),
                        bits(),
                        Location::Resource(target.ints[0]),
                        Location::Resource(target.ints[1]),
                    ),
                );
            }
            let checked = check_placement(draft, target);
            if count == 2 {
                assert!(checked.is_ok(), "{:?}", checked.err());
            } else {
                reject(checked, CheckReason::MissingValue);
            }
        }
    });
}
#[test]
fn loop_swaps_rebind_parameters_simultaneously_and_forget_old_epochs() {
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let a = b.value(bits(), None).unwrap();
        let c = b.value(bits(), None).unwrap();
        let p = b.value(bits(), None).unwrap();
        let q = b.value(bits(), None).unwrap();
        let loop_block = b.block(&[p, q], None).unwrap();
        let exit = b.block(&[], None).unwrap();
        b.append(entry, Node::new(Op::Constant(val(a, bits())), target))
            .unwrap();
        b.append(entry, Node::new(Op::Constant(val(c, bits())), target))
            .unwrap();
        b.terminate(
            entry,
            Node::new(Op::Jump(1), target),
            &[(loop_block, vec![a, c])],
        )
        .unwrap();
        b.append(loop_block, Node::new(Op::Read(val(p, bits())), target))
            .unwrap();
        b.append(loop_block, Node::new(Op::Read(val(q, bits())), target))
            .unwrap();
        b.terminate(
            loop_block,
            Node::new(Op::Jump(2), target),
            &[(loop_block, vec![q, p]), (exit, vec![])],
        )
        .unwrap();
        b.append(exit, Node::new(Op::Read(val(p, bits())), target))
            .unwrap();
        b.terminate(exit, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for (resolve, stale) in [(false, false), (true, false), (true, true)] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.ints[0]);
            operand(&mut draft, entry, 1, 0, target.ints[1]);
            operand(&mut draft, loop_block, 0, 0, target.ints[0]);
            operand(&mut draft, loop_block, 1, 0, target.ints[1]);
            operand(
                &mut draft,
                exit,
                0,
                0,
                target.ints[if stale { 2 } else { 0 }],
            );
            for slot in 0..2 {
                draft
                    .assign(
                        Assignment::Parameter {
                            block: loop_block.id(),
                            slot,
                        },
                        Location::Resource(target.ints[slot]),
                    )
                    .unwrap();
                draft
                    .assign(
                        Assignment::EdgeArgument {
                            block: entry.id(),
                            edge: 0,
                            slot,
                        },
                        Location::Resource(target.ints[slot]),
                    )
                    .unwrap();
                draft
                    .assign(
                        Assignment::EdgeArgument {
                            block: loop_block.id(),
                            edge: 0,
                            slot,
                        },
                        Location::Resource(target.ints[1 - slot]),
                    )
                    .unwrap();
            }
            // A saved old p must not certify p after the next loop rebinding.
            draft.transfer(
                TransferPoint::Edge {
                    block: entry.id(),
                    slot: 0,
                },
                copy(
                    a.id(),
                    bits(),
                    Location::Resource(target.ints[0]),
                    Location::Resource(target.ints[2]),
                ),
            );
            if resolve {
                let point = TransferPoint::Edge {
                    block: loop_block.id(),
                    slot: 0,
                };
                let scratch = draft.storage(Storage {
                    representation: bits(),
                    bytes: 8,
                    alignment: 8,
                    purpose: StoragePurpose::TransferScratch,
                    lifetime: StorageLifetime::Transfer(point),
                });
                draft.transfer(
                    point,
                    copy(
                        p.id(),
                        bits(),
                        Location::Resource(target.ints[0]),
                        Location::Storage(scratch),
                    ),
                );
                draft.transfer(
                    point,
                    copy(
                        q.id(),
                        bits(),
                        Location::Resource(target.ints[1]),
                        Location::Resource(target.ints[0]),
                    ),
                );
                draft.transfer(
                    point,
                    copy(
                        p.id(),
                        bits(),
                        Location::Storage(scratch),
                        Location::Resource(target.ints[1]),
                    ),
                );
            }
            let checked = check_placement(draft, target);
            if resolve && !stale {
                assert!(checked.is_ok(), "{:?}", checked.err());
            } else {
                reject(checked, CheckReason::MissingValue);
            }
        }
    });
}

#[test]
fn loop_definitions_cannot_reuse_a_home_from_an_earlier_iteration() {
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let body = b.block(&[], None).unwrap();
        let exit = b.block(&[], None).unwrap();
        let v = b.value(bits(), None).unwrap();
        b.terminate(entry, Node::new(Op::Jump(1), target), &[(body, vec![])])
            .unwrap();
        b.append(body, Node::new(Op::Constant(val(v, bits())), target))
            .unwrap();
        b.terminate(
            body,
            Node::new(Op::Jump(2), target),
            &[(body, vec![]), (exit, vec![])],
        )
        .unwrap();
        b.append(exit, Node::new(Op::Read(val(v, bits())), target))
            .unwrap();
        b.terminate(exit, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for fresh in [false, true] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, body, 0, 0, target.ints[0]);
            operand(&mut draft, exit, 0, 0, target.ints[1]);
            let home = draft.storage(Storage {
                representation: bits(),
                bytes: 8,
                alignment: 8,
                purpose: StoragePurpose::Home(v.id()),
                lifetime: StorageLifetime::WholeCallable,
            });
            draft.transfer(
                TransferPoint::Edge {
                    block: body.id(),
                    slot: 0,
                },
                copy(
                    v.id(),
                    bits(),
                    Location::Resource(target.ints[0]),
                    Location::Storage(home),
                ),
            );
            if fresh {
                draft.transfer(
                    TransferPoint::Edge {
                        block: body.id(),
                        slot: 1,
                    },
                    copy(
                        v.id(),
                        bits(),
                        Location::Resource(target.ints[0]),
                        Location::Storage(home),
                    ),
                );
            }
            draft.transfer(
                TransferPoint::Before(Site::Instruction {
                    block: exit.id(),
                    ordinal: 0,
                }),
                copy(
                    v.id(),
                    bits(),
                    Location::Storage(home),
                    Location::Resource(target.ints[1]),
                ),
            );
            let checked = check_placement(draft, target);
            if fresh {
                assert!(checked.is_ok(), "{:?}", checked.err());
            } else {
                reject(checked, CheckReason::MissingValue);
            }
        }
    });
}

#[test]
fn a_divergent_predecessor_cannot_supply_a_join_value() {
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let left = b.block(&[], None).unwrap();
        let right = b.block(&[], None).unwrap();
        let join = b.block(&[], None).unwrap();
        let v = b.value(bits(), None).unwrap();
        let w = b.value(bits(), None).unwrap();
        b.append(entry, Node::new(Op::Constant(val(v, bits())), target))
            .unwrap();
        b.terminate(
            entry,
            Node::new(Op::Jump(2), target),
            &[(left, vec![]), (right, vec![])],
        )
        .unwrap();
        b.terminate(left, Node::new(Op::Jump(1), target), &[(join, vec![])])
            .unwrap();
        b.append(right, Node::new(Op::Constant(val(w, bits())), target))
            .unwrap();
        b.terminate(right, Node::new(Op::Jump(1), target), &[(join, vec![])])
            .unwrap();
        b.append(join, Node::new(Op::Read(val(v, bits())), target))
            .unwrap();
        b.terminate(join, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for view in [target.ints[0], target.ints[1]] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.ints[0]);
            operand(&mut draft, right, 0, 0, view);
            operand(&mut draft, join, 0, 0, target.ints[0]);
            let result = check_placement(draft, target);
            if view == target.ints[0] {
                reject(result, CheckReason::MissingValue);
            } else {
                assert!(result.is_ok());
            }
        }
    });
}

#[test]
fn parameter_coalescing_requires_proven_equal_current_arguments() {
    fixture(false, |context, lower, target| {
        for equal in [false, true] {
            let (mut b, entry) = begin(context, lower);
            let a = b.value(bits(), None).unwrap();
            let c = b.value(bits(), None).unwrap();
            let p = b.value(bits(), None).unwrap();
            let q = b.value(bits(), None).unwrap();
            let exit = b.block(&[p, q], None).unwrap();
            b.append(entry, Node::new(Op::Constant(val(a, bits())), target))
                .unwrap();
            b.append(entry, Node::new(Op::Constant(val(c, bits())), target))
                .unwrap();
            b.terminate(
                entry,
                Node::new(Op::Jump(1), target),
                &[(exit, vec![a, if equal { a } else { c }])],
            )
            .unwrap();
            for value in [p, q] {
                b.append(exit, Node::new(Op::Read(val(value, bits())), target))
                    .unwrap();
            }
            b.terminate(exit, Node::new(Op::Return, target), &[])
                .unwrap();
            let selected = verify_selected(b.finish(), target).ok().unwrap();
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.ints[0]);
            operand(&mut draft, entry, 1, 0, target.ints[1]);
            for slot in 0..2 {
                draft
                    .assign(
                        Assignment::Parameter {
                            block: exit.id(),
                            slot,
                        },
                        Location::Resource(target.ints[2]),
                    )
                    .unwrap();
                draft
                    .assign(
                        Assignment::EdgeArgument {
                            block: entry.id(),
                            edge: 0,
                            slot,
                        },
                        Location::Resource(target.ints[if equal { 0 } else { slot }]),
                    )
                    .unwrap();
                operand(&mut draft, exit, slot, 0, target.ints[2]);
            }
            let point = TransferPoint::Edge {
                block: entry.id(),
                slot: 0,
            };
            draft.transfer(
                point,
                copy(
                    a.id(),
                    bits(),
                    Location::Resource(target.ints[0]),
                    Location::Resource(target.ints[2]),
                ),
            );
            if !equal {
                draft.transfer(
                    point,
                    copy(
                        c.id(),
                        bits(),
                        Location::Resource(target.ints[1]),
                        Location::Resource(target.ints[2]),
                    ),
                );
            }
            let result = check_placement(draft, target);
            if equal {
                assert!(result.is_ok(), "{:?}", result.err());
            } else {
                reject(result, CheckReason::MissingValue);
            }
        }
    });
}
