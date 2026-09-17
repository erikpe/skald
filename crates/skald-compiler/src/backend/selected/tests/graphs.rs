use super::*;

#[test]
fn payload_events_and_structural_consumers_need_no_target_enum_matches() {
    let mut f = facts();
    f.signatures[0].inputs.push(plan::Component {
        ty: plan::ScalarType::I64,
        role: plan::ComponentRole::Parameter(0),
    });
    let p = CheckedPlan::check(f).unwrap();
    let mut inventory = lir::ProgramBuilder::new(p.view());
    let a = lower(&p, source(0));
    let b = lower(&p, source(1));
    for body in [&a, &b] {
        inventory.begin(body.receipt().owner().key()).unwrap();
        inventory.complete(body, &body.receipt()).unwrap();
    }
    let program = inventory.finish().unwrap();
    let extension = lir::TargetDeclarations::new(&program).freeze().unwrap();
    let (mut resources, view, unit) = catalog();
    let bank = resources.bank(BankKind::Integer);
    let additional = resources.unit().unwrap();
    let additional_view = resources.view(bank, 64, &[additional], false).unwrap();
    let scratch_unit = resources.unit().unwrap();
    let scratch_view = resources.view(bank, 64, &[scratch_unit], false).unwrap();
    let ctx = SelectionContext::new(&extension, resources)
        .with_abi_areas(AbiAreas {
            incoming: vec![repr()],
            ..AbiAreas::default()
        })
        .unwrap();
    let signature = p.view().callables().next().unwrap().signature;
    let fact = p.view().callable(source(0)).unwrap().signature().unwrap();
    let bindings = ctx
        .abi_bindings(
            signature,
            fact.inputs
                .iter()
                .map(|component| AbiBinding {
                    component: *component,
                    representation: repr(),
                    location: AbiLocation::Slot {
                        area: AbiArea::Incoming,
                        index: 0,
                    },
                })
                .collect(),
            vec![],
        )
        .unwrap();
    assert_eq!(bindings.inputs().len(), fact.inputs.len());
    ctx.resources
        .require_view(view, 64, BankKind::Integer, true)
        .unwrap();
    let mut build = SelectedBuilder::<Synthetic>::new(&ctx, source(0), Some(a.receipt())).unwrap();
    let entry = build.block(&[], None).unwrap();
    let input = build.value(repr(), None).unwrap();
    let result = build.value(repr(), None).unwrap();
    let source_value = a
        .draft()
        .values()
        .find(|(_, value)| value.origin.is_some())
        .unwrap()
        .0;
    build
        .map_block_origin(
            &a,
            a.draft().blocks().next().unwrap().0,
            entry,
            Some(origin()),
        )
        .unwrap();
    let object = build
        .object(facts().layouts[0], ObjectRole::Semantic, None)
        .unwrap();
    build
        .map_object_origin(&a, a.draft().objects().next().unwrap().0, object)
        .unwrap();
    assert_eq!(build.object_description(object).unwrap().2, Some(origin()));
    // The explicit remap copies only origins, never lower definition sites.
    build.map_origin(&a, source_value, input).unwrap();
    let stale = lower(&p, source(0));
    assert_eq!(
        build.map_origin(&stale, source_value, result),
        Err(SelectedBuildError::Context(plan::PlanError::WrongContext))
    );
    assert_eq!(
        build.entry(entry, &[input, input], bindings.clone()),
        Err(SelectedBuildError::DuplicateDefinition)
    );
    build.entry(entry, &[input], bindings).unwrap();
    assert_eq!(build.block_origin(entry).unwrap(), Some(origin()));
    let mut call = Synthetic::new(Opcode::Call, vec![input.id()], Some(result.id()), view);
    call.clobbers.push((Timing::Late, unit));
    call.effects = Effects::new([
        Effect::Call,
        Effect::Read(crate::backend::effects::MemoryRegion::Object(object.id())),
    ]);
    call.objects.push(object.id());
    call.refs.push((
        plan::ArtifactId::Callable(source(1)),
        plan::ArtifactCategory::Callable,
    ));
    let desc = call.describe();
    let events: Vec<_> = desc.events().collect();
    assert_eq!(
        events,
        vec![
            Event::Operand {
                phase: Phase::EarlyUses,
                slot: 0
            },
            Event::Clobber {
                phase: Phase::LateClobbers,
                unit
            },
            Event::Operand {
                phase: Phase::LateDefinitions,
                slot: 1
            }
        ]
    );
    assert!(desc.effects.contains(Effect::Call));
    assert_eq!(desc.indirect_target, None);
    assert!(desc.abi_results.is_empty());
    assert_eq!(
        desc.artifacts,
        &[(
            plan::ArtifactId::Callable(source(1)),
            plan::ArtifactCategory::Callable
        )]
    );
    assert_eq!(desc.objects, &[object.id()]);
    assert!(matches!(desc.operands[0].constraint, Constraint::Fixed(v) if v == view));
    build.append(entry, call).unwrap();
    let tied_result = build.value(repr(), None).unwrap();
    let tied = Synthetic::new(
        Opcode::TwoAddress,
        vec![result.id()],
        Some(tied_result.id()),
        view,
    );
    let desc = tied.describe();
    assert_eq!(
        desc.ties,
        &[Tie {
            input: 0,
            output: 1
        }]
    );
    assert_ne!(desc.operands[0].value, desc.operands[1].value);
    if let Constraint::Resources { views, memory } = desc.operands[0].constraint {
        assert_eq!(views, &[view]);
        assert!(!memory);
    } else {
        panic!("resource alternatives expected");
    }
    build.append(entry, tied).unwrap();
    let target = build.block(&[], None).unwrap();
    let target2 = build.block(&[], None).unwrap();
    let flags = Synthetic::new(Opcode::Flags, vec![tied_result.id()], None, view);
    assert!(matches!(flags.describe().bundle, Some(Bundle::Atomic)));
    assert_eq!(
        flags.describe().events().collect::<Vec<_>>(),
        vec![Event::Operand {
            phase: Phase::LateUses,
            slot: 0
        }]
    );
    build
        .terminate(entry, flags, &[(target, vec![]), (target2, vec![])])
        .unwrap();
    let three_result = build.value(repr(), None).unwrap();
    let mut three_payload = Synthetic::new(
        Opcode::ThreeAddress,
        vec![result.id(), tied_result.id()],
        Some(three_result.id()),
        view,
    );
    three_payload.views.push(additional_view);
    three_payload.scratch_views = vec![scratch_view];
    build.append(target, three_payload).unwrap();
    for block in [target, target2] {
        build
            .terminate(
                block,
                Synthetic::new(Opcode::Return, vec![result.id()], None, view),
                &[],
            )
            .unwrap();
    }
    let selected: SelectedDraft<'_, Synthetic> = build.finish();
    graph::check_graph(&selected).unwrap();
    assert_eq!(
        selected.describe().unwrap().values[0].origin,
        Some(origin())
    );
    assert_eq!(selected.abi().unwrap().inputs().len(), 1);
    assert_eq!(
        selected.describe().unwrap().blocks[0]
            .terminal
            .as_ref()
            .unwrap()
            .1
            .len(),
        2
    );
    let three = Synthetic::new(
        Opcode::ThreeAddress,
        vec![input.id(), result.id()],
        Some(tied_result.id()),
        view,
    );
    let desc = three.describe();
    assert_eq!(desc.operands.len(), 3);
    assert!(desc.ties.is_empty());
    if let Some(Bundle::Bounded { steps, scratch }) = desc.bundle {
        assert_eq!(steps.get(), 2);
        assert_eq!(scratch.len(), 1);
        assert_eq!(scratch[0].views, &[view]);
    } else {
        panic!("bounded recipe expected");
    }
    let mut early = Synthetic::new(
        Opcode::EarlyClobber,
        vec![input.id()],
        Some(result.id()),
        view,
    );
    assert_eq!(early.describe().operands[1].timing, Timing::Early);
    early.clobbers.push((Timing::Early, unit));
    assert_eq!(
        early.describe().events().collect::<Vec<_>>(),
        vec![
            Event::Operand {
                phase: Phase::EarlyUses,
                slot: 0
            },
            Event::Clobber {
                phase: Phase::EarlyClobbers,
                unit
            },
            Event::Operand {
                phase: Phase::EarlyDefinitions,
                slot: 1
            }
        ]
    );
    let (_r, _v, _u) = catalog();
    let other = SelectionContext::new(&extension, ResourceCatalog::default());
    let mut foreign =
        SelectedBuilder::<Synthetic>::new(&other, source(0), Some(a.receipt())).unwrap();
    assert!(matches!(
        foreign.block(&[input], None),
        Err(SelectedBuildError::Context(plan::PlanError::WrongContext))
    ));
    let signature = p.view().callables().next().unwrap().signature;
    for ty in [
        RepresentationKind::DataAddress,
        RepresentationKind::CodeAddress(signature),
    ] {
        foreign
            .value(Representation::new(ty, 64).unwrap(), None)
            .unwrap();
    }
    assert!(foreign
        .value(
            Representation::new(RepresentationKind::DataAddress, 32).unwrap(),
            None
        )
        .is_err());
    for role in [
        ObjectRole::Semantic,
        ObjectRole::Trace,
        ObjectRole::Abi(AbiArea::Incoming),
    ] {
        let object = foreign.object(facts().layouts[0], role, None).unwrap();
        let (layout, role, origin) = foreign.object_description(object).unwrap();
        assert_eq!(layout.size, 8);
        assert!(origin.is_none());
        if let ObjectRole::Abi(area) = role {
            assert_eq!(*area, AbiArea::Incoming);
        }
    }
    let component = plan::Component {
        ty: plan::ScalarType::DataAddress,
        role: plan::ComponentRole::ReceiverStatic,
    };
    let binding = AbiBinding {
        component,
        representation: repr(),
        location: AbiLocation::Fixed(view),
    };
    assert_eq!(binding.component.role, plan::ComponentRole::ReceiverStatic);
    assert_eq!(binding.representation, repr());
    assert_eq!(binding.location, AbiLocation::Fixed(view));
    for area in [AbiArea::Incoming, AbiArea::Outgoing, AbiArea::Results] {
        let location = AbiLocation::Slot { area, index: 0 };
        assert!(matches!(location, AbiLocation::Slot { index: 0, .. }));
        let constraint = Constraint::AbiSlot { area, index: 0 };
        assert!(matches!(constraint, Constraint::AbiSlot {area: a, index:0} if a == area));
    }
    let scratch = Scratch {
        representation: repr(),
        views: &[],
        count: NonZeroU16::new(1).unwrap(),
    };
    assert_eq!(scratch.representation, repr());
    assert!(scratch.views.is_empty());
    assert_eq!(scratch.count.get(), 1);
    assert!(Synthetic::new(Opcode::Return, vec![], None, view)
        .describe()
        .abi_inputs
        .is_empty());
}

#[test]
fn selected_thunk_drafts_require_frozen_declarations_without_fabricated_inputs() {
    let p = CheckedPlan::check(facts()).unwrap();
    let mut inventory = lir::ProgramBuilder::new(p.view());
    for index in 0..2 {
        let body = lower(&p, source(index));
        inventory.begin(source(index)).unwrap();
        inventory.complete(&body, &body.receipt()).unwrap();
    }
    let program = inventory.finish().unwrap();
    let thunk = plan::LirCallableId::TargetThunk(plan::TargetThunkKey {
        family: 0,
        specialization: 0,
    });
    let mut extension = lir::TargetDeclarations::new(&program);
    extension
        .declare(plan::ArtifactDeclaration {
            key: plan::ArtifactId::Callable(thunk),
            signature: Some(p.view().callables().next().unwrap().signature),
            layout: None,
        })
        .unwrap();
    let extension = extension.freeze().unwrap();
    let (resources, view, _) = catalog();
    let ctx = SelectionContext::new(&extension, resources);
    assert!(SelectedBuilder::<Synthetic>::new(&ctx, source(0), None).is_err());
    let mut build = SelectedBuilder::<Synthetic>::new(&ctx, thunk, None).unwrap();
    let entry = build.block(&[], None).unwrap();
    let bindings = ctx
        .abi_bindings(
            p.view().callables().next().unwrap().signature,
            vec![],
            vec![],
        )
        .unwrap();
    build.entry(entry, &[], bindings).unwrap();
    build
        .terminate(
            entry,
            Synthetic::new(Opcode::Return, vec![], None, view),
            &[],
        )
        .unwrap();
    assert_eq!(
        build.append(entry, Synthetic::new(Opcode::Return, vec![], None, view)),
        Err(SelectedBuildError::Terminated)
    );
    graph::check_graph(&build.finish()).unwrap();
    let missing = plan::LirCallableId::TargetThunk(plan::TargetThunkKey {
        family: 0,
        specialization: 1,
    });
    assert!(SelectedBuilder::<Synthetic>::new(&ctx, missing, None).is_err());
}
