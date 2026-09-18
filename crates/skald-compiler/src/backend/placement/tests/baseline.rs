//! Independent simultaneous-copy reference and real CFG baseline acceptance.
use super::fixtures::*;
use crate::backend::{placement::*, selected::*};
use std::collections::BTreeMap;

#[test]
fn baseline_keeps_live_ties_and_loop_parameter_swaps_across_duplicate_edges() {
    for second in [false, true] {
        fixture(second, |context, lower, target| {
            let (mut b, entry) = begin(context, lower);
            let a = b.value(bits(), None).unwrap();
            let c = b.value(bits(), None).unwrap();
            let p = b.value(bits(), None).unwrap();
            let q = b.value(bits(), None).unwrap();
            let result = b.value(bits(), None).unwrap();
            let signature = context
                .binding(crate::backend::plan::test_fixtures::source(1))
                .unwrap()
                .signature_id();
            let code = Representation::new(RepresentationKind::CodeAddress(signature), 64).unwrap();
            let address = b.value(code, None).unwrap();
            let floating = b.value(float(), None).unwrap();
            let looping = b.block(&[p, q], None).unwrap();
            let exit = b.block(&[], None).unwrap();
            b.append(
                entry,
                Node::new(Op::Pair(val(a, bits()), val(c, bits())), target),
            )
            .unwrap();
            b.append(entry, Node::new(Op::Address(val(address, code)), target))
                .unwrap();
            b.append(
                entry,
                Node::new(Op::Constant(val(floating, float())), target),
            )
            .unwrap();
            b.terminate(
                entry,
                Node::new(Op::Jump(2), target),
                &[(looping, vec![a, c]), (looping, vec![a, c])],
            )
            .unwrap();
            b.append(
                looping,
                Node::new(
                    Op::Add {
                        input: val(p, bits()),
                        other: val(q, bits()),
                        result: val(result, bits()),
                        tied: true,
                    },
                    target,
                ),
            )
            .unwrap();
            b.append(looping, Node::new(Op::Read(val(p, bits())), target))
                .unwrap();
            b.terminate(
                looping,
                Node::new(Op::Jump(2), target),
                &[(looping, vec![q, p]), (exit, vec![])],
            )
            .unwrap();
            b.append(
                exit,
                Node::new(
                    Op::Call {
                        target: Some(val(address, code)),
                        signature,
                    },
                    target,
                ),
            )
            .unwrap();
            b.append(exit, Node::new(Op::Read(val(p, bits())), target))
                .unwrap();
            b.append(exit, Node::new(Op::Read(val(floating, float())), target))
                .unwrap();
            b.terminate(exit, Node::new(Op::Return, target), &[])
                .unwrap();
            let selected = verify_selected(b.finish(), target).ok().unwrap();
            let checked = place_baseline(&selected, target).unwrap_or_else(|e| panic!("{e:?}"));
            assert!(checked
                .storage()
                .iter()
                .any(|s| s.purpose == StoragePurpose::TransferScratch));
            assert!(!checked
                .transfers(TransferPoint::Edge {
                    block: entry.id(),
                    slot: 0
                })
                .is_empty());
            assert!(!checked
                .transfers(TransferPoint::Edge {
                    block: entry.id(),
                    slot: 1
                })
                .is_empty());
        });
    }
}
#[test]
fn resolved_cycles_match_simultaneous_bits_and_are_construction_order_independent() {
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let values: Vec<_> = (0..4).map(|_| b.value(bits(), None).unwrap()).collect();
        for &v in &values {
            b.append(entry, Node::new(Op::Constant(val(v, bits())), target))
                .unwrap();
        }
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for memory in [false, true] {
            for count in [2, 3] {
                let mut canonical = None;
                let orders = if count == 2 {
                    vec![vec![0, 1], vec![1, 0]]
                } else {
                    vec![
                        vec![0, 1, 2],
                        vec![0, 2, 1],
                        vec![1, 0, 2],
                        vec![1, 2, 0],
                        vec![2, 0, 1],
                        vec![2, 1, 0],
                    ]
                };
                for order in orders {
                    let mut draft = PlacementDraft::new(&selected);
                    let locations: Vec<_> = (0..count)
                        .map(|i| {
                            if memory {
                                Location::Storage(draft.storage(Storage {
                                    representation: bits(),
                                    bytes: 8,
                                    alignment: 8,
                                    purpose: StoragePurpose::Home(values[i].id()),
                                    lifetime: StorageLifetime::WholeCallable,
                                }))
                            } else {
                                Location::Resource(target.ints[i])
                            }
                        })
                        .collect();
                    let initial: BTreeMap<_, _> = locations
                        .iter()
                        .enumerate()
                        .map(|(i, &l)| (l, 0x7ff8_0000_0000_0001u64 + i as u64))
                        .collect();
                    let expected: BTreeMap<_, _> = (0..count)
                        .map(|i| (locations[(i + 1) % count], initial[&locations[i]]))
                        .collect();
                    let moves: Vec<_> = order
                        .into_iter()
                        .map(|i| {
                            copy(
                                values[i].id(),
                                bits(),
                                locations[i],
                                locations[(i + 1) % count],
                            )
                        })
                        .collect();
                    let point = TransferPoint::Before(Site::Terminal(entry.id()));
                    crate::backend::placement::transfers::resolve(&mut draft, target, point, moves)
                        .unwrap();
                    let sequence = &draft.transfers[&point];
                    let mut actual = initial;
                    for t in sequence {
                        let bits = actual[&t.source];
                        for &scratch in &t.scratch {
                            actual.remove(&Location::Resource(scratch));
                        }
                        actual.insert(t.destination, bits);
                    }
                    for (l, bits) in expected {
                        assert_eq!(actual[&l], bits);
                    }
                    let rendered = format!("{sequence:?}");
                    if let Some(expected) = &canonical {
                        assert_eq!(&rendered, expected);
                    } else {
                        canonical = Some(rendered);
                    }
                    assert!(draft
                        .storage
                        .iter()
                        .any(|s| s.purpose == StoragePurpose::TransferScratch
                            && s.lifetime == StorageLifetime::Transfer(point)));
                }
            }
        }
    });
}

#[test]
fn mixed_bank_cycle_preserves_bits_and_passes_the_same_checker() {
    for second in [false, true] {
        fixture(second, |context, lower, target| {
            let (mut b, entry) = begin(context, lower);
            let integer = b.value(bits(), None).unwrap();
            let floating = b.value(float(), None).unwrap();
            b.append(entry, Node::new(Op::Constant(val(integer, bits())), target))
                .unwrap();
            b.append(
                entry,
                Node::new(Op::Constant(val(floating, float())), target),
            )
            .unwrap();
            b.terminate(entry, Node::new(Op::Return, target), &[])
                .unwrap();
            let selected = verify_selected(b.finish(), target).ok().unwrap();
            for memory in [false, true] {
                let mut draft = PlacementDraft::new(&selected);
                operand(&mut draft, entry, 0, 0, target.ints[0]);
                operand(&mut draft, entry, 1, 0, target.low);
                let mut locations = [
                    Location::Resource(target.ints[0]),
                    Location::Resource(target.low),
                ];
                if memory {
                    for (i, (value, rep)) in [(integer, bits()), (floating, float())]
                        .into_iter()
                        .enumerate()
                    {
                        let home = Location::Storage(draft.storage(Storage {
                            representation: rep,
                            bytes: 8,
                            alignment: 8,
                            purpose: StoragePurpose::Home(value.id()),
                            lifetime: StorageLifetime::WholeCallable,
                        }));
                        draft.transfer(
                            TransferPoint::After(Site::Instruction {
                                block: entry.id(),
                                ordinal: i,
                            }),
                            copy(value.id(), rep, locations[i], home),
                        );
                        locations[i] = home;
                    }
                }
                let mut forward = copy(integer.id(), bits(), locations[0], locations[1]);
                forward.kind = TransferKind::Bitwise;
                forward.destination_representation = float();
                let mut backward = copy(floating.id(), float(), locations[1], locations[0]);
                backward.kind = TransferKind::Bitwise;
                backward.destination_representation = bits();
                let point = TransferPoint::Before(Site::Terminal(entry.id()));
                crate::backend::placement::transfers::resolve(
                    &mut draft,
                    target,
                    point,
                    vec![forward, backward],
                )
                .unwrap();
                let initial = BTreeMap::from([
                    (locations[0], 0x8000_0000_0000_0000u64),
                    (locations[1], 0x7ff8_1234_5678_90abu64),
                ]);
                let mut state = initial.clone();
                for transfer in &draft.transfers[&point] {
                    state.insert(transfer.destination, state[&transfer.source]);
                }
                assert_eq!(state[&locations[1]], initial[&locations[0]]);
                assert_eq!(state[&locations[0]], initial[&locations[1]]);
                for transfer in &draft.transfers[&point] {
                    for &scratch in &transfer.scratch {
                        assert!(target
                            .preserved_views()
                            .iter()
                            .all(|&view| !target.resources.overlaps(scratch, view).unwrap()));
                    }
                }
                save_promises(&mut draft, target, entry.id());
                assert!(check_placement(draft, target).is_ok());
            }
        });
    }
}
#[test]
fn working_scratch_preserves_identity_outputs_and_rejects_exhausted_resources() {
    fixture(false, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let values: Vec<_> = (0..6).map(|_| b.value(bits(), None).unwrap()).collect();
        for &v in &values {
            b.append(entry, Node::new(Op::Constant(val(v, bits())), target))
                .unwrap();
        }
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).ok().unwrap();
        for locked in [1, 4] {
            let mut draft = PlacementDraft::new(&selected);
            let locations: Vec<_> = (0..2)
                .map(|i| {
                    Location::Storage(draft.storage(Storage {
                        representation: bits(),
                        bytes: 8,
                        alignment: 8,
                        purpose: StoragePurpose::Home(values[i].id()),
                        lifetime: StorageLifetime::WholeCallable,
                    }))
                })
                .collect();
            let mut moves = vec![copy(values[0].id(), bits(), locations[0], locations[1])];
            for i in 0..locked {
                let location = Location::Resource(target.ints[i]);
                moves.push(copy(values[i + 2].id(), bits(), location, location));
            }
            let point = TransferPoint::Before(Site::Terminal(entry.id()));
            let result =
                crate::backend::placement::transfers::resolve(&mut draft, target, point, moves);
            if locked == 4 {
                assert_eq!(result.unwrap_err().reason, CheckReason::Scratch);
            } else {
                result.unwrap();
                assert!(!draft.transfers[&point][0].scratch.contains(&target.ints[0]));
            }
        }
    });
}
