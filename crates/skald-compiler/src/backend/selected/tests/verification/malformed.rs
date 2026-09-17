use super::*;
#[test]
fn descriptor_failures_are_independent_of_append_and_target_checks() {
    let p = CheckedPlan::check(supplied(Shape::Two)).unwrap();
    let (program, bodies) = inventory(&p);
    let extension = lir::TargetDeclarations::new(program.parent())
        .freeze()
        .unwrap();
    let (mut r, views, _) = resources();
    let bank = r.bank(BankKind::Integer);
    let u = r.unit().unwrap();
    let byte = r.view(bank, 8, &[u], false).unwrap();
    let ctx = context(&extension, r, vec![repr(); 2]);
    for (corruption, expected) in [
        (Corruption::TieSlot, SelectedReason::Tie),
        (Corruption::TieTiming, SelectedReason::Timing),
        (Corruption::MissingScratch, SelectedReason::Bundle),
        (Corruption::WrongFlow, SelectedReason::Flow),
        (Corruption::FixedWidth, SelectedReason::Resource),
    ] {
        let (mut b, entry, args) = begin(&ctx, &bodies[0]);
        let out = b.value(repr(), None).unwrap();
        let node = Node::new(
            Op::Add {
                a: vf(args[0], repr()),
                b: vf(args[1], repr()),
                out: vf(out, repr()),
                destructive: true,
            },
            &views,
        );
        b.append(entry, node).unwrap();
        ret(&mut b, entry, vec![], vec![], &views);
        let mut draft = b.finish();
        let node = &mut draft.blocks.get_mut(entry).unwrap().instructions[0];
        node.corrupt = Some(corruption);
        if matches!(corruption, Corruption::FixedWidth) {
            node.views[0] = byte;
        }
        let target = WitnessTarget {
            profile: p.view().profile(),
            shape: Shape::Two,
            reject: false,
        };
        assert!(reasons(verify_selected(draft, &target)).contains(&expected));
    }
    let signature = p
        .view()
        .callables()
        .find(|c| c.key == source(1))
        .unwrap()
        .signature;
    for corruption in [
        Corruption::MissingReference,
        Corruption::MissingEffect,
        Corruption::WrongAbi,
    ] {
        let (mut b, entry, args) = begin(&ctx, &bodies[0]);
        let out = b.value(repr(), None).unwrap();
        b.append(
            entry,
            call_node(
                &ctx,
                signature,
                args.iter().map(|v| vf(*v, repr())).collect(),
                vec![vf(out, repr())],
                ArtifactId::Callable(source(1)),
                &views,
                &[],
            ),
        )
        .unwrap();
        ret(&mut b, entry, vec![], vec![], &views);
        let mut draft = b.finish();
        draft.blocks.get_mut(entry).unwrap().instructions[0].corrupt = Some(corruption);
        let target = WitnessTarget {
            profile: p.view().profile(),
            shape: Shape::Two,
            reject: false,
        };
        assert!(!reasons(verify_selected(draft, &target)).is_empty());
    }
    let mut foreign_resources = ResourceCatalog::default();
    foreign_resources.unit().unwrap();
    foreign_resources.unit().unwrap();
    foreign_resources.unit().unwrap();
    foreign_resources.unit().unwrap();
    foreign_resources.unit().unwrap();
    let unknown = foreign_resources.unit().unwrap();
    let (mut b, entry, args) = begin(&ctx, &bodies[0]);
    let out = b.value(repr(), None).unwrap();
    let mut node = Node::new(
        Op::Add {
            a: vf(args[0], repr()),
            b: vf(args[1], repr()),
            out: vf(out, repr()),
            destructive: true,
        },
        &views,
    );
    node.corrupt = Some(Corruption::UnknownClobber);
    node.clobbers.push((Timing::Late, unknown));
    b.append(entry, node).unwrap();
    ret(&mut b, entry, vec![], vec![], &views);
    let target = WitnessTarget {
        profile: p.view().profile(),
        shape: Shape::Two,
        reject: false,
    };
    assert!(reasons(verify_selected(b.finish(), &target)).contains(&SelectedReason::Resource));
}
#[test]
fn shared_success_cannot_bypass_target_rejection_or_wrong_profile() {
    let p = CheckedPlan::check(supplied(Shape::Two)).unwrap();
    let (program, bodies) = inventory(&p);
    let extension = lir::TargetDeclarations::new(program.parent())
        .freeze()
        .unwrap();
    let (r, views, _) = resources();
    let ctx = context(&extension, r, vec![repr(); 2]);
    for wrong_profile in [false, true] {
        let (mut b, entry, _) = begin(&ctx, &bodies[0]);
        ret(&mut b, entry, vec![], vec![], &views);
        let mut target = WitnessTarget {
            profile: p.view().profile(),
            shape: Shape::Two,
            reject: true,
        };
        if wrong_profile {
            target.profile.architecture = plan::Architecture::Aarch64;
            target.profile.abi = plan::Abi::Aapcs64;
        }
        let errors = reasons(verify_selected(b.finish(), &target));
        assert!(errors.contains(&if wrong_profile {
            SelectedReason::Context(plan::PlanError::WrongTarget)
        } else {
            SelectedReason::Target("adversarial target rejection")
        }));
    }
}

#[test]
fn unsecured_division_and_malformed_edges_cannot_publish() {
    let p = CheckedPlan::check(supplied(Shape::Two)).unwrap();
    let (program, bodies) = inventory(&p);
    let extension = lir::TargetDeclarations::new(program.parent())
        .freeze()
        .unwrap();
    let (r, views, _) = resources();
    let ctx = context(&extension, r, vec![repr(); 2]);
    let (mut b, entry, args) = begin(&ctx, &bodies[0]);
    let quotient = b.value(repr(), None).unwrap();
    let remainder = b.value(repr(), None).unwrap();
    b.append(
        entry,
        Node::new(
            Op::Divide {
                numerator: vf(args[0], repr()),
                divisor: vf(args[1], repr()),
                quotient: vf(quotient, repr()),
                remainder: vf(remainder, repr()),
            },
            &views,
        ),
    )
    .unwrap();
    ret(&mut b, entry, vec![], vec![], &views);
    let target = WitnessTarget {
        profile: p.view().profile(),
        shape: Shape::Two,
        reject: false,
    };
    assert!(reasons(verify_selected(b.finish(), &target))
        .contains(&SelectedReason::Target("unsecured divisor")));

    let (mut b, entry, args) = begin(&ctx, &bodies[0]);
    let parameter = b.value(repr(), None).unwrap();
    let exit = b.block(&[parameter], None).unwrap();
    b.terminate(entry, Node::new(Op::Jump, &views), &[(exit, vec![args[0]])])
        .unwrap();
    ret(&mut b, exit, vec![], vec![], &views);
    let mut draft = b.finish();
    // Bypass constructor checks and omit the parameter transfer.
    draft
        .blocks
        .get_mut(entry)
        .unwrap()
        .terminal
        .as_mut()
        .unwrap()
        .edges[0]
        .1
        .clear();
    assert!(reasons(verify_selected(draft, &target))
        .iter()
        .any(|reason| matches!(reason, SelectedReason::Graph(_))));
}

#[test]
fn omitted_trace_objects_and_incompatible_call_origins_are_rejected() {
    let p = CheckedPlan::check(supplied(Shape::Two)).unwrap();
    let (program, bodies) = inventory(&p);
    let extension = lir::TargetDeclarations::new(program.parent())
        .freeze()
        .unwrap();
    let (r, views, _) = resources();
    let ctx = context(&extension, r, vec![repr(); 2]);
    let target = WitnessTarget {
        profile: p.view().profile(),
        shape: Shape::Two,
        reject: false,
    };
    let (mut b, entry, _) = begin(&ctx, &bodies[0]);
    b.object(facts().layouts[0], ObjectRole::Trace, Some(origin()))
        .unwrap();
    ret(&mut b, entry, vec![], vec![], &views);
    assert!(reasons(verify_selected(b.finish(), &target)).contains(&SelectedReason::Reference));
    let (mut b, entry, args) = begin(&ctx, &bodies[0]);
    let result = b.value(repr(), None).unwrap();
    let signature = p
        .view()
        .callables()
        .find(|c| c.key == source(1))
        .unwrap()
        .signature;
    let mut call = call_node(
        &ctx,
        signature,
        args.iter().map(|v| vf(*v, repr())).collect(),
        vec![vf(result, repr())],
        ArtifactId::Callable(source(1)),
        &views,
        &[],
    );
    call.attribution = lir::CallAttribution::SourceBodyFromOmittedHelper {
        boundary: source(0),
    };
    b.append(entry, call).unwrap();
    ret(&mut b, entry, vec![], vec![], &views);
    assert!(reasons(verify_selected(b.finish(), &target)).contains(&SelectedReason::Abi));
}

#[test]
fn call_bindings_cannot_reuse_incoming_slot_area() {
    let p = CheckedPlan::check(supplied(Shape::Two)).unwrap();
    let (program, bodies) = inventory(&p);
    let extension = lir::TargetDeclarations::new(program.parent())
        .freeze()
        .unwrap();
    let (r, views, _) = resources();
    let ctx = context(&extension, r, vec![repr(); 2]);
    let (mut b, entry, args) = begin(&ctx, &bodies[0]);
    let out = b.value(repr(), None).unwrap();
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
            args.iter().map(|v| vf(*v, repr())).collect(),
            vec![vf(out, repr())],
            ArtifactId::Callable(source(1)),
            &views,
            &[],
        ),
    )
    .unwrap();
    ret(&mut b, entry, vec![], vec![], &views);
    let mut draft = b.finish();
    if let Op::Call { abi, .. } = &mut draft.blocks.get_mut(entry).unwrap().instructions[0].op {
        let input = abi
            .inputs()
            .iter()
            .enumerate()
            .map(|(index, binding)| AbiBinding {
                location: AbiLocation::Slot {
                    area: AbiArea::Incoming,
                    index,
                },
                ..*binding
            })
            .collect();
        // Shape and slot existence alone pass; publication must enforce call role.
        *abi = ctx
            .abi_bindings(signature, input, abi.results().to_vec())
            .unwrap();
    }
    let target = WitnessTarget {
        profile: p.view().profile(),
        shape: Shape::Two,
        reject: false,
    };
    assert!(reasons(verify_selected(draft, &target)).contains(&SelectedReason::Abi));
}

#[test]
fn static_effects_are_typed_receipt_dependencies_and_inactive_statics_fail() {
    use crate::backend::effects::MemoryRegion;
    use crate::identity::{ClassId, StaticFieldId};
    let field = StaticFieldId::new(ClassId::new(0), 0);
    let artifact = ArtifactId::Data(plan::DataKey::Static(field));
    for active in [true, false] {
        let mut f = supplied(Shape::Two);
        let layout = f.add_layout(f.layouts[0]).unwrap();
        f.artifacts.push(plan::ArtifactDeclaration {
            key: artifact,
            signature: None,
            layout: Some(layout),
        });
        if active {
            f.active_statics.insert(field);
        }
        let p = CheckedPlan::check(f).unwrap();
        let bodies = [lower(&p, source(0)), lower(&p, source(1))];
        let mut builder = lir::ProgramBuilder::new(p.view());
        for body in &bodies {
            builder.begin(body.receipt().owner().key()).unwrap();
            builder.complete(body, &body.receipt()).unwrap();
        }
        if active {
            builder
                .define_data(lir::DataDefinition {
                    key: plan::DataKey::Static(field),
                    initializers: vec![lir::DataInitializer::Zero(8)],
                })
                .unwrap();
        }
        let program = builder.finish().unwrap();
        let extension = lir::TargetDeclarations::new(program.parent())
            .freeze()
            .unwrap();
        let (r, views, _) = resources();
        let ctx = context(&extension, r, vec![repr(); 2]);
        let (mut b, entry, args) = begin(&ctx, &bodies[0]);
        let out = b.value(repr(), None).unwrap();
        let mut node = Node::new(
            Op::Add {
                a: vf(args[0], repr()),
                b: vf(args[1], repr()),
                out: vf(out, repr()),
                destructive: true,
            },
            &views,
        );
        // Widened effects are valid even without an opcode's explicit artifact operand.
        node.effects = Effects::new([
            Effect::Read(MemoryRegion::Static(field)),
            Effect::Write(MemoryRegion::Static(field)),
        ]);
        b.append(entry, node).unwrap();
        ret(&mut b, entry, vec![], vec![], &views);
        let target = WitnessTarget {
            profile: p.view().profile(),
            shape: Shape::Two,
            reject: false,
        };
        let result = verify_selected(b.finish(), &target);
        if active {
            assert!(result.unwrap().receipt().references().contains(&artifact));
        } else {
            assert!(reasons(result).contains(&SelectedReason::Reference));
        }
    }
}
