use crate::{
    backend::{
        lir::{CallAttribution, CallTarget, DataInitializer, Operation, Terminator, TraceAction},
        lowering::{lower_program, LowerError},
        plan::{ArtifactId, Convention, DataKey, LirCallableId, RuntimeService},
        planning::admit,
        BackendInput,
    },
    test_support::lower_source_to_complete_final_mir_with_sources,
};

#[test]
fn object_aliases_and_direct_receivers_cross_role_based_lowering() {
    use crate::backend::plan::{ComponentRole, ScalarType};

    let fixture = lower_source_to_complete_final_mir_with_sources(
        "aggregate-calls.ska",
        "class Pair { tag: u8; value: i64; init(value: i64) { self.tag = 1u8; self.value = value; } fn read() -> i64 { return self.value; } } \
         fn invoke(ref pair: Pair) -> i64 { return pair.read(); } \
         fn inspect_value(pair: Pair) -> i64 { return pair.value; } \
         fn main() -> i64 { var pair: Pair = Pair(7); return invoke(pair) + inspect_value(pair); }",
    );
    let admitted =
        admit(BackendInput::without_runtime_trace(&fixture.mir).with_reachable_artifacts_only())
            .unwrap();

    let definitions = fixture
        .mir
        .program()
        .executable_definitions()
        .filter(|definition| {
            definition.receiver().is_some()
                || definition.storage_entries().iter().any(|storage| {
                    matches!(
                        storage.kind,
                        crate::mir::MirStorageKind::AliasParameter(_)
                            | crate::mir::MirStorageKind::Parameter
                    ) && matches!(storage.ty, crate::mir::MirType::Class(_))
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(definitions.len(), 4); // initializer, method, alias, aggregate value

    let mut receiver_call = false;
    let mut projected_load = false;
    for definition in definitions {
        let owner = admitted
            .plan()
            .view()
            .callable(LirCallableId::Source(definition.callable()))
            .unwrap();
        let body = super::super::context::Lowerer::new(&admitted, owner)
            .unwrap()
            .finish()
            .unwrap();
        let draft = body.draft();
        let signature = owner.signature().unwrap();
        if definition.receiver().is_some() {
            assert!(signature.inputs.iter().any(|component| {
                component.role == ComponentRole::ReceiverStatic
                    && component.ty == ScalarType::DataAddress
            }));
        }
        if definition.storage_entries().iter().any(|storage| {
            storage.kind == crate::mir::MirStorageKind::Parameter
                && matches!(storage.ty, crate::mir::MirType::Class(_))
        }) {
            assert!(signature.inputs.iter().any(|component| {
                matches!(component.role, ComponentRole::AggregateAddress { .. })
            }));
            assert_eq!(draft.objects().len(), 0);
        }
        for (_, block) in draft.blocks() {
            for instruction in &block.instructions {
                match &instruction.operation {
                    Operation::Call(call)
                        if call
                            .arguments
                            .iter()
                            .any(|argument| argument.role == ComponentRole::ReceiverMetadata) =>
                    {
                        receiver_call = true;
                        assert_eq!(call.arguments.len(), 3);
                    }
                    Operation::ByteOffset { .. } => projected_load = true,
                    _ => {}
                }
            }
        }
    }
    assert!(receiver_call);
    assert!(projected_load);
}

#[test]
fn direct_indirect_external_and_unit_calls_close_the_entire_inventory() {
    let fixture = lower_source_to_complete_final_mir_with_sources("calls.ska", "extern fn foreign(a: i64, b: f64) -> i64; fn identity(a: i64) -> i64 { return a; } fn ignore(a: i64) -> unit { } fn invoke(f: fn(i64) -> i64, a: i64) -> i64 { return f(a); } fn main() -> i64 { ignore(1); return foreign(invoke(identity, 7), 2.5); }");
    let admitted = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let mut direct = 0;
    let mut indirect = 0;
    let mut external = 0;
    let program = lower_program(&admitted, |body| {
        assert!(body.draft().trace_plan().is_none());
        for (_, block) in body.draft().blocks() {
            for instruction in &block.instructions {
                if let Operation::Call(call) = &instruction.operation {
                    let signature = admitted
                        .plan()
                        .view()
                        .signature(
                            admitted
                                .plan()
                                .view()
                                .signature_id(call.signature.index())
                                .unwrap(),
                        )
                        .unwrap();
                    match call.target {
                        CallTarget::Direct(ArtifactId::External(_)) => {
                            external += 1;
                            assert_eq!(signature.convention, Convention::ExternC);
                            assert_eq!(call.arguments.len(), 2);
                        }
                        CallTarget::Indirect(_) => {
                            indirect += 1;
                            assert_eq!(signature.convention, Convention::Language);
                        }
                        CallTarget::Direct(ArtifactId::Callable(_)) => direct += 1,
                        _ => {}
                    }
                    if matches!(call.attribution, CallAttribution::SourceOperation { .. }) {
                        assert!(matches!(
                            call.attribution,
                            CallAttribution::SourceOperation { location: None, .. }
                        ));
                    }
                }
            }
        }
        Ok(())
    })
    .unwrap();
    assert_eq!((direct, indirect, external), (3, 1, 1));
    assert_eq!(
        program.receipts().len(),
        fixture.mir.program().executable_definitions().count() + 1
    );
    assert_eq!(
        program.data().len(),
        crate::backend::failure::FailureMessage::ALL.len()
    );
}

#[test]
fn process_entry_calls_marker_then_main_and_returns_its_exact_result_without_a_trace_frame() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "entry.ska",
        "fn main() -> i64 { return 37; }",
    );
    let admitted = admit(BackendInput::with_runtime_trace(
        &fixture.mir,
        &fixture.sources,
    ))
    .unwrap();
    let mut entry_seen = false;
    let program = lower_program(&admitted, |body| {
        if body.receipt().owner().key() == LirCallableId::Entry {
            entry_seen = true;
            assert!(body.draft().trace_plan().is_none());
            assert_eq!(body.draft().objects().len(), 0);
            let block = body.draft().blocks().next().unwrap().1;
            assert_eq!(block.instructions.len(), 2);
            let targets = block
                .instructions
                .iter()
                .map(|instruction| {
                    let Operation::Call(call) = &instruction.operation else {
                        panic!("entry must consist of process calls");
                    };
                    assert_eq!(call.attribution, CallAttribution::ProcessBoundary);
                    call.target.clone()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                targets,
                vec![
                    CallTarget::Direct(ArtifactId::Runtime(RuntimeService::AbiMarker)),
                    CallTarget::Direct(ArtifactId::Callable(LirCallableId::Source(
                        fixture.mir.program().entry_function.into()
                    )))
                ]
            );
            assert_eq!(
                block.terminator,
                Some(Terminator::Return(block.instructions[1].results.clone()))
            );
        }
        Ok(())
    })
    .unwrap();
    assert!(entry_seen);
    assert_eq!(program.receipts().len(), 2);
}

#[test]
fn traced_calls_store_results_before_pop_and_failure_updates_are_failure_only() {
    use crate::backend::effects::Effect;
    let fixture = lower_source_to_complete_final_mir_with_sources("trace.ska", "fn divide(a: i64, b: i64) -> i64 { return a / b; } fn main() -> i64 { var result: i64 = divide(12, 3); return result; }");
    let admitted = admit(BackendInput::with_runtime_trace(
        &fixture.mir,
        &fixture.sources,
    ))
    .unwrap();
    let mut failures = 0;
    let mut calls = 0;
    let program = lower_program(&admitted, |body| {
        if body.receipt().owner().key() == LirCallableId::Entry {return Ok(());}
        let draft = body.draft();
        let trace = draft.trace_plan().unwrap();
        assert!(trace.locations.contains(&trace.initial_location.unwrap()));
        assert_eq!(draft.objects().find(|(id,_)|Some(*id)==trace.record).unwrap().1.layout.size,16);
        let entry = draft.blocks().find(|(id,_)|Some(*id)==draft.entry()).unwrap().1;
        assert!(matches!(entry.instructions[0].operation,Operation::Trace(TraceAction::PushFrame {..})));
        for (id,block) in draft.blocks() {
            for (ordinal,instruction) in block.instructions.iter().enumerate() {
                if let Operation::Call(call) = &instruction.operation {
                    calls += 1;
                    let CallAttribution::SourceOperation {location:Some(location),..}=call.attribution else {panic!("missing source location");};
                    assert!(matches!(block.instructions[ordinal-1].operation,Operation::Trace(TraceAction::ReplaceLocation {location:actual,..}) if actual==location));
                    let result = instruction.results[0];
                    assert!(block.instructions[ordinal+1..].iter().any(|i|matches!(i.operation,Operation::Store {value,..} if value==result)));
                }
            }
            match &block.terminator {
                Some(Terminator::ReportFailure {call,..}) => {
                    failures += 1;
                    let CallAttribution::SourceOperation {location:Some(location),..}=call.attribution else {panic!("missing failure location");};
                    assert!(matches!(block.instructions.last().unwrap().operation,Operation::Trace(TraceAction::ReplaceLocation {location:actual,site:crate::backend::lir::TraceSite::Terminator(target),..}) if actual==location && target==id));
                    let effects = block.terminal_effects.as_ref().unwrap();
                    assert!(effects.contains(Effect::Report) && effects.contains(Effect::HardTrap));
                    assert!(!block.instructions.iter().any(|i|matches!(i.operation,Operation::Trace(TraceAction::PopFrame {..}))));
                }
                Some(Terminator::ScalarCheck {..}) => assert!(!block.instructions.iter().any(|i|matches!(i.operation,Operation::Trace(TraceAction::ReplaceLocation {..})))),
                Some(Terminator::Return(values)) => {
                    assert!(matches!(block.instructions.last().unwrap().operation,Operation::Trace(TraceAction::PopFrame {..})));
                    for value in values {assert!(!block.instructions.last().unwrap().results.contains(value));}
                }
                _ => {}
            }
        }
        Ok(())
    }).unwrap();
    assert_eq!((calls, failures), (1, 1));
    assert!(program
        .data()
        .any(|data| matches!(data.key, DataKey::TraceLocation(_))
            && matches!(
                data.initializers[0],
                DataInitializer::Address {
                    target: ArtifactId::Data(DataKey::TraceContext(_)),
                    ..
                }
            )));
}

#[test]
fn omitted_lowering_needs_no_sources_and_requests_no_trace_data_or_tls() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "omit.ska",
        "fn main() -> i64 { return 12 / 3; }",
    );
    let admitted = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let program = lower_program(&admitted, |body| {
        assert!(body.draft().trace_plan().is_none());
        assert!(!body.receipt().references().contains(&ArtifactId::TraceTls));
        assert!(body
            .draft()
            .blocks()
            .flat_map(|(_, b)| &b.instructions)
            .all(|i| !matches!(i.operation, Operation::Trace(_))));
        Ok(())
    })
    .unwrap();
    assert!(program
        .data()
        .all(|data| matches!(data.key, DataKey::FailureMessage(_))));
    assert!(admitted.trace().requests.is_empty());
}

#[test]
fn a_consumer_failure_cannot_publish_complete_program_authority() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "consumer.ska",
        "fn main() -> i64 { return 0; }",
    );
    let admitted = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let mut visited = 0;
    assert!(lower_program(&admitted, |_| {
        visited += 1;
        Err(LowerError::Plan(
            crate::backend::plan::PlanError::InvalidDomain,
        ))
    })
    .is_err());
    assert_eq!(visited, 1);
}

#[test]
fn an_indirect_target_cannot_borrow_another_canonical_signature() {
    use crate::backend::{
        lir::{BuildError, Call, CallArgument, Constant, DraftBuilder},
        plan::{ComponentRole, ScalarType},
    };
    let fixture = lower_source_to_complete_final_mir_with_sources("signatures.ska", "fn integer(a: i64) -> i64 { return a; } fn floating(a: f64) -> f64 { return a; } fn main() -> i64 { var i: fn(i64) -> i64 = integer; var f: fn(f64) -> f64 = floating; return i(1); }");
    let admitted = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let plan = admitted.plan().view();
    let mut signatures = admitted
        .program()
        .function_types
        .iter()
        .map(|ty| admitted.function_type(ty.id).unwrap());
    let first = signatures.next().unwrap();
    let other = signatures.find(|id| *id != first).unwrap();
    let mut builder = DraftBuilder::new(
        plan.callable(LirCallableId::Source(
            admitted.program().entry_function.into(),
        ))
        .unwrap(),
    )
    .unwrap();
    let block = builder.reserve_block().unwrap();
    builder.define_block(block, &[]).unwrap();
    builder.set_entry(block).unwrap();
    let target = builder
        .reserve_value(ScalarType::CodeAddress(other), None)
        .unwrap();
    let signature = plan
        .signature(plan.signature_id(first.index()).unwrap())
        .unwrap();
    let constant = match signature.inputs[0].ty {
        ScalarType::I64 => Constant::I64(1),
        ScalarType::F64 => Constant::F64(1.0_f64.to_bits()),
        _ => panic!("fixture signature"),
    };
    let argument = builder
        .append(block, Operation::Constant(constant))
        .unwrap()[0];
    assert!(matches!(
        builder.append(
            block,
            Operation::Call(Call {
                target: CallTarget::Indirect(target),
                signature: first,
                arguments: vec![CallArgument {
                    role: ComponentRole::Parameter(0),
                    value: argument
                }],
                attribution: CallAttribution::SourceOperation {
                    origin: admitted.program().span,
                    location: None
                }
            })
        ),
        Err(BuildError::InvalidCall)
    ));
}

#[test]
fn sparse_receiverless_static_calls_close_without_resurrecting_lifecycle_bodies() {
    use crate::mir::retain::{prepare_reachable_definition_retention, MirDefinitionRetention};
    let fixture = lower_source_to_complete_final_mir_with_sources("static.ska", "class Math { init() {} static fn add(x: i64) -> i64 { return x + 1; } } fn main() -> i64 { return Math.add(4); }");
    let MirDefinitionRetention::Changed(retention) =
        prepare_reachable_definition_retention(fixture.mir.program(), fixture.mir.reachability())
            .unwrap()
    else {
        panic!("unused initializer must be removed");
    };
    let sparse =
        crate::passes::verify_final_mir(retention.apply(fixture.mir.program().clone()).program)
            .unwrap();
    let admitted = admit(
        BackendInput::with_runtime_trace(&sparse, &fixture.sources).with_reachable_artifacts_only(),
    )
    .unwrap();
    let mut method_calls = 0;
    let program = lower_program(&admitted, |body| {
        for (_, block) in body.draft().blocks() {
            for instruction in &block.instructions {
                if let Operation::Call(call) = &instruction.operation {
                    if matches!(
                        call.target,
                        CallTarget::Direct(ArtifactId::Callable(LirCallableId::Source(
                            crate::identity::CallableId::Method(_)
                        )))
                    ) {
                        method_calls += 1;
                    }
                }
            }
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(method_calls, 1);
    assert_eq!(program.receipts().len(), 4);
}
