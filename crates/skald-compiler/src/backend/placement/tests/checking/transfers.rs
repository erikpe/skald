use super::*;

#[test]
fn second_target_preserves_only_promised_width_and_restores_link_register() {
    fixture(true, |context, lower, target| {
        for wide in [false, true] {
            let rep = if wide {
                Representation::new(RepresentationKind::Float, 128).unwrap()
            } else {
                float()
            };
            let (mut b, entry) = begin(context, lower);
            let v = b.value(rep, None).unwrap();
            let sig = context
                .binding(crate::backend::plan::test_fixtures::source(1))
                .unwrap()
                .signature_id();
            b.append(entry, Node::new(Op::Constant(val(v, rep)), target))
                .unwrap();
            b.append(
                entry,
                Node::new(
                    Op::Call {
                        target: None,
                        signature: sig,
                    },
                    target,
                ),
            )
            .unwrap();
            b.append(entry, Node::new(Op::Read(val(v, rep)), target))
                .unwrap();
            b.terminate(entry, Node::new(Op::Return, target), &[])
                .unwrap();
            let selected = verify_selected(b.finish(), target).ok().unwrap();
            let mut draft = PlacementDraft::new(&selected);
            let view = if wide { target.wide } else { target.low };
            operand(&mut draft, entry, 0, 0, view);
            operand(&mut draft, entry, 2, 0, view);
            save_promises(&mut draft, target, entry.id());
            let checked = check_with_round_oracle(draft, target);
            if wide {
                reject(checked, CheckReason::MissingValue);
            } else {
                assert!(checked.is_ok(), "{:?}", checked.err());
            }
        }
    });
}
#[test]
fn mixed_bank_cycles_need_a_real_typed_temporary() {
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let v = b.value(bits(), None).unwrap();
        let w = b.value(float(), None).unwrap();
        b.append(entry, Node::new(Op::Constant(val(v, bits())), target))
            .unwrap();
        b.append(entry, Node::new(Op::Constant(val(w, float())), target))
            .unwrap();
        b.append(entry, Node::new(Op::Read(val(v, bits())), target))
            .unwrap();
        b.append(entry, Node::new(Op::Read(val(w, float())), target))
            .unwrap();
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for saved in [false, true] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.ints[0]);
            operand(&mut draft, entry, 1, 0, target.fp);
            operand(&mut draft, entry, 2, 0, target.ints[1]);
            operand(&mut draft, entry, 3, 0, target.fp);
            let point = TransferPoint::Before(Site::Instruction {
                block: entry.id(),
                ordinal: 2,
            });
            let bitwise =
                |value, source, source_representation, destination, destination_representation| {
                    Transfer {
                        value: TransferValue::Selected(value),
                        source,
                        destination,
                        source_representation,
                        destination_representation,
                        kind: TransferKind::Bitwise,
                        scratch: vec![],
                    }
                };
            let temporary = draft.storage(Storage {
                representation: bits(),
                bytes: 8,
                alignment: 8,
                purpose: StoragePurpose::TransferScratch,
                lifetime: StorageLifetime::Transfer(point),
            });
            if saved {
                draft.transfer(
                    point,
                    copy(
                        v.id(),
                        bits(),
                        Location::Resource(target.ints[0]),
                        Location::Storage(temporary),
                    ),
                );
            }
            draft.transfer(
                point,
                bitwise(
                    w.id(),
                    Location::Resource(target.fp),
                    float(),
                    Location::Resource(target.ints[0]),
                    bits(),
                ),
            );
            draft.transfer(
                point,
                bitwise(
                    v.id(),
                    if saved {
                        Location::Storage(temporary)
                    } else {
                        Location::Resource(target.ints[0])
                    },
                    bits(),
                    Location::Resource(target.fp),
                    float(),
                ),
            );
            draft.transfer(
                point,
                bitwise(
                    v.id(),
                    Location::Resource(target.fp),
                    float(),
                    Location::Resource(target.ints[1]),
                    bits(),
                ),
            );
            draft.transfer(
                point,
                bitwise(
                    w.id(),
                    Location::Resource(target.ints[0]),
                    bits(),
                    Location::Resource(target.fp),
                    float(),
                ),
            );
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
fn saved_original_contents_and_transfer_lifetimes_are_independent_obligations() {
    fixture(true, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let v = b.value(float(), None).unwrap();
        b.append(entry, Node::new(Op::Constant(val(v, float())), target))
            .unwrap();
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for saved in [false, true] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.low);
            if saved {
                save_promises(&mut draft, target, entry.id());
            }
            let result = check_with_round_oracle(draft, target);
            if saved {
                assert!(result.is_ok());
            } else {
                reject(result, CheckReason::Preservation);
            }
        }
        let mut draft = PlacementDraft::new(&selected);
        operand(&mut draft, entry, 0, 0, target.low);
        save_promises(&mut draft, target, entry.id());
        let point = TransferPoint::After(Site::Instruction {
            block: entry.id(),
            ordinal: 0,
        });
        let temp = draft.storage(Storage {
            representation: float(),
            bytes: 8,
            alignment: 8,
            purpose: StoragePurpose::TransferScratch,
            lifetime: StorageLifetime::Transfer(point),
        });
        draft.transfer(
            TransferPoint::Before(Site::Terminal(entry.id())),
            copy(
                v.id(),
                float(),
                Location::Resource(target.low),
                Location::Storage(temp),
            ),
        );
        reject(
            check_with_round_oracle(draft, target),
            CheckReason::Lifetime,
        );
    });
}

#[test]
fn signature_keys_do_not_make_overlapping_abi_slots_disjoint() {
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let v = b.value(bits(), None).unwrap();
        let w = b.value(bits(), None).unwrap();
        for value in [v, w] {
            b.append(entry, Node::new(Op::Constant(val(value, bits())), target))
                .unwrap();
        }
        b.append(entry, Node::new(Op::Read(val(v, bits())), target))
            .unwrap();
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for index in [0, 1] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.ints[0]);
            operand(&mut draft, entry, 1, 0, target.ints[1]);
            operand(&mut draft, entry, 2, 0, target.ints[2]);
            let point = TransferPoint::After(Site::Instruction {
                block: entry.id(),
                ordinal: 1,
            });
            let slot = Location::Abi {
                signature: context
                    .catalog()
                    .plan()
                    .signatures_with_ids()
                    .next()
                    .unwrap()
                    .0,
                area: AbiArea::Outgoing,
                index: 0,
            };
            let other = Location::Abi {
                signature: context
                    .catalog()
                    .plan()
                    .signatures_with_ids()
                    .nth(1)
                    .unwrap()
                    .0,
                area: AbiArea::Outgoing,
                index,
            };
            draft.transfer(
                point,
                copy(v.id(), bits(), Location::Resource(target.ints[0]), slot),
            );
            draft.transfer(
                point,
                copy(w.id(), bits(), Location::Resource(target.ints[1]), other),
            );
            draft.transfer(
                point,
                copy(v.id(), bits(), slot, Location::Resource(target.ints[2])),
            );
            let result = check_with_round_oracle(draft, target);
            if index == 0 {
                reject(result, CheckReason::MissingValue);
            } else {
                assert!(result.is_ok());
            }
        }
    });
}

#[test]
fn original_preserved_bits_can_be_saved_and_restored_through_another_bank() {
    fixture(true, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let v = b.value(float(), None).unwrap();
        b.append(entry, Node::new(Op::Constant(val(v, float())), target))
            .unwrap();
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for restore in [None, Some(TransferKind::Copy), Some(TransferKind::Bitwise)] {
            let mut draft = PlacementDraft::new(&selected);
            operand(&mut draft, entry, 0, 0, target.low);
            let mut transfer = Transfer {
                value: TransferValue::Preserved(target.low),
                source: Location::Resource(target.low),
                destination: Location::Resource(target.ints[0]),
                source_representation: float(),
                destination_representation: bits(),
                kind: TransferKind::Bitwise,
                scratch: vec![],
            };
            draft.transfer(TransferPoint::Entry, transfer.clone());
            if let Some(kind) = restore {
                std::mem::swap(&mut transfer.source, &mut transfer.destination);
                std::mem::swap(
                    &mut transfer.source_representation,
                    &mut transfer.destination_representation,
                );
                transfer.kind = kind;
                draft.transfer(TransferPoint::Before(Site::Terminal(entry.id())), transfer);
            }
            let result = check_with_round_oracle(draft, target);
            match restore {
                None => reject(result, CheckReason::Preservation),
                Some(TransferKind::Copy) => assert!(matches!(
                    result.err().unwrap().structure.as_deref(),
                    Some(PlacementError::InvalidTransfer(_, 0))
                )),
                Some(TransferKind::Bitwise) => assert!(result.is_ok(), "{:?}", result.err()),
            }
        }
    });
}
