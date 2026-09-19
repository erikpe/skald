//! Adversarial physical drafts, supplied after successful independent publication.
use super::super::super::Gpr;
use super::*;
use crate::backend::plan::ArtifactId;
fn rejection(
    body: &PhysicalDraft<'_, '_, '_, '_>,
    mutate: impl FnOnce(&mut PhysicalDraft<'_, '_, '_, '_>),
) -> Reason {
    let mut bad = body.clone();
    mutate(&mut bad);
    let frame = body.frame;
    let placement = frame.checked_placement();
    check_native_physical(bad, placement.selected(), placement, frame)
        .err()
        .expect("forged draft was published")
        .reason
}
#[test]
fn invalid_encodings_recipes_dependencies_and_provenance_reject() {
    super::super::tests::complete(
        "fn add(a:i64,b:i64)->i64{return a+b;} fn main()->i64{return add(2,3);}",
        false,
        |body| {
            assert_eq!(
                rejection(body, |bad| {
                    let movement = bad
                        .blocks
                        .iter_mut()
                        .flat_map(|b| &mut b.groups)
                        .flat_map(|g| &mut g.instructions)
                        .find(|i| matches!(i, Instruction::Move { .. }))
                        .unwrap();
                    if let Instruction::Move { bits, .. } = movement {
                        *bits = 7;
                    }
                }),
                Reason::Encoding
            );
            assert_eq!(
                rejection(body, |bad| {
                    let first = bad.blocks[bad.entry.0].groups.first_mut().unwrap();
                    first.dependencies.push(ArtifactId::TraceTls);
                }),
                Reason::Dependency
            );
            assert_eq!(
                rejection(body, |bad| {
                    let movement = bad
                        .blocks
                        .iter_mut()
                        .flat_map(|b| &mut b.groups)
                        .filter(|g| matches!(g.origin, Origin::Selected(_)))
                        .flat_map(|g| &mut g.instructions)
                        .find(|i| {
                            matches!(
                                i,
                                Instruction::Move {
                                    source: Operand::Immediate(_),
                                    ..
                                }
                            )
                        });
                    // Some bodies have no constant; corrupt their first explicit transfer.
                    if let Some(Instruction::Move {
                        source: Operand::Immediate(v),
                        ..
                    }) = movement
                    {
                        *v ^= 1;
                    } else {
                        let transfer = bad
                            .blocks
                            .iter_mut()
                            .flat_map(|b| &mut b.groups)
                            .find(|g| matches!(g.origin, Origin::Transfer { .. }))
                            .unwrap();
                        if let Instruction::Move { destination, .. } = &mut transfer.instructions[0]
                        {
                            *destination = Operand::Register(Register::Gpr(Gpr::R10));
                        }
                    }
                }),
                Reason::Recipe
            );
            assert_eq!(
                rejection(body, |bad| {
                    bad.blocks[bad.entry.0].id = BlockId(usize::MAX);
                }),
                Reason::Topology
            );
            let mut policy = body.frame.policy();
            policy.direct_max = i64::MAX;
            let loose =
                crate::backend::frame::plan_frame(body.frame.checked_placement(), policy).unwrap();
            let raw = super::super::super::realize_native(
                body.frame.checked_placement().selected(),
                body.frame.checked_placement(),
                &loose,
            )
            .unwrap();
            assert_eq!(
                check_native_physical(
                    raw,
                    body.frame.checked_placement().selected(),
                    body.frame.checked_placement(),
                    &loose
                )
                .err()
                .unwrap()
                .reason,
                Reason::Recipe
            );
            let other_frame =
                super::super::super::plan_native_frame(body.frame.checked_placement()).unwrap();
            assert_eq!(
                check_native_physical(
                    body.clone(),
                    body.frame.checked_placement().selected(),
                    body.frame.checked_placement(),
                    &other_frame
                )
                .err()
                .unwrap()
                .reason,
                Reason::Provenance
            );
            let first = check_native_physical(
                body.clone(),
                body.frame.checked_placement().selected(),
                body.frame.checked_placement(),
                body.frame,
            )
            .unwrap();
            let second = check_native_physical(
                body.clone(),
                body.frame.checked_placement().selected(),
                body.frame.checked_placement(),
                body.frame,
            )
            .unwrap();
            assert!(!first.receipt().same_snapshot(&second.receipt()));
            assert!(!first.receipt().matches(&second));
        },
    );
}
#[test]
fn bypassed_epilogues_missing_frame_establishment_and_unsaved_bits_reject() {
    super::super::tests::complete("fn main()->i64{return 7;}", false, |body| {
        assert_eq!(
            rejection(body, |bad| {
                let epilogue = bad
                    .blocks
                    .iter_mut()
                    .flat_map(|b| &mut b.groups)
                    .find(|g| matches!(g.origin, Origin::Epilogue(_)))
                    .unwrap();
                epilogue.instructions = vec![Instruction::Return];
            }),
            Reason::StackState
        );
        assert_eq!(
            rejection(body, |bad| {
                bad.blocks[bad.entry.0].groups[0].instructions.remove(0);
            }),
            Reason::StackState
        );
        assert_eq!(
            rejection(body, |bad| {
                let epilogue = bad
                    .blocks
                    .iter_mut()
                    .flat_map(|b| &mut b.groups)
                    .find(|g| matches!(g.origin, Origin::Epilogue(_)))
                    .unwrap();
                epilogue.instructions.insert(
                    0,
                    Instruction::Move {
                        kind: MoveKind::Integer,
                        bits: 64,
                        source: Operand::Immediate(0),
                        destination: Operand::Register(Register::Gpr(Gpr::R12)),
                    },
                );
            }),
            Reason::Preservation
        );
    });
}
#[test]
fn call_alignment_and_cfg_stack_joins_are_independent_of_recipe_acceptance() {
    let source = "fn add(a:i64)->i64{return a+1;} fn main()->i64{var n:i64=0;while(n<3){n=add(n);}return n;}";
    super::super::tests::complete(source, false, |body| {
        let has_call = body
            .blocks
            .iter()
            .flat_map(|b| &b.groups)
            .flat_map(|g| &g.instructions)
            .any(|i| matches!(i, Instruction::Call(_)));
        if has_call {
            assert_eq!(
                rejection(body, |bad| {
                    let group = bad.blocks[bad.entry.0].groups.first_mut().unwrap();
                    group.instructions.push(Instruction::StackSubtract(8));
                }),
                Reason::Alignment
            );
        }
        let branch = body.blocks.iter().position(|b| {
            b.groups
                .iter()
                .flat_map(|g| &g.instructions)
                .any(|i| matches!(i, Instruction::JumpIf { .. }))
        });
        if let Some(branch) = branch {
            let mut bad = body.clone();
            let groups = &mut bad.blocks[branch].groups;
            let group = groups
                .iter_mut()
                .find(|g| {
                    g.instructions
                        .iter()
                        .any(|i| matches!(i, Instruction::JumpIf { .. }))
                })
                .unwrap();
            let jump = group
                .instructions
                .iter()
                .position(|i| matches!(i, Instruction::JumpIf { .. }))
                .unwrap();
            group
                .instructions
                .insert(jump, Instruction::StackSubtract(16));
            let frame = body.frame;
            assert_eq!(
                state::check(&bad, frame.checked_placement().selected().receipt().key())
                    .err()
                    .unwrap()
                    .reason,
                Reason::StackJoin
            );
        }
    });
}
#[test]
fn xmm_indices_immediates_and_two_operand_multiply_widths_are_checked_without_an_assembler() {
    assert_eq!(
        encoding::check(
            &Instruction::Move {
                kind: MoveKind::Float,
                bits: 64,
                source: Operand::Register(Register::Xmm(16)),
                destination: Operand::Register(Register::Xmm(0))
            },
            1
        ),
        Err(Reason::Encoding)
    );
    assert_eq!(
        encoding::check(
            &Instruction::Compare {
                float: false,
                bits: 8,
                left: Operand::Register(Register::Gpr(Gpr::Rax)),
                right: Operand::Immediate(256)
            },
            1
        ),
        Err(Reason::Encoding)
    );
    assert_eq!(
        encoding::check(
            &Instruction::Alu {
                op: Alu::Multiply,
                float: false,
                bits: 8,
                source: Operand::Register(Register::Gpr(Gpr::Rcx)),
                destination: Register::Gpr(Gpr::Rax)
            },
            1
        ),
        Err(Reason::Encoding)
    );
}

#[test]
fn concrete_callee_saves_require_the_original_full_width_bits() {
    use crate::backend::{
        placement::*,
        selected::{Payload, SelectedFact},
        x86_64_sysv::native::{
            check_native_placement, plan_native_frame, realize_native, selected::Opcode,
            NativeResources,
        },
    };
    let mut witnesses = 0;
    super::super::tests::complete("fn main()->i64{return 7;}", false, |body| {
        let selected = body.frame.checked_placement().selected();
        let resources = NativeResources::new().unwrap();
        let rbx = resources.gpr(Gpr::Rbx, 64).unwrap();
        let rax = resources.gpr(Gpr::Rax, 64).unwrap();
        let mut draft = PlacementDraft::new(selected);
        let mut constant = None;
        let mut terminal = None;
        let mut compatible = true;
        selected
            .visit::<()>(|fact| {
                match fact {
                    SelectedFact::Instruction {
                        block,
                        ordinal,
                        payload,
                    } => {
                        if let Opcode::Constant { out, .. } = payload.opcode() {
                            let site = Site::Instruction { block, ordinal };
                            draft
                                .assign(
                                    Assignment::Operand { site, slot: 0 },
                                    Location::Resource(rbx),
                                )
                                .unwrap();
                            constant = Some((site, *out));
                        } else {
                            compatible = false;
                        }
                    }
                    SelectedFact::Terminal {
                        block,
                        payload: Some(payload),
                        ..
                    } => {
                        if matches!(payload.opcode(), Opcode::Return { .. }) {
                            let site = Site::Terminal(block);
                            terminal = Some(site);
                            for slot in 0..payload.describe().operands.len() {
                                draft
                                    .assign(
                                        Assignment::Operand { site, slot },
                                        Location::Resource(rax),
                                    )
                                    .unwrap();
                            }
                        } else {
                            compatible = false;
                        }
                    }
                    _ => {}
                }
                Ok(())
            })
            .unwrap();
        if !compatible {
            return;
        }
        let (site, out) = constant.unwrap();
        let terminal = terminal.unwrap();
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
        // Original preserved bits can legally pass through a caller SIMD view.
        let xmm = resources.xmm(0, 64).unwrap();
        let float = crate::backend::selected::Representation::new(
            crate::backend::selected::RepresentationKind::Float,
            64,
        )
        .unwrap();
        let mut to_simd = transfer(
            TransferValue::Preserved(rbx),
            Location::Resource(rbx),
            Location::Resource(xmm),
        );
        to_simd.kind = TransferKind::Bitwise;
        to_simd.destination_representation = float;
        draft.transfer(TransferPoint::Entry, to_simd);
        let mut from_simd = transfer(
            TransferValue::Preserved(rbx),
            Location::Resource(xmm),
            Location::Resource(rbx),
        );
        from_simd.kind = TransferKind::Bitwise;
        from_simd.source_representation = float;
        draft.transfer(TransferPoint::Entry, from_simd);
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
        draft.transfer(
            TransferPoint::Before(terminal),
            transfer(
                TransferValue::Preserved(rbx),
                Location::Storage(saved),
                Location::Resource(rbx),
            ),
        );
        let placement = check_native_placement(draft).unwrap();
        let frame = plan_native_frame(&placement).unwrap();
        let physical = realize_native(selected, &placement, &frame).unwrap();
        check_native_physical(physical.clone(), selected, &placement, &frame).unwrap();
        witnesses += 1;
        assert_eq!(
            rejection(&physical, |bad| {
                let save = bad.blocks[bad.entry.0]
                    .groups
                    .iter_mut()
                    .find(|g| {
                        matches!(
                            g.origin,
                            Origin::Transfer {
                                point: TransferPoint::Entry,
                                ..
                            }
                        ) && matches!(
                            g.instructions.first(),
                            Some(Instruction::Move {
                                destination: Operand::Memory { .. },
                                ..
                            })
                        )
                    })
                    .unwrap();
                if let Instruction::Move { bits, .. } = &mut save.instructions[0] {
                    *bits = 32;
                }
            }),
            Reason::Preservation
        );
        assert_eq!(
            rejection(&physical, |bad| {
                let restore = bad
                    .blocks
                    .iter_mut()
                    .flat_map(|b| &mut b.groups)
                    .find(|g| {
                        g.origin
                            == Origin::Transfer {
                                point: TransferPoint::Before(terminal),
                                index: 0,
                            }
                    })
                    .unwrap();
                restore.instructions.clear();
            }),
            Reason::Preservation
        );
    });
    assert_eq!(witnesses, 1);
}

#[test]
fn declared_scratch_and_actual_call_dependencies_are_required() {
    let source =
        "fn floating(a:f64)->f64{return a+1.5;} fn main()->i64{return (i64)floating(2.5);}";
    let mut scratch_cases = 0;
    let mut call_cases = 0;
    super::super::tests::complete(source, false, |body| {
        if body.blocks.iter().flat_map(|b| &b.groups).any(|g| {
            matches!(
                g.instructions.as_slice(),
                [
                    Instruction::Move {
                        source: Operand::Immediate(_),
                        destination: Operand::Register(Register::Gpr(_)),
                        ..
                    },
                    Instruction::Move {
                        destination: Operand::Register(Register::Xmm(_)),
                        ..
                    }
                ]
            )
        }) {
            scratch_cases += 1;
            assert_eq!(
                rejection(body, |bad| {
                    let group = bad
                        .blocks
                        .iter_mut()
                        .flat_map(|b| &mut b.groups)
                        .find(|g| {
                            matches!(
                                g.instructions.as_slice(),
                                [
                                    Instruction::Move {
                                        source: Operand::Immediate(_),
                                        destination: Operand::Register(Register::Gpr(_)),
                                        ..
                                    },
                                    Instruction::Move {
                                        destination: Operand::Register(Register::Xmm(_)),
                                        ..
                                    }
                                ]
                            )
                        })
                        .unwrap();
                    if let Instruction::Move { destination, .. } = &mut group.instructions[0] {
                        *destination = Operand::Register(Register::Gpr(
                            if *destination == Operand::Register(Register::Gpr(Gpr::R10)) {
                                Gpr::R11
                            } else {
                                Gpr::R10
                            },
                        ));
                    }
                }),
                Reason::Recipe
            );
        }
        if body.blocks.iter().flat_map(|b| &b.groups).any(|g| {
            g.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Call(CallTarget::Direct(_))))
        }) {
            call_cases += 1;
            assert_eq!(
                rejection(body, |bad| {
                    let group = bad
                        .blocks
                        .iter_mut()
                        .flat_map(|b| &mut b.groups)
                        .find(|g| {
                            g.instructions
                                .iter()
                                .any(|i| matches!(i, Instruction::Call(CallTarget::Direct(_))))
                        })
                        .unwrap();
                    group.dependencies.clear();
                }),
                Reason::Dependency
            );
        }
    });
    assert!(scratch_cases > 0);
    assert!(call_cases > 0);
}
