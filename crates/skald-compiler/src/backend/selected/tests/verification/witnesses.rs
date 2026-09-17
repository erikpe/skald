use super::*;
#[test]
pub(super) fn destructive_and_three_address_targets_preserve_live_input_flow() {
    for shape in [Shape::Two, Shape::Three] {
        let p = CheckedPlan::check(supplied(shape)).unwrap();
        let (program, bodies) = inventory(&p);
        let extension = lir::TargetDeclarations::new(program.parent())
            .freeze()
            .unwrap();
        let (r, views, units) = resources();
        let ctx = context(&extension, r, vec![repr(); 2]);
        let (mut b, entry, args) = begin(&ctx, &bodies[0]);
        let sum = b.value(repr(), Some(origin())).unwrap();
        b.append(
            entry,
            Node::new(
                Op::Add {
                    a: vf(args[0], repr()),
                    b: vf(args[1], repr()),
                    out: vf(sum, repr()),
                    destructive: shape == Shape::Two,
                },
                &views,
            ),
        )
        .unwrap();
        let result = b.value(repr(), None).unwrap();
        let signature = p
            .view()
            .callables()
            .find(|c| c.key == source(1))
            .unwrap()
            .signature;
        b.append(
            entry,
            call_node(
                &ctx,
                signature,
                vec![vf(args[0], repr()), vf(sum, repr())],
                vec![vf(result, repr())],
                ArtifactId::Callable(source(1)),
                &views,
                &units,
            ),
        )
        .unwrap();
        ret(&mut b, entry, vec![], vec![], &views);
        let product = checked(b, shape);
        let receipt: SelectedReceipt<'_> = product.receipt();
        assert!(receipt.matches(&product));
        assert!(receipt.input().unwrap().same_snapshot(&bodies[0].receipt()));
        assert!(receipt
            .references()
            .contains(&ArtifactId::Callable(source(1))));
        let description = product.draft().describe().unwrap();
        assert_eq!(
            description.blocks[0].instructions[1].uses[0].value,
            args[0].id().index()
        );
        let mut inventory = SelectedProgramBuilder::new(&ctx);
        inventory.complete(&product, &receipt).unwrap();
        drop(product);
        let (mut b, entry, args) = begin(&ctx, &bodies[1]);
        let binding = AbiBinding {
            component: Component {
                ty: ScalarType::I64,
                role: ComponentRole::Result,
            },
            representation: repr(),
            location: AbiLocation::Slot {
                area: AbiArea::Results,
                index: 0,
            },
        };
        ret(
            &mut b,
            entry,
            vec![vf(args[0], repr())],
            vec![binding],
            &views,
        );
        let callee = checked(b, shape);
        inventory.complete(&callee, &callee.receipt()).unwrap();
        let selected: VerifiedSelectedProgram<'_> = inventory.finish(&program).unwrap();
        assert!(std::ptr::eq(selected.context(), &ctx));
        assert_eq!(selected.receipts().len(), 2);
    }
}
#[test]
pub(super) fn division_selection_exposes_guard_correction_blocks_and_join_arguments() {
    let p = CheckedPlan::check(supplied(Shape::Two)).unwrap();
    let (program, bodies) = inventory(&p);
    let extension = lir::TargetDeclarations::new(program.parent())
        .freeze()
        .unwrap();
    let (r, views, _) = resources();
    let ctx = context(&extension, r, vec![repr(); 2]);
    let (mut b, entry, args) = begin(&ctx, &bodies[0]);
    let fail = b.block(&[], None).unwrap();
    let success = b.block(&[], None).unwrap();
    let correction = b.block(&[], None).unwrap();
    let q = b.value(repr(), None).unwrap();
    let remainder = b.value(repr(), None).unwrap();
    let corrected = b.value(repr(), None).unwrap();
    let joined = b.value(repr(), None).unwrap();
    let joined_remainder = b.value(repr(), None).unwrap();
    let join = b.block(&[joined, joined_remainder], None).unwrap();
    b.terminate(
        entry,
        Node::new(
            Op::Branch {
                condition: vf(args[1], repr()),
                successors: 2,
            },
            &views,
        ),
        &[(success, vec![]), (fail, vec![])],
    )
    .unwrap();
    b.terminate(fail, Node::new(Op::Trap, &views), &[]).unwrap();
    b.append(
        success,
        Node::new(
            Op::Divide {
                numerator: vf(args[0], repr()),
                divisor: vf(args[1], repr()),
                quotient: vf(q, repr()),
                remainder: vf(remainder, repr()),
            },
            &views,
        ),
    )
    .unwrap();
    b.terminate(
        success,
        Node::new(
            Op::Branch {
                condition: vf(args[0], repr()),
                successors: 2,
            },
            &views,
        ),
        &[(join, vec![q, remainder]), (correction, vec![])],
    )
    .unwrap();
    b.append(
        correction,
        Node::new(
            Op::Add {
                a: vf(q, repr()),
                b: vf(args[1], repr()),
                out: vf(corrected, repr()),
                destructive: true,
            },
            &views,
        ),
    )
    .unwrap();
    b.terminate(
        correction,
        Node::new(Op::Jump, &views),
        &[(join, vec![corrected, remainder])],
    )
    .unwrap();
    let signature = p
        .view()
        .callables()
        .find(|c| c.key == source(1))
        .unwrap()
        .signature;
    for inputs in [
        vec![vf(joined, repr()), vf(joined_remainder, repr())],
        args.iter().map(|v| vf(*v, repr())).collect(),
    ] {
        let out = b.value(repr(), None).unwrap();
        b.append(
            join,
            call_node(
                &ctx,
                signature,
                inputs,
                vec![vf(out, repr())],
                ArtifactId::Callable(source(1)),
                &views,
                &[],
            ),
        )
        .unwrap();
    }
    ret(&mut b, join, vec![], vec![], &views);
    let product = checked(b, Shape::Two);
    let graph = graph::check_graph(product.draft()).unwrap();
    assert_eq!(graph.predecessors(join.id().index()).unwrap().len(), 2);
    assert!(graph
        .dominates(success.id().index(), join.id().index())
        .unwrap());
}
#[test]
pub(super) fn loop_swaps_parallel_successors_and_critical_edges_remain_simultaneous() {
    let p = CheckedPlan::check(supplied(Shape::Three)).unwrap();
    let (program, bodies) = inventory(&p);
    let extension = lir::TargetDeclarations::new(program.parent())
        .freeze()
        .unwrap();
    let (r, views, _) = resources();
    let ctx = context(&extension, r, vec![repr(); 2]);
    let (mut b, entry, args) = begin(&ctx, &bodies[0]);
    let x = b.value(repr(), None).unwrap();
    let y = b.value(repr(), None).unwrap();
    let header = b.block(&[x, y], None).unwrap();
    let tail = b.block(&[], None).unwrap();
    let exit = b.block(&[], None).unwrap();
    b.terminate(
        entry,
        Node::new(Op::Jump, &views),
        &[(header, args.clone())],
    )
    .unwrap();
    b.terminate(
        header,
        Node::new(
            Op::Branch {
                condition: vf(x, repr()),
                successors: 2,
            },
            &views,
        ),
        &[(tail, vec![]), (exit, vec![])],
    )
    .unwrap();
    let next = b.value(repr(), None).unwrap();
    b.append(
        tail,
        Node::new(
            Op::Add {
                a: vf(x, repr()),
                b: vf(y, repr()),
                out: vf(next, repr()),
                destructive: false,
            },
            &views,
        ),
    )
    .unwrap();
    b.terminate(
        tail,
        Node::new(
            Op::Branch {
                condition: vf(next, repr()),
                successors: 2,
            },
            &views,
        ),
        &[(header, vec![next, x]), (header, vec![y, x])],
    )
    .unwrap();
    ret(&mut b, exit, vec![], vec![], &views);
    let product = checked(b, Shape::Three);
    let graph = graph::check_graph(product.draft()).unwrap();
    assert_eq!(graph.predecessors(header.id().index()).unwrap().len(), 3);
    let desc = product.draft().describe().unwrap();
    let edges = &desc.blocks[tail.id().index()].terminal.as_ref().unwrap().1;
    assert_eq!(edges[1].arguments[0].value, y.id().index());
    assert_eq!(edges[1].arguments[1].value, x.id().index());
}
#[test]
pub(super) fn hidden_destination_receiver_and_mixed_banks_keep_exact_components() {
    for shape in [Shape::Two, Shape::Three] {
        let mut f = supplied(shape);
        let layout = f.add_layout(f.layouts[0]).unwrap();
        let mut inputs = vec![Component {
            ty: ScalarType::DataAddress,
            role: ComponentRole::ResultDestination(layout),
        }];
        inputs.extend(
            [
                ComponentRole::ReceiverStatic,
                ComponentRole::ReceiverComplete,
                ComponentRole::ReceiverMetadata,
            ]
            .map(|role| Component {
                ty: ScalarType::DataAddress,
                role,
            }),
        );
        inputs.extend((0..7).map(|i| Component {
            ty: ScalarType::I64,
            role: ComponentRole::Parameter(i),
        }));
        inputs.extend((7..16).map(|i| Component {
            ty: ScalarType::F64,
            role: ComponentRole::Parameter(i),
        }));
        f.signatures[0].inputs = inputs;
        f.signatures[0].returns = ReturnShape::Aggregate(layout);
        f.callables[1].signature = f.callables[0].signature;
        let p = CheckedPlan::check(f).unwrap();
        let (program, bodies) = inventory(&p);
        let extension = lir::TargetDeclarations::new(program.parent())
            .freeze()
            .unwrap();
        let (mut r, views, _) = resources();
        let float = r.bank(BankKind::Float);
        let u = r.unit().unwrap();
        let float_view = r.view(float, 64, &[u], false).unwrap();
        let reps: Vec<_> = p
            .view()
            .callable(source(0))
            .unwrap()
            .signature()
            .unwrap()
            .inputs
            .iter()
            .map(|c| match c.ty {
                ScalarType::DataAddress => {
                    Representation::new(RepresentationKind::DataAddress, 64).unwrap()
                }
                ScalarType::F64 => Representation::new(RepresentationKind::Float, 64).unwrap(),
                _ => repr(),
            })
            .collect();
        let ctx = context(&extension, r, reps.clone());
        let (mut b, entry, args) = begin(&ctx, &bodies[0]);
        assert_eq!(args.len(), 20);
        let signature = p
            .view()
            .callables()
            .find(|c| c.key == source(0))
            .unwrap()
            .signature;
        let code = Representation::new(RepresentationKind::CodeAddress(signature), 64).unwrap();
        let secured = b.value(code, Some(origin())).unwrap();
        b.append(
            entry,
            Node::new(
                Op::Address {
                    out: vf(secured, code),
                    artifact: ArtifactId::Callable(source(1)),
                },
                &views,
            ),
        )
        .unwrap();
        let mut call = call_node(
            &ctx,
            signature,
            args.iter()
                .zip(reps.iter())
                .map(|(v, ty)| vf(*v, *ty))
                .collect(),
            vec![],
            ArtifactId::Callable(source(1)),
            &views,
            &[],
        );
        if let Op::Call { target, abi, .. } = &mut call.op {
            *target = Some(vf(secured, code));
            let mut input = abi.inputs().to_vec();
            for (binding, view) in input.iter_mut().take(4).zip(&views) {
                binding.location = AbiLocation::Fixed(*view);
            }
            input[11].location = AbiLocation::Fixed(float_view);
            *abi = ctx.abi_bindings(signature, input, vec![]).unwrap();
            assert_eq!(
                abi.inputs()
                    .iter()
                    .filter(|b| matches!(b.location, AbiLocation::Fixed(_)))
                    .count(),
                5
            );
            assert_eq!(
                abi.inputs()
                    .iter()
                    .filter(|b| matches!(b.location, AbiLocation::Slot { .. }))
                    .count(),
                15
            );
        }
        b.append(entry, call).unwrap();
        ret(&mut b, entry, vec![], vec![], &views);
        let product = checked(b, shape);
        assert_eq!(
            product.draft().abi().unwrap().inputs()[0].component.role,
            ComponentRole::ResultDestination(layout)
        );
        assert_eq!(
            product
                .draft()
                .abi()
                .unwrap()
                .inputs()
                .iter()
                .filter(|b| b.representation.bank() == BankKind::Float)
                .count(),
            9
        );
    }
}
#[test]
pub(super) fn release_uses_original_header_after_finalizer_and_rejects_omitted_trace() {
    let mut f = facts();
    let addr = Representation::new(RepresentationKind::DataAddress, 64).unwrap();
    f.signatures[0].inputs = vec![Component {
        ty: ScalarType::DataAddress,
        role: ComponentRole::Parameter(0),
    }];
    let services = plan::test_fixtures::runtime_declarations(&mut f);
    let free = services[&plan::RuntimeService::Free];
    let p = CheckedPlan::check(f).unwrap();
    let (program, bodies) = inventory(&p);
    let extension = lir::TargetDeclarations::new(program.parent())
        .freeze()
        .unwrap();
    let (r, views, units) = resources();
    let ctx = context(&extension, r, vec![addr]);
    let (mut b, entry, args) = begin(&ctx, &bodies[0]);
    let count = b.value(repr(), None).unwrap();
    b.append(
        entry,
        Node::new(
            Op::Load {
                address: vf(args[0], addr),
                out: vf(count, repr()),
            },
            &views,
        ),
    )
    .unwrap();
    let immortal = b.block(&[], None).unwrap();
    let ordinary = b.block(&[], None).unwrap();
    let last = b.block(&[], None).unwrap();
    let done = b.block(&[], None).unwrap();
    b.terminate(
        entry,
        Node::new(
            Op::Branch {
                condition: vf(count, repr()),
                successors: 2,
            },
            &views,
        ),
        &[(immortal, vec![]), (ordinary, vec![])],
    )
    .unwrap();
    ret(&mut b, immortal, vec![], vec![], &views);
    b.terminate(
        ordinary,
        Node::new(
            Op::Branch {
                condition: vf(count, repr()),
                successors: 2,
            },
            &views,
        ),
        &[(last, vec![]), (done, vec![])],
    )
    .unwrap();
    let signature = p
        .view()
        .callables()
        .find(|c| c.key == source(1))
        .unwrap()
        .signature;
    b.append(
        last,
        call_node(
            &ctx,
            signature,
            vec![vf(args[0], addr)],
            vec![],
            ArtifactId::Callable(source(1)),
            &views,
            &units,
        ),
    )
    .unwrap();
    b.append(
        last,
        call_node(
            &ctx,
            free,
            vec![vf(args[0], addr)],
            vec![],
            ArtifactId::Runtime(plan::RuntimeService::Free),
            &views,
            &units,
        ),
    )
    .unwrap();
    ret(&mut b, last, vec![], vec![], &views);
    ret(&mut b, done, vec![], vec![], &views);
    let product = checked(b, Shape::Two);
    let desc = product.draft().describe().unwrap();
    assert_eq!(
        desc.blocks[last.id().index()].instructions[1].uses[0].value,
        args[0].id().index()
    );
    drop(product);
    let (mut b, entry, _) = begin(&ctx, &bodies[0]);
    b.append(entry, Node::new(Op::Trace, &views)).unwrap();
    ret(&mut b, entry, vec![], vec![], &views);
    let draft = b.finish();
    let target = WitnessTarget {
        profile: p.view().profile(),
        shape: Shape::Two,
        reject: false,
    };
    assert!(reasons(verify_selected(draft, &target)).contains(&SelectedReason::Effect));
}
#[test]
pub(super) fn resource_extensions_and_thunk_receipts_are_context_and_snapshot_bound() {
    let p = CheckedPlan::check(facts()).unwrap();
    let (program, bodies) = inventory(&p);
    let thunk = plan::LirCallableId::TargetThunk(plan::TargetThunkKey {
        family: 1,
        specialization: 0,
    });
    let mut extension = lir::TargetDeclarations::new(program.parent());
    extension
        .declare(plan::ArtifactDeclaration {
            key: ArtifactId::Callable(thunk),
            signature: Some(p.view().callables().next().unwrap().signature),
            layout: None,
        })
        .unwrap();
    let extension = extension.freeze().unwrap();
    let (mut r, views, _) = resources();
    let float = r.bank(BankKind::Float);
    let lo = r.unit().unwrap();
    let hi = r.unit().unwrap();
    let narrow = r.view(float, 64, &[lo], false).unwrap();
    let wide = r.view(float, 128, &[lo, hi], false).unwrap();
    assert!(r.preserved(narrow, &[lo]).unwrap());
    assert!(!r.preserved(wide, &[lo]).unwrap());
    let ctx = context(&extension, r, vec![]);
    let make = || {
        let mut b = SelectedBuilder::<Node>::new(&ctx, thunk, None).unwrap();
        let entry = b.block(&[], None).unwrap();
        let bindings = ctx
            .abi_bindings(
                p.view().callables().next().unwrap().signature,
                vec![],
                vec![],
            )
            .unwrap();
        b.entry(entry, &[], bindings).unwrap();
        ret(&mut b, entry, vec![], vec![], &views);
        checked(b, Shape::Two)
    };
    let product = make();
    assert!(product.receipt().input().is_none());
    let other = make();
    let mut inventory = SelectedProgramBuilder::new(&ctx);
    assert_eq!(
        inventory.complete(&product, &other.receipt()),
        Err(lir::ProgramError::StaleReceipt)
    );
    inventory.complete(&product, &product.receipt()).unwrap();
    assert!(inventory.finish(&program).is_err());
    let foreign = context(&extension, ResourceCatalog::default(), vec![]);
    assert_eq!(
        product.receipt().require_context(&foreign),
        Err(plan::PlanError::WrongContext)
    );
    assert_eq!(bodies.len(), 2);
    assert_ne!(
        std::any::TypeId::of::<graph::LoweredValueId>(),
        std::any::TypeId::of::<SelectedValueId>()
    );
}
