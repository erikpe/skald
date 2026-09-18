//! Checked manual placement exercises portable frame policy and declared recipes.
use super::fixtures::*;
use crate::backend::{
    frame::*,
    placement::*,
    plan::{LayoutDisposition, LayoutFact},
    selected::*,
};

#[test]
fn abi_object_publication_rejects_forged_extent_and_alignment() {
    fixture(true, |context, lower, target| {
        for (size, alignment) in [(16, 16), (32, 8)] {
            let (mut b, entry) = begin(context, lower);
            let signature = context
                .binding(crate::backend::plan::test_fixtures::source(0))
                .unwrap()
                .signature_id();
            b.object(
                LayoutFact {
                    size,
                    alignment,
                    disposition: LayoutDisposition::Addressable,
                },
                ObjectRole::Abi {
                    signature,
                    area: AbiArea::Outgoing,
                },
                None,
            )
            .unwrap();
            b.terminate(entry, Node::new(Op::Return, target), &[])
                .unwrap();
            assert!(verify_selected(b.finish(), target).is_err());
        }
    });
}

#[test]
fn narrow_displacements_materialize_only_with_existing_bounded_address_scratch() {
    fixture(true, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let object = b
            .object(
                LayoutFact {
                    size: 80,
                    alignment: 16,
                    disposition: LayoutDisposition::Addressable,
                },
                ObjectRole::Semantic,
                None,
            )
            .unwrap();
        let v = b.value(bits(), None).unwrap();
        let mut node = Node::new(Op::Scratch(val(v, bits())), target);
        node.objects.push(object.id());
        b.append(entry, node).unwrap();
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).unwrap();
        let mut draft = PlacementDraft::new(&selected);
        operand(&mut draft, entry, 0, 0, target.ints[0]);
        let site = Site::Instruction {
            block: entry.id(),
            ordinal: 0,
        };
        for (slot, view) in [target.ints[1], target.ints[2]].into_iter().enumerate() {
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
        let checked = check_placement(draft, target).unwrap();
        let policy = FramePolicy {
            alignment: 16,
            entry_remainder: 0,
            header_bytes: 16,
            incoming_base: 16,
            return_address: ReturnAddress::Link {
                view: target.link,
                offset: 8,
                bytes: 8,
            },
            max_frame: 4096,
            direct_min: -32,
            direct_max: 31,
            materialization: Some((-4096, 4095, 2)),
            address_scratch_group: 0,
        };
        let frame = plan_frame(&checked, policy).unwrap();
        assert_eq!(frame.bytes(), 80);
        assert!(
            matches!(frame.policy().return_address, ReturnAddress::Link { view, .. } if view == target.link)
        );
        assert_eq!(frame.object(object.id()).unwrap().offset, -80);
        assert_eq!(frame.object(object.id()).unwrap().base, Base::Frame);
        let region: Region = frame.object(object.id()).unwrap();
        assert_eq!(region.alignment, 16);
        assert_eq!(
            frame.object_access(site, object.id()),
            Some(AddressRecipe::Materialized {
                scratch: target.ints[1],
                steps: 2
            })
        );
        assert_eq!(frame.require_placement(&checked), Ok(()));
        let other = place_baseline(&selected, target).unwrap();
        assert_eq!(
            frame.require_placement(&other),
            Err(FrameError::WrongPlacement)
        );
        assert!(matches!(
            plan_frame(&other, policy).err(),
            Some(FrameError::UnsupportedDisplacement(_))
        ));
        assert_eq!(
            plan_frame(
                &checked,
                FramePolicy {
                    address_scratch_group: 99,
                    ..policy
                }
            )
            .err(),
            Some(FrameError::UndeclaredAddressScratch(site))
        );
        assert_eq!(
            plan_frame(
                &checked,
                FramePolicy {
                    return_address: ReturnAddress::Link {
                        view: target.link,
                        offset: 24,
                        bytes: 8
                    },
                    ..policy
                }
            )
            .err(),
            Some(FrameError::InvalidPolicy)
        );
        for invalid in [
            FramePolicy {
                incoming_base: 8,
                ..policy
            },
            FramePolicy {
                return_address: ReturnAddress::Link {
                    view: target.link,
                    offset: 0,
                    bytes: 8,
                },
                ..policy
            },
            FramePolicy {
                return_address: ReturnAddress::Link {
                    view: target.low,
                    offset: 8,
                    bytes: 8,
                },
                ..policy
            },
        ] {
            assert_eq!(
                plan_frame(&checked, invalid).err(),
                Some(FrameError::InvalidPolicy)
            );
        }
        assert_eq!(
            plan_frame(
                &checked,
                FramePolicy {
                    materialization: None,
                    address_scratch_group: 0,
                    ..policy
                }
            )
            .err(),
            Some(FrameError::UnsupportedDisplacement(-80))
        );
        assert_eq!(
            plan_frame(
                &checked,
                FramePolicy {
                    materialization: Some((-64, 63, 2)),
                    ..policy
                }
            )
            .err(),
            Some(FrameError::UnsupportedDisplacement(-80))
        );
        assert_eq!(
            plan_frame(
                &checked,
                FramePolicy {
                    materialization: Some((-4096, 4095, 3)),
                    ..policy
                }
            )
            .err(),
            Some(FrameError::UndeclaredAddressScratch(site))
        );
        assert_eq!(
            plan_frame(
                &checked,
                FramePolicy {
                    max_frame: 64,
                    ..policy
                }
            )
            .err(),
            Some(FrameError::UnsupportedSize(80))
        );
    });
}

#[test]
fn portable_abi_objects_overlay_fixed_areas_and_partial_width_saves_stay_separate() {
    fixture(true, |context, lower, target| {
        let (mut b, entry) = begin(context, lower);
        let signature = context
            .binding(crate::backend::plan::test_fixtures::source(0))
            .unwrap()
            .signature_id();
        let outgoing = b
            .object(
                LayoutFact {
                    size: 32,
                    alignment: 16,
                    disposition: LayoutDisposition::Addressable,
                },
                ObjectRole::Abi {
                    signature,
                    area: AbiArea::Outgoing,
                },
                None,
            )
            .unwrap();
        let result = b
            .object(
                LayoutFact {
                    size: 16,
                    alignment: 16,
                    disposition: LayoutDisposition::Addressable,
                },
                ObjectRole::Abi {
                    signature,
                    area: AbiArea::Results,
                },
                None,
            )
            .unwrap();
        b.terminate(entry, Node::new(Op::Return, target), &[])
            .unwrap();
        let selected = verify_selected(b.finish(), target).unwrap();
        let checked = place_baseline(&selected, target).unwrap();
        let frame = plan_frame(
            &checked,
            FramePolicy {
                alignment: 16,
                entry_remainder: 0,
                header_bytes: 16,
                incoming_base: 16,
                return_address: ReturnAddress::Link {
                    view: target.link,
                    offset: 8,
                    bytes: 8,
                },
                max_frame: 4096,
                direct_min: -4096,
                direct_max: 4095,
                materialization: None,
                address_scratch_group: 0,
            },
        )
        .unwrap();
        assert_eq!(frame.outgoing_bytes(), 32);
        assert_eq!(frame.object(outgoing.id()).unwrap().base, Base::Stack);
        assert_eq!(frame.object(result.id()).unwrap().offset, 32);
        assert_eq!(frame.bytes(), 64); // Two 64-bit preservation promises, including a partial SIMD view.
        assert_eq!(checked.storage().len(), 2);
        for storage in checked.storage() {
            assert_eq!(storage.bytes, 8);
        }
        for point in checked.transfer_points() {
            for transfer in checked.transfers(point) {
                if let Location::Storage(id) = transfer.source {
                    let region = frame.storage(id).unwrap();
                    assert_eq!(region.base, Base::Frame);
                    assert_eq!(region.bytes, 8);
                }
            }
        }
    });
}
