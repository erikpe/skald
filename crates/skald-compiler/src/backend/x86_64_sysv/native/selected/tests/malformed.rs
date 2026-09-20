use super::*;
#[test]
fn independent_native_verification_rejects_corrupt_fields_effects_flags_and_reference_caches() {
    let mut tested = [false; 4];
    for_sources(
        "fn main() -> i64 { var value: i64 = 2; return value + 3; }",
        |context, lower| {
            let body = select(context, lower).unwrap();
            let verifier = Verifier::new(lower.receipt().owner().context().profile()).unwrap();
            body.visit(|fact| {
                if let SelectedFact::Instruction { payload, .. } = fact {
                    match payload.opcode {
                        Opcode::Alu { .. } => {
                            let mut malformed = payload.clone();
                            malformed.clobbers.clear();
                            assert!(verifier.verify_payload(&malformed, false).is_err());
                            tested[0] = true;
                        }
                        Opcode::Load { .. } => {
                            let mut malformed = payload.clone();
                            malformed.effects = Effects::default();
                            assert!(verifier.verify_payload(&malformed, false).is_err());
                            if let Opcode::Load { bytes, .. } = &mut malformed.opcode {
                                *bytes = 4;
                            }
                            malformed.refresh();
                            assert!(verifier.verify_payload(&malformed, false).is_err());
                            tested[1] = true;
                        }
                        Opcode::ObjectAddress { .. } => {
                            let mut malformed = payload.clone();
                            malformed.objects.clear();
                            assert!(verifier.verify_payload(&malformed, false).is_err());
                            if let Opcode::ObjectAddress { out, .. } = &mut malformed.opcode {
                                out.representation = selected::Representation::new(
                                    selected::RepresentationKind::DataAddress,
                                    32,
                                )
                                .unwrap();
                            }
                            malformed.refresh();
                            assert!(verifier.verify_payload(&malformed, false).is_err());
                            tested[2] = true;
                        }
                        Opcode::Constant { .. } => {
                            let mut malformed = payload.clone();
                            if let Opcode::Constant { out, .. } = &mut malformed.opcode {
                                out.representation = selected::Representation::new(
                                    selected::RepresentationKind::Float,
                                    64,
                                )
                                .unwrap();
                            }
                            malformed.refresh();
                            assert!(verifier.verify_payload(&malformed, false).is_err());
                            tested[3] = true;
                        }
                        Opcode::Lifetime { .. } => {
                            let mut malformed = payload.clone();
                            if let Opcode::Lifetime { site, sites, .. } = &mut malformed.opcode {
                                *site = *sites;
                            }
                            assert!(verifier.verify_payload(&malformed, false).is_err());
                        }
                        _ => {}
                    }
                    // Total descriptors also cover malformed return metadata.
                    let malformed = Instruction::new(
                        Opcode::Return {
                            values: payload.operands().into_iter().map(|(v, _)| v).collect(),
                            bindings: vec![],
                        },
                        payload.origin,
                        &payload.resources,
                    );
                    assert!(std::panic::catch_unwind(|| malformed.describe()).is_ok());
                    if !payload.operands().is_empty() {
                        assert!(verifier.verify_payload(&malformed, true).is_err());
                    }
                }
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
        },
    );
    assert!(tested.into_iter().all(|v| v));
}

#[test]
fn joint_publication_checks_native_clobbers_after_a_consuming_split() {
    for_sources("fn main() -> i64 { return 0; }", |context, lower| {
        let body = select(context, lower).unwrap();
        let verifier = Verifier::new(lower.receipt().owner().context().profile()).unwrap();
        let mut transfer = Instruction::new(
            Opcode::Jump,
            Origin {
                site: Site::Terminator(lower.draft().entry().unwrap()),
                span: None,
            },
            &verifier.resources,
        );
        // Shared event checking permits a flag clobber here; native JMP cannot.
        transfer
            .clobbers
            .push((Timing::Late, verifier.resources.flags()));
        let mut editor = body.into_editor();
        let entry = editor.blocks().next().unwrap();
        editor.split_block(entry, 0, transfer).unwrap();
        match editor.finish(&verifier) {
            Err(selected::SelectedEditFailure::Verify(failures)) => {
                assert!(failures.iter().any(|f| matches!(
                    f.reason,
                    selected::SelectedReason::Target("missing or extraneous x86 flag clobber")
                )))
            }
            _ => panic!("joint publication must reject a native opcode event mismatch"),
        }
    });
}

#[test]
fn native_profile_and_resource_catalog_are_independent_publication_requirements() {
    let mut profile = plan::test_fixtures::facts().profile;
    profile.data_layout.pointer_bytes = 4;
    assert!(Verifier::new(profile).is_err());
    profile.data_layout.pointer_bytes = 8;
    profile.data_layout.pointer_alignment = 4;
    assert!(Verifier::new(profile).is_err());
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "native.ska",
        "fn main() -> i64 { return 0; }",
    );
    let planned = plan_program(crate::backend::BackendInput::without_runtime_trace(
        &fixture.mir,
    ))
    .unwrap();
    let catalog = TargetDeclarations::new(planned.plan().view())
        .freeze()
        .unwrap();
    let mut context = selection_context(&catalog).unwrap();
    context.resources.unit().unwrap(); // Structurally legal, but not x86's catalog.
    let mut worklist = ProgramBuilder::new(planned.plan().view());
    let lower = lower_next(&planned, &mut worklist).unwrap().unwrap();
    match select(&context, &lower) {
        Err(SelectionError::Verify(failures)) => assert!(failures.iter().any(|f| matches!(
            f.reason,
            selected::SelectedReason::Target("incorrect native unit catalog")
        ))),
        _ => panic!("joint publication must check canonical native resource authority"),
    }
}
