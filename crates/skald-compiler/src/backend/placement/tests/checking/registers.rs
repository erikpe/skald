use super::*;

#[test]
fn register_only_acceptance_and_snapshot_authority() {
    fixture(false, |context, lower, target| {
        let build = || {
            let (mut b, entry) = begin(context, lower);
            let v = b.value(bits(), None).unwrap();
            b.append(entry, Node::new(Op::Constant(val(v, bits())), target))
                .unwrap();
            b.append(entry, Node::new(Op::Read(val(v, bits())), target))
                .unwrap();
            b.terminate(entry, Node::new(Op::Return, target), &[])
                .unwrap();
            (verify_selected(b.finish(), target).ok().unwrap(), entry)
        };
        let (selected, entry) = build();
        let (replacement, _) = build();
        for view in [target.ints[0], target.ints[1], target.reserved] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.ints[0]);
            operand(&mut draft, entry, 1, 0, view);
            if view == target.ints[0] {
                let checked = check_placement(draft, target).unwrap();
                assert!(std::ptr::eq(checked.selected(), &selected));
                assert!(checked.storage().is_empty());
                assert!(checked.transfers(TransferPoint::Entry).is_empty());
                assert_eq!(
                    checked.assignment(Assignment::Operand {
                        site: Site::Instruction {
                            block: entry.id(),
                            ordinal: 1
                        },
                        slot: 0
                    }),
                    Some(Location::Resource(view))
                );
                assert_eq!(checked.require_selected(&selected), Ok(()));
                assert_eq!(
                    checked.require_selected(&replacement),
                    Err(PlacementError::WrongSnapshot)
                );
            } else {
                reject(
                    check_placement(draft, target),
                    if view == target.reserved {
                        CheckReason::Reserved
                    } else {
                        CheckReason::MissingValue
                    },
                );
            }
        }
        let mut wrong = target.clone();
        wrong.profile.architecture = crate::backend::plan::Architecture::Aarch64;
        let mut draft = PlacementDraft::new(&selected);
        operand(&mut draft, entry, 0, 0, target.ints[0]);
        operand(&mut draft, entry, 1, 0, target.ints[0]);
        reject(check_placement(draft, &wrong), CheckReason::WrongTarget);
    });
}
#[test]
fn live_tied_input_requires_an_actual_copy_and_exact_tie() {
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let v = b.value(bits(), None).unwrap();
        let w = b.value(bits(), None).unwrap();
        let result = b.value(bits(), None).unwrap();
        for value in [v, w] {
            b.append(entry, Node::new(Op::Constant(val(value, bits())), target))
                .unwrap();
        }
        b.append(
            entry,
            Node::new(
                Op::Add {
                    input: val(v, bits()),
                    other: val(w, bits()),
                    result: val(result, bits()),
                    tied: true,
                },
                target,
            ),
        )
        .unwrap();
        b.append(entry, Node::new(Op::Read(val(v, bits())), target))
            .unwrap();
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for (copy_input, untie) in [(false, false), (true, false), (true, true)] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.ints[0]);
            operand(&mut draft, entry, 1, 0, target.ints[1]);
            for (slot, view) in [
                target.ints[0],
                target.ints[1],
                target.ints[usize::from(untie) * 3],
            ]
            .into_iter()
            .enumerate()
            {
                operand(&mut draft, entry, 2, slot, view);
            }
            operand(&mut draft, entry, 3, 0, target.ints[2]);
            if copy_input {
                draft.transfer(
                    TransferPoint::Before(Site::Instruction {
                        block: entry.id(),
                        ordinal: 2,
                    }),
                    copy(
                        v.id(),
                        bits(),
                        Location::Resource(target.ints[0]),
                        Location::Resource(target.ints[2]),
                    ),
                );
            }
            let checked = check_placement(draft, target);
            if untie {
                reject(checked, CheckReason::Tie);
            } else if copy_input {
                assert!(checked.is_ok());
            } else {
                reject(checked, CheckReason::MissingValue);
            }
        }
    });
}
#[test]
fn simultaneous_scratch_is_disjoint_and_invalid_unreachable_code_is_rejected() {
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let dead = b.block(&[], None).unwrap();
        let v = b.value(bits(), None).unwrap();
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        b.append(dead, Node::new(Op::Scratch(val(v, bits())), target))
            .unwrap();
        b.terminate(dead, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for view in [target.ints[0], target.ints[1], target.ints[2]] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, dead, 0, 0, target.ints[0]);
            let site = Site::Instruction {
                block: dead.id(),
                ordinal: 0,
            };
            draft
                .assign(
                    Assignment::Scratch {
                        site,
                        group: 0,
                        slot: 0,
                    },
                    Location::Resource(target.ints[1]),
                )
                .unwrap();
            draft
                .assign(
                    Assignment::Scratch {
                        site,
                        group: 0,
                        slot: 1,
                    },
                    Location::Resource(view),
                )
                .unwrap();
            let checked = check_placement(draft, target);
            if view == target.ints[2] {
                assert!(checked.is_ok());
            } else {
                reject(checked, CheckReason::Scratch);
            }
        }
    });
}
#[test]
fn narrow_writes_do_not_certify_wide_values() {
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let small = Representation::new(RepresentationKind::Bits, 8).unwrap();
        let v = b.value(bits(), None).unwrap();
        let w = b.value(small, None).unwrap();
        b.append(entry, Node::new(Op::Constant(val(v, bits())), target))
            .unwrap();
        b.append(entry, Node::new(Op::Constant(val(w, small)), target))
            .unwrap();
        b.append(entry, Node::new(Op::Read(val(v, bits())), target))
            .unwrap();
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        let mut draft = PlacementDraft::new(&selected);
        operand(&mut draft, entry, 0, 0, target.ints[0]);
        operand(&mut draft, entry, 1, 0, target.byte);
        operand(&mut draft, entry, 2, 0, target.ints[0]);
        reject(check_placement(draft, target), CheckReason::MissingValue);
    });
}

#[test]
fn early_definitions_precede_late_uses_and_same_phase_definitions_do_not_alias() {
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let input = b.value(bits(), None).unwrap();
        let output = b.value(bits(), None).unwrap();
        b.append(entry, Node::new(Op::Constant(val(input, bits())), target))
            .unwrap();
        b.append(
            entry,
            Node::new(
                Op::EarlyWrite {
                    input: val(input, bits()),
                    output: val(output, bits()),
                },
                target,
            ),
        )
        .unwrap();
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for view in [target.ints[0], target.ints[1]] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.ints[0]);
            operand(&mut draft, entry, 1, 0, target.ints[0]);
            operand(&mut draft, entry, 1, 1, view);
            let result = check_placement(draft, target);
            if view == target.ints[0] {
                reject(result, CheckReason::MissingValue);
            } else {
                assert!(result.is_ok());
            }
        }
    });
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let a = b.value(bits(), None).unwrap();
        let b_value = b.value(bits(), None).unwrap();
        b.append(
            entry,
            Node::new(Op::Pair(val(a, bits()), val(b_value, bits())), target),
        )
        .unwrap();
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for view in [target.ints[0], target.ints[1]] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.ints[0]);
            operand(&mut draft, entry, 0, 1, view);
            let result = check_placement(draft, target);
            if view == target.ints[0] {
                reject(result, CheckReason::Overlap);
            } else {
                assert!(result.is_ok());
            }
        }
    });
}

#[test]
fn structure_and_availability_failures_retain_selected_provenance() {
    fixture(false, |context, lower, target| {
        let mut sources = crate::source::SourceDatabase::new();
        let source = sources.add("checker.ska", "value");
        let origin = crate::source::Span::empty(source, 0);
        let (mut b, entry) = begin(context, lower);
        let exit = b.block(&[], Some(origin)).unwrap();
        let v = b.value(bits(), Some(origin)).unwrap();
        b.append(entry, Node::new(Op::Constant(val(v, bits())), target))
            .unwrap();
        b.terminate(entry, Node::new(Op::Jump(1), target), &[(exit, vec![])])
            .unwrap();
        b.append(exit, Node::new(Op::Read(val(v, bits())), target))
            .unwrap();
        b.terminate(exit, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for complete in [false, true] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.ints[0]);
            if complete {
                operand(&mut draft, exit, 0, 0, target.ints[1]);
            }
            let failure = check_placement(draft, target).err().unwrap();
            assert_eq!(failure.origin, Some(origin));
            assert_eq!(
                failure.location,
                CheckLocation::Assignment(Assignment::Operand {
                    site: Site::Instruction {
                        block: exit.id(),
                        ordinal: 0
                    },
                    slot: 0
                })
            );
            assert_eq!(
                failure.reason,
                if complete {
                    CheckReason::MissingValue
                } else {
                    CheckReason::Constraint
                }
            );
            assert_eq!(failure.structure.is_some(), !complete);
        }
    });
}
