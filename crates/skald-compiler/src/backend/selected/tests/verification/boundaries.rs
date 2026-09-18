use super::*;

#[test]
fn one_context_publishes_calls_with_distinct_slot_zero_shapes() {
    let mut facts = facts();
    let integer = repr();
    let float = Representation::from_scalar(ScalarType::F64, 64).unwrap();
    facts.signatures[0].inputs = vec![
        Component {
            ty: ScalarType::I64,
            role: ComponentRole::Parameter(0),
        },
        Component {
            ty: ScalarType::F64,
            role: ComponentRole::Parameter(1),
        },
    ];
    let mut callee = facts.signatures[0].clone();
    callee.inputs.truncate(1);
    let integers = facts.add_signature(callee.clone()).unwrap();
    callee.inputs[0].ty = ScalarType::F64;
    let floats = facts.add_signature(callee).unwrap();
    facts.callables[1].signature = integers;
    facts.callables.push(plan::CallableDeclaration {
        key: source(2),
        signature: floats,
        body: plan::BodyDisposition::Required,
    });
    facts
        .executable_sources
        .insert(crate::identity::FunctionId::new(2).into());
    let plan = CheckedPlan::check(facts).unwrap();
    let lower = lower(&plan, source(0));
    let catalog = lir::TargetDeclarations::new(plan.view()).freeze().unwrap();
    let (resources, views, units) = resources();
    let mut context = SelectionContext::new(&catalog, resources);
    for (signature, representations) in [
        (
            plan.view().callables().next().unwrap().signature,
            vec![integer, float],
        ),
        (integers, vec![integer]),
        (floats, vec![float]),
    ] {
        context = context
            .with_abi_areas(
                signature,
                AbiAreas {
                    incoming: representations.clone(),
                    outgoing: representations,
                    results: vec![],
                },
            )
            .unwrap();
    }
    let (mut builder, entry, inputs) = begin(&context, &lower);
    for (signature, argument, representation, target) in [
        (integers, inputs[0], integer, source(1)),
        (floats, inputs[1], float, source(2)),
    ] {
        builder
            .append(
                entry,
                call_node(
                    &context,
                    signature,
                    vec![vf(argument, representation)],
                    vec![],
                    ArtifactId::Callable(target),
                    &views,
                    &units,
                ),
            )
            .unwrap();
    }
    ret(&mut builder, entry, vec![], vec![], &views);
    let body = checked(builder, Shape::Two);
    let mut inventory = SelectedProgramBuilder::new(&context);
    inventory.complete(&body, &body.receipt()).unwrap();
    // The same slot index is strict within its signature, including its bank.
    let signature = plan
        .view()
        .signature(plan.view().signature_id(floats.index()).unwrap())
        .unwrap();
    let wrong = AbiBinding {
        component: signature.inputs[0],
        representation: float,
        location: AbiLocation::Slot {
            area: AbiArea::Outgoing,
            index: 1,
        },
    };
    assert!(context.abi_bindings(floats, vec![wrong], vec![]).is_err());
    assert!(context
        .require_slot(floats, AbiArea::Outgoing, 0, integer)
        .is_err());
    assert!(context
        .require_slot(floats, AbiArea::Results, 0, float)
        .is_err());
}

#[test]
fn omitted_mode_keeps_call_trace_barriers_without_generated_tls_accesses() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let lower = lower(&plan, source(0));
    let catalog = lir::TargetDeclarations::new(plan.view()).freeze().unwrap();
    let (resources, views, units) = resources();
    let context = SelectionContext::new(&catalog, resources);
    let (mut builder, entry, _) = begin(&context, &lower);
    let signature = context.binding(source(1)).unwrap().signature_id();
    let mut call = call_node(
        &context,
        signature,
        vec![],
        vec![],
        ArtifactId::Callable(source(1)),
        &views,
        &units,
    );
    call.effects = Effects::new([
        Effect::Call,
        Effect::Read(crate::backend::effects::MemoryRegion::Unknown),
        Effect::TraceState,
    ]);
    builder.append(entry, call).unwrap();
    ret(&mut builder, entry, vec![], vec![], &views);
    let body = checked(builder, Shape::Two);
    body.visit(|fact| {
        if let SelectedFact::Instruction { payload, .. } = fact {
            assert!(!payload
                .describe()
                .artifacts
                .iter()
                .any(|(id, _)| *id == ArtifactId::TraceTls));
        }
        Ok::<_, std::convert::Infallible>(())
    })
    .unwrap();
    // A non-call trace action still needs enabled policy and TLS authority.
    let (mut builder, entry, _) = begin(&context, &lower);
    builder.append(entry, Node::new(Op::Trace, &views)).unwrap();
    ret(&mut builder, entry, vec![], vec![], &views);
    assert!(reasons(verify_selected(
        builder.finish(),
        &WitnessTarget {
            profile: context.catalog().plan().profile(),
            shape: Shape::Two,
            reject: false,
        },
    ))
    .contains(&SelectedReason::Effect));
}

#[test]
fn shared_indirect_events_accept_target_chosen_early_and_late_uses_but_require_typed_use() {
    for timing in [Timing::Early, Timing::Late] {
        for invalid in [false, true] {
            let plan = CheckedPlan::check(facts()).unwrap();
            let lower = lower(&plan, source(0));
            let catalog = lir::TargetDeclarations::new(plan.view()).freeze().unwrap();
            let (resources, views, units) = resources();
            let context = SelectionContext::new(&catalog, resources);
            let (mut builder, entry, _) = begin(&context, &lower);
            let signature = context.binding(source(1)).unwrap().signature_id();
            let code = Representation::from_scalar(ScalarType::CodeAddress(signature), 64).unwrap();
            let address = builder.value(code, None).unwrap();
            builder
                .append(
                    entry,
                    Node::new(
                        Op::Address {
                            out: vf(address, code),
                            artifact: ArtifactId::Callable(source(1)),
                        },
                        &views,
                    ),
                )
                .unwrap();
            let mut call = call_node(
                &context,
                signature,
                vec![],
                vec![],
                ArtifactId::Callable(source(1)),
                &views,
                &units,
            );
            if let Op::Call { target, .. } = &mut call.op {
                *target = Some(vf(address, code));
            }
            call.indirect_timing = timing;
            // Use general register alternatives; the synthetic target does not
            // inherit a native fixed-register choice.
            builder.append(entry, call).unwrap();
            ret(&mut builder, entry, vec![], vec![], &views);
            let mut draft = builder.finish();
            if invalid {
                if let Op::Call {
                    target: Some(target),
                    ..
                } = &mut draft.blocks.get_mut(entry).unwrap().instructions[1].op
                {
                    target.ty = repr();
                }
            }
            let result = verify_selected(
                draft,
                &WitnessTarget {
                    profile: context.catalog().plan().profile(),
                    shape: Shape::Three,
                    reject: false,
                },
            );
            if invalid {
                assert!(reasons(result).contains(&SelectedReason::Abi));
            } else {
                assert!(result.is_ok());
            }
        }
    }
}
