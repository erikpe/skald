use super::*;
use crate::backend::x86_64_sysv::native::{place_native_baseline, plan_native_frame};
#[test]
fn native_publication_rejects_structurally_valid_noncanonical_area_padding() {
    super::calls::complete("fn main()->i64{return 7;}", false, |context, lower| {
        let mut forged =
            selected::SelectionContext::new(context.catalog(), context.resources.clone());
        for (signature, _) in context.catalog().plan().signatures_with_ids() {
            let Some(areas) = context.areas(signature) else {
                continue;
            };
            forged = forged
                .with_abi_areas(
                    signature,
                    selected::AbiAreas {
                        incoming: areas.incoming.clone(),
                        outgoing: areas.outgoing.clone(),
                        results: areas.results.clone(),
                    },
                )
                .unwrap();
            for area in [
                selected::AbiArea::Incoming,
                selected::AbiArea::Outgoing,
                selected::AbiArea::Results,
            ] {
                let mut layout = context.abi_layout(signature, area).unwrap().clone();
                if area == selected::AbiArea::Outgoing {
                    layout.alignment = 8;
                }
                forged = forged.with_abi_layout(signature, area, layout).unwrap();
            }
        }
        assert!(select(&forged, lower).is_err());
    });
}
#[test]
fn manual_native_callee_save_placement_gets_a_width_correct_frame() {
    use crate::backend::placement::*;
    use crate::backend::x86_64_sysv::native::{check_native_placement, Gpr, NativeResources};
    for_sources("fn main()->i64{return 7;}", |context, lower| {
        let selected = select(context, lower).unwrap();
        let resources = NativeResources::new().unwrap();
        let rbx = resources.gpr(Gpr::Rbx, 64).unwrap();
        let rax = resources.gpr(Gpr::Rax, 64).unwrap();
        let mut draft = PlacementDraft::new(&selected);
        let mut constant = None;
        selected
            .visit::<()>(|fact| {
                match fact {
                    SelectedFact::Instruction {
                        block,
                        ordinal,
                        payload,
                    } => {
                        if let Opcode::Constant { out, .. } = payload.opcode() {
                            let site =
                                crate::backend::placement::Site::Instruction { block, ordinal };
                            draft
                                .assign(
                                    Assignment::Operand { site, slot: 0 },
                                    Location::Resource(rbx),
                                )
                                .unwrap();
                            constant = Some((site, *out));
                        }
                    }
                    SelectedFact::Terminal {
                        block,
                        payload: Some(payload),
                        ..
                    } => {
                        for slot in 0..payload.describe().operands.len() {
                            draft
                                .assign(
                                    Assignment::Operand {
                                        site: crate::backend::placement::Site::Terminal(block),
                                        slot,
                                    },
                                    Location::Resource(rax),
                                )
                                .unwrap();
                        }
                    }
                    _ => {}
                }
                Ok(())
            })
            .unwrap();
        let (site, out) = constant.unwrap();
        let saved = draft.storage(Storage {
            representation: out.representation,
            bytes: 8,
            alignment: 8,
            purpose: StoragePurpose::CalleeSave(rbx),
            lifetime: StorageLifetime::WholeCallable,
        });
        let transfer = |value, source, destination| Transfer {
            value,
            source,
            destination,
            source_representation: out.representation,
            destination_representation: out.representation,
            kind: TransferKind::Copy,
            scratch: vec![],
        };
        draft.transfer(
            TransferPoint::Entry,
            transfer(
                TransferValue::Preserved(rbx),
                Location::Resource(rbx),
                Location::Storage(saved),
            ),
        );
        draft.transfer(
            TransferPoint::After(site),
            transfer(
                TransferValue::Selected(out.value),
                Location::Resource(rbx),
                Location::Resource(rax),
            ),
        );
        let block = if let crate::backend::placement::Site::Instruction { block, .. } = site {
            block
        } else {
            unreachable!()
        };
        draft.transfer(
            TransferPoint::Before(crate::backend::placement::Site::Terminal(block)),
            transfer(
                TransferValue::Preserved(rbx),
                Location::Storage(saved),
                Location::Resource(rbx),
            ),
        );
        let checked = check_native_placement(draft).unwrap();
        let frame = plan_native_frame(&checked).unwrap();
        assert_eq!(frame.bytes(), 16);
        assert_eq!(frame.storage(saved).unwrap().bytes, 8);
        assert_eq!(frame.storage(saved).unwrap().offset, -8);
    });
}
#[test]
fn byte_and_boolean_stack_slots_keep_word_stride_and_outgoing_padding() {
    for source in [
        "fn pick(a:u8,b:u8,c:u8,d:u8,e:u8,f:u8,g:u8,h:u8,i:u8)->u8{return i;} fn main()->i64{return (i64) pick(1u8,2u8,3u8,4u8,5u8,6u8,7u8,8u8,9u8);}",
        "fn pick(a:bool,b:bool,c:bool,d:bool,e:bool,f:bool,g:bool,h:bool,i:bool)->bool{return i;} fn main()->i64{if (pick(true,true,true,true,true,true,true,true,true)){return 7;}return 0;}",
    ] {
        let mut seen = 0;
        super::calls::complete(source, false, |context, lower| {
            let selected = select(context, lower).unwrap();
            let placement = place_native_baseline(&selected).unwrap();
            let frame = plan_native_frame(&placement).unwrap();
            for point in placement.transfer_points() {
                for transfer in placement.transfers(point) {
                    for location in [transfer.source, transfer.destination] {
                        if let crate::backend::placement::Location::Abi { signature, area, index } = location {
                            let layout = context.abi_layout(signature, area).unwrap();
                            assert_eq!(layout.slots[index].offset, index * 8);
                            assert_eq!(layout.slots[index].bytes, 8);
                            let region = frame.location(location).unwrap().unwrap();
                            assert_eq!(region.alignment, 8);
                            match area {
                                selected::AbiArea::Incoming => { assert_eq!(region.offset, 16 + index as i64 * 8); assert_eq!(layout.bytes, 24); }
                                selected::AbiArea::Outgoing => { assert_eq!(region.offset, index as i64 * 8); assert_eq!(layout.bytes, 32); }
                                _ => panic!("native scalar result is a resource"),
                            }
                            seen += 1;
                        }
                    }
                }
            }
        });
        assert!(seen >= 6);
    }
}
#[test]
fn frame_plans_cover_native_pressure_and_both_trace_policies() {
    let source = "fn sum(a: i64,b: i64,c: i64,d: i64,e: i64,f: i64,g: i64,h: i64,i: i64) -> i64 { return a+b+c+d+e+f+g+h+i; } fn main() -> i64 { return sum(1,2,3,4,5,6,7,8,9); }";
    for trace in [false, true] {
        let mut pressure = 0;
        super::calls::complete(source, trace, |context, lower| {
            let selected = select(context, lower).unwrap();
            let placement = place_native_baseline(&selected).unwrap();
            let frame = plan_native_frame(&placement).unwrap();
            assert_eq!(frame.bytes() % 16, 0);
            assert_eq!(frame.require_placement(&placement), Ok(()));
            let again = plan_native_frame(&placement).unwrap();
            assert_eq!(frame.bytes(), again.bytes());
            if frame.outgoing_bytes() >= 32 {
                pressure += 1;
            }
            selected
                .visit::<()>(|fact| {
                    if let SelectedFact::Object { id, layout, .. } = fact {
                        let region = frame.object(id).unwrap();
                        assert_eq!(region.bytes, layout.size);
                        assert_eq!(region.offset % layout.alignment as i64, 0);
                    }
                    Ok(())
                })
                .unwrap();
        });
        assert!(pressure > 0);
    }
}
