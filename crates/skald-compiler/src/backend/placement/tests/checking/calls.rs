use super::*;

#[test]
fn calls_kill_caller_values_but_explicit_save_and_reload_work() {
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let v = b.value(bits(), None).unwrap();
        let signature = context
            .binding(crate::backend::plan::test_fixtures::source(1))
            .unwrap()
            .signature_id();
        b.append(entry, Node::new(Op::Constant(val(v, bits())), target))
            .unwrap();
        b.append(
            entry,
            Node::new(
                Op::Call {
                    target: None,
                    signature,
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
        for saved in [false, true] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.ints[0]);
            operand(&mut draft, entry, 2, 0, target.ints[0]);
            if saved {
                let home = draft.storage(Storage {
                    representation: bits(),
                    bytes: 8,
                    alignment: 8,
                    purpose: StoragePurpose::Home(v.id()),
                    lifetime: StorageLifetime::WholeCallable,
                });
                let site = Site::Instruction {
                    block: entry.id(),
                    ordinal: 1,
                };
                draft.transfer(
                    TransferPoint::Before(site),
                    copy(
                        v.id(),
                        bits(),
                        Location::Resource(target.ints[0]),
                        Location::Storage(home),
                    ),
                );
                draft.transfer(
                    TransferPoint::After(site),
                    copy(
                        v.id(),
                        bits(),
                        Location::Storage(home),
                        Location::Resource(target.ints[0]),
                    ),
                );
            }
            let checked = check_with_round_oracle(draft, target);
            if saved {
                assert!(checked.is_ok(), "{:?}", checked.err());
            } else {
                reject(checked, CheckReason::MissingValue);
            }
        }
    });
}
#[test]
fn marshaling_cannot_overwrite_a_secured_indirect_target() {
    fixture(false, |context, lower, target| {
        let sig = context
            .binding(crate::backend::plan::test_fixtures::source(1))
            .unwrap()
            .signature_id();
        let code = Representation::new(RepresentationKind::CodeAddress(sig), 64).unwrap();
        let (mut b, entry) = begin(context, lower);
        let address = b.value(code, None).unwrap();
        let value = b.value(bits(), None).unwrap();
        b.append(entry, Node::new(Op::Address(val(address, code)), target))
            .unwrap();
        b.append(entry, Node::new(Op::Constant(val(value, bits())), target))
            .unwrap();
        b.append(
            entry,
            Node::new(
                Op::Call {
                    target: Some(val(address, code)),
                    signature: sig,
                },
                target,
            ),
        )
        .unwrap();
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for overwrite in [false, true] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.secured);
            operand(&mut draft, entry, 1, 0, target.ints[0]);
            operand(&mut draft, entry, 2, 0, target.secured);
            if overwrite {
                draft.transfer(
                    TransferPoint::Before(Site::Instruction {
                        block: entry.id(),
                        ordinal: 2,
                    }),
                    copy(
                        value.id(),
                        bits(),
                        Location::Resource(target.ints[0]),
                        Location::Resource(target.secured),
                    ),
                );
            }
            let checked = check_with_round_oracle(draft, target);
            if overwrite {
                reject(checked, CheckReason::MissingValue);
            } else {
                assert!(checked.is_ok(), "{:?}", checked.err());
            }
        }
    });
}
#[test]
fn call_clobbers_invalidate_outgoing_slots_and_scratch_kills_held_values() {
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let v = b.value(bits(), None).unwrap();
        let signature = context
            .binding(crate::backend::plan::test_fixtures::source(1))
            .unwrap()
            .signature_id();
        b.append(entry, Node::new(Op::Constant(val(v, bits())), target))
            .unwrap();
        b.append(
            entry,
            Node::new(
                Op::Call {
                    target: None,
                    signature,
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
        let mut draft = PlacementDraft::new(&selected);
        operand(&mut draft, entry, 0, 0, target.ints[0]);
        operand(&mut draft, entry, 2, 0, target.ints[0]);
        let slot = Location::Abi {
            signature,
            area: AbiArea::Outgoing,
            index: 0,
        };
        let site = Site::Instruction {
            block: entry.id(),
            ordinal: 1,
        };
        draft.transfer(
            TransferPoint::Before(site),
            copy(v.id(), bits(), Location::Resource(target.ints[0]), slot),
        );
        draft.transfer(
            TransferPoint::After(site),
            copy(v.id(), bits(), slot, Location::Resource(target.ints[0])),
        );
        reject(
            check_with_round_oracle(draft, target),
            CheckReason::MissingValue,
        );
    });
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let v = b.value(bits(), None).unwrap();
        let result = b.value(bits(), None).unwrap();
        b.append(entry, Node::new(Op::Constant(val(v, bits())), target))
            .unwrap();
        b.append(entry, Node::new(Op::Scratch(val(result, bits())), target))
            .unwrap();
        b.append(entry, Node::new(Op::Read(val(v, bits())), target))
            .unwrap();
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        let mut draft = PlacementDraft::new(&selected);
        operand(&mut draft, entry, 0, 0, target.ints[3]);
        operand(&mut draft, entry, 1, 0, target.ints[0]);
        operand(&mut draft, entry, 2, 0, target.ints[3]);
        let site = Site::Instruction {
            block: entry.id(),
            ordinal: 1,
        };
        for (slot, view) in [target.ints[1], target.ints[3]].into_iter().enumerate() {
            draft
                .assign(
                    Assignment::Scratch {
                        site,
                        group: 0,
                        slot,
                    },
                    Location::Resource(view),
                )
                .unwrap();
        }
        reject(
            check_with_round_oracle(draft, target),
            CheckReason::MissingValue,
        );
    });
}
