use super::*;
use crate::backend::{
    lir::{CallTarget, Operation, Terminator},
    plan::{ArtifactId, DataKey, RuntimeService},
    BackendInput,
};
pub(super) fn complete(
    source: &str,
    trace: bool,
    mut check: impl for<'p> FnMut(&'p selected::SelectionContext<'p>, &lir::VerifiedCallable<'p>),
) {
    let fixture = lower_source_to_complete_final_mir_with_sources("native.ska", source);
    let input = if trace {
        BackendInput::with_runtime_trace(&fixture.mir, &fixture.sources)
    } else {
        BackendInput::without_runtime_trace(&fixture.mir)
    };
    let admitted = admit(input).unwrap();
    let catalog = TargetDeclarations::new(admitted.plan().view())
        .freeze()
        .unwrap();
    let context = selection_context(&catalog).unwrap();
    let mut worklist = ProgramBuilder::new(admitted.plan().view());
    while worklist.next().is_some() {
        let body = lower_next(&admitted, &mut worklist).unwrap().unwrap();
        check(&context, &body);
    }
}
#[test]
fn complete_direct_c_indirect_unit_and_entry_calls_select_under_both_trace_policies() {
    let source = "extern fn foreign(a: i64, b: f64) -> i64; fn identity(a: i64) -> i64 { return a; } fn ignore(a: i64) -> unit {} fn invoke(f: fn(i64) -> i64, a: i64) -> i64 { return f(a); } fn main() -> i64 { ignore(1); return foreign(invoke(identity, 7), 2.5); }";
    for trace in [false, true] {
        let mut indirect = 0;
        let mut external = 0;
        let mut entry = 0;
        let mut tls = 0;
        let mut calls = 0;
        complete(source, trace, |context, lower| {
            let requests = super::super::requests::requests(lower).unwrap();
            assert_eq!(requests, super::super::requests::requests(lower).unwrap());
            let selected = select(context, lower).unwrap();
            let mut actual_calls = 0;
            selected.visit(|fact| {
                if let SelectedFact::Instruction { payload, .. } | SelectedFact::Terminal { payload: Some(payload), .. } = fact {
                    let verifier = Verifier::new(lower.receipt().owner().context().profile()).unwrap();
                    if let Opcode::Call(call) = &payload.opcode {
                        if matches!(call.attribution, lir::CallAttribution::SourceOperation { location: Some(_), .. }) {
                            let mut malformed = payload.clone();
                            if let Opcode::Call(call) = &mut malformed.opcode { call.attribution = lir::CallAttribution::ProcessBoundary; }
                            malformed.refresh();
                            assert!(verifier.check_trace_attribution(selected.draft(), &malformed).is_err());
                        }
                    }
                    let desc = payload.describe();
                    if let Opcode::TlsAddress { .. } = payload.opcode { tls += 1; }
                    if let Opcode::Call(call) = &payload.opcode {
                        actual_calls += 1; calls += 1;
                        assert_eq!(desc.clobbers.len(), payload.resources.caller_clobbers().len());
                        assert!(desc.operands.iter().all(|op| op.timing == Timing::Late));
                        assert_eq!(desc.abi_inputs, call.inputs);
                        assert_eq!(desc.abi_results, call.outputs);
                        if let CallTarget::Indirect(target) = call.target {
                            indirect += 1;
                            let op = &desc.operands[desc.indirect_target.unwrap()];
                            assert_eq!(op.value, target.value);
                            assert!(matches!(op.constraint, Constraint::Fixed(view) if view == payload.resources.gpr(super::super::super::Gpr::R11, 64).unwrap()));
                        }
                        if matches!(call.target, CallTarget::Direct(ArtifactId::External(_))) { external += 1; }
                        let verifier = Verifier::new(lower.receipt().owner().context().profile()).unwrap();
                        let mut wrong_call = payload.clone();
                        if let Opcode::Call(call) = &mut wrong_call.opcode { call.never = !call.never; }
                        assert!(verifier.check_call(context, &wrong_call).is_err());
                        if !call.inputs.is_empty() {
                            let mut wrong_call = payload.clone();
                            if let Opcode::Call(call) = &mut wrong_call.opcode { call.inputs[0].location = selected::AbiLocation::Fixed(payload.resources.gpr(super::super::super::Gpr::Rsp, call.inputs[0].representation.bits()).unwrap()); }
                            wrong_call.refresh();
                            assert!(verifier.check_call(context, &wrong_call).is_err());
                        }
                        let mut malformed = payload.clone(); malformed.clobbers.pop();
                        assert!(verifier.verify_payload(&malformed, false).is_err());
                        malformed = payload.clone(); malformed.effects = Effects::default();
                        assert!(verifier.verify_payload(&malformed, false).is_err());
                        malformed.describe(); // Total even for an invalid cache.
                    }
                    if !trace {
                        assert!(!desc.artifacts.iter().any(|(key, _)| matches!(key, ArtifactId::TraceTls | ArtifactId::Data(DataKey::TraceContext(_) | DataKey::TraceLocation(_)))));
                        assert!(!matches!(payload.opcode, Opcode::TlsAddress { .. } | Opcode::TraceLoad { .. } | Opcode::TraceStore { .. }));
                    }
                }
                Ok::<_, std::convert::Infallible>(())
            }).unwrap();
            let lower_calls = lower
                .draft()
                .blocks()
                .flat_map(|(_, b)| &b.instructions)
                .filter(|i| matches!(i.operation, Operation::Call(_)))
                .count();
            assert_eq!(actual_calls, lower_calls);
            if lower.receipt().owner().key() == LirCallableId::Entry {
                entry += 1;
            }
        });
        assert_eq!((indirect, external, entry), (1, 1, 1));
        assert!(calls >= 6);
        assert_eq!(tls > 0, trace);
    }
}
#[test]
fn pressure_call_uses_signature_local_outgoing_slots_in_component_order() {
    let inputs = (0..7)
        .map(|i| format!("i{i}: i64"))
        .chain((0..9).map(|i| format!("f{i}: f64")))
        .collect::<Vec<_>>()
        .join(", ");
    let args = (0..7)
        .map(|i| i.to_string())
        .chain((0..9).map(|i| format!("{i}.5")))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!("extern fn pressure({inputs}) -> f64; fn main() -> i64 {{ var result: f64 = pressure({args}); return (i64) result; }}");
    let mut seen = false;
    complete(&source, false, |context, lower| {
        let selected = select(context, lower).unwrap();
        selected
            .visit(|fact| {
                if let SelectedFact::Instruction { payload, .. } = fact {
                    if let Opcode::Call(call) = &payload.opcode {
                        if matches!(call.target, CallTarget::Direct(ArtifactId::External(_))) {
                            seen = true;
                            let slots = call
                                .inputs
                                .iter()
                                .filter_map(|b| match b.location {
                                    selected::AbiLocation::Slot { area, index } => {
                                        Some((area, index, b.representation.kind))
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>();
                            assert_eq!(
                                slots,
                                vec![
                                    (
                                        selected::AbiArea::Outgoing,
                                        0,
                                        selected::RepresentationKind::Bits
                                    ),
                                    (
                                        selected::AbiArea::Outgoing,
                                        1,
                                        selected::RepresentationKind::Float
                                    )
                                ]
                            );
                        }
                    }
                }
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
    });
    assert!(seen);
}
#[test]
fn traced_numeric_failure_has_location_update_and_defensive_nonreturning_reporter() {
    let mut failure = false;
    complete("fn divide(a: i64, b: i64) -> i64 { return a / b; } fn main() -> i64 { return divide(6, 2); }", true, |context, lower| {
        let selected = select(context, lower).unwrap();
        selected.visit(|fact| {
            if let SelectedFact::Terminal { payload: Some(payload), edges, .. } = fact {
                if let Opcode::Failure { .. } = payload.opcode {
                    failure = true; assert!(edges.is_empty());
                    assert_eq!(payload.describe().flow, selected::Flow::Never);
                    assert!(payload.describe().artifacts.contains(&(ArtifactId::Runtime(RuntimeService::Panic), plan::ArtifactCategory::Runtime)));
                }
            }
            Ok::<_, std::convert::Infallible>(())
        }).unwrap();
        assert!(lower.draft().blocks().all(|(_, block)| !matches!(block.terminator, Some(Terminator::NonReturningCall(_)))));
    });
    assert!(failure);
}

#[test]
fn consuming_trace_edits_preserve_frozen_record_authority_and_reject_wrong_tls_publish_value() {
    let mut rejected = false;
    complete(
        "fn identity(a: i64) -> i64 { return a; } fn main() -> i64 { return identity(7); }",
        true,
        |context, lower| {
            let selected = select(context, lower).unwrap();
            let verifier = Verifier::new(lower.receipt().owner().context().profile()).unwrap();
            let editor = selected.into_editor();
            let mut values = editor.values().collect::<Vec<_>>();
            values.reverse();
            let mut blocks = editor.blocks().collect::<Vec<_>>();
            blocks.reverse();
            let mut objects = editor.objects().collect::<Vec<_>>();
            objects.reverse();
            let (editor, _) = editor.rebuild(&values, &blocks, &objects).unwrap();
            let selected = editor.finish(&verifier).unwrap();
            let mut wrong = None;
            let mut store = None;
            selected
                .visit(|fact| {
                    if let SelectedFact::Instruction {
                        block,
                        ordinal,
                        payload,
                    } = fact
                    {
                        match &payload.opcode {
                            Opcode::TraceLoad {
                                out, record: None, ..
                            } => wrong = Some(out.value),
                            Opcode::TraceStore { record: None, .. } if store.is_none() => {
                                store = Some((block, ordinal))
                            }
                            _ => {}
                        }
                    }
                    Ok::<_, std::convert::Infallible>(())
                })
                .unwrap();
            if let (Some(value), Some((block, ordinal))) = (wrong, store) {
                let mut editor = selected.into_editor();
                editor
                    .replace_operand(
                        editor.block(block).unwrap(),
                        ordinal,
                        1,
                        editor.value(value).unwrap(),
                    )
                    .unwrap();
                match editor.finish(&verifier) {
                    Err(selected::SelectedEditFailure::Verify(failures)) => {
                        assert!(failures.iter().any(|f| matches!(
                            f.reason,
                            selected::SelectedReason::Target(
                                "malformed native trace memory sequence"
                            )
                        )))
                    }
                    _ => panic!(
                        "well-typed substitution must not publish the previous head as this frame"
                    ),
                }
                rejected = true;
            }
        },
    );
    assert!(rejected);
}

#[test]
fn independent_payload_validation_covers_nonreturning_calls_and_hard_traps() {
    let mut seen = false;
    complete(
        "fn divide(a: i64, b: i64) -> i64 { return a / b; } fn main() -> i64 { return 0; }",
        false,
        |context, lower| {
            let plan = lower.receipt().owner().context();
            let panic = ArtifactId::Runtime(RuntimeService::Panic);
            let signature = plan
                .artifact(plan.artifact_id(panic).unwrap(), panic.category())
                .unwrap()
                .signature
                .unwrap();
            let mut builder = lir::DraftBuilder::new(lower.receipt().owner()).unwrap();
            let entry = builder.reserve_block().unwrap();
            builder.define_block(entry, &[]).unwrap();
            builder.set_entry(entry).unwrap();
            let address = builder
                .append(
                    entry,
                    Operation::Constant(lir::Constant::Null(plan::ScalarType::DataAddress)),
                )
                .unwrap()[0];
            let length = builder
                .append(entry, Operation::Constant(lir::Constant::U64(0)))
                .unwrap()[0];
            let components = &plan
                .signature(plan.signature_id(signature.index()).unwrap())
                .unwrap()
                .inputs;
            builder
                .terminate(
                    entry,
                    Terminator::NonReturningCall(lir::Call {
                        target: CallTarget::Direct(panic),
                        signature,
                        arguments: vec![
                            lir::CallArgument {
                                role: components[0].role,
                                value: address,
                            },
                            lir::CallArgument {
                                role: components[1].role,
                                value: length,
                            },
                        ],
                        attribution: lir::CallAttribution::ProcessBoundary,
                    }),
                )
                .unwrap();
            let lower = lir::verify_callable(builder.finish()).unwrap();
            let selected = select(context, &lower).unwrap();
            crate::backend::x86_64_sysv::native::place_native_baseline(&selected).unwrap();
            let verifier = Verifier::new(plan.profile()).unwrap();
            selected.visit(|fact| {
            if let SelectedFact::Terminal { payload: Some(payload), edges, .. } = fact {
                assert!(matches!(&payload.opcode, Opcode::Call(call) if call.never && call.results.is_empty()));
                assert_eq!(payload.describe().flow, selected::Flow::Never); assert!(edges.is_empty());
                assert!(payload.effects.contains(crate::backend::effects::Effect::HardTrap));
                assert!(verifier.verify_payload(payload, true).is_ok()); assert!(verifier.verify_payload(payload, false).is_err());
                let trap = Instruction::new(Opcode::HardTrap, payload.origin, &payload.resources);
                assert!(verifier.verify_payload(&trap, true).is_ok());
                seen = true;
            }
            Ok::<_, std::convert::Infallible>(())
        }).unwrap();
        },
    );
    assert!(seen);
}

#[test]
fn canonical_discovery_rejects_missing_late_and_unsupported_thunk_requests() {
    let mut seen = false;
    complete(
        "extern fn foreign(a: i64) -> i64; fn main() -> i64 { return foreign(7); }",
        false,
        |context, lower| {
            let selected = select(context, lower).unwrap();
            let receipt = selected.receipt();
            let requests = super::super::requests::requests(lower).unwrap();
            super::super::requests::check_references(&requests, receipt.references()).unwrap();
            if let Some(key) = receipt.references().iter().next().copied() {
                let mut missing = requests.clone();
                missing.remove(&key);
                assert!(
                    matches!(super::super::requests::check_references(&missing, receipt.references()), Err(SelectionError::Undiscovered(actual)) if actual == key)
                );
                seen = true;
            }
            let thunk = ArtifactId::Callable(LirCallableId::TargetThunk(plan::TargetThunkKey {
                family: 0,
                specialization: 0,
            }));
            assert!(matches!(
                super::super::requests::check_references(
                    &[thunk].into_iter().collect(),
                    &[thunk].into_iter().collect()
                ),
                Err(SelectionError::Unsupported("native ABI thunk form"))
            ));
        },
    );
    assert!(seen);
}
#[test]
fn whole_program_selected_closure_reconciles_released_lower_bodies_under_both_trace_policies() {
    let source = "extern fn foreign(a: i64) -> i64; fn invoke(f: fn(i64) -> i64, a: i64) -> i64 { return f(a); } fn identity(a: i64) -> i64 { return a; } fn main() -> i64 { return foreign(invoke(identity, 7)); }";
    for trace in [false, true] {
        let fixture = lower_source_to_complete_final_mir_with_sources("closure.ska", source);
        let admitted = admit(if trace {
            BackendInput::with_runtime_trace(&fixture.mir, &fixture.sources)
        } else {
            BackendInput::without_runtime_trace(&fixture.mir)
        })
        .unwrap();
        let catalog = TargetDeclarations::new(admitted.plan().view())
            .freeze()
            .unwrap();
        let context = selection_context(&catalog).unwrap();
        let mut inventory = selected::SelectedProgramBuilder::new(&context);
        let lower = crate::backend::pilot::lower_program(&admitted, |body| {
            let selected = select(&context, &body).unwrap();
            inventory.complete(&selected, &selected.receipt()).unwrap();
            drop(body);
            drop(selected);
            Ok(())
        })
        .unwrap();
        let selected = inventory.finish(&lower).unwrap();
        assert_eq!(selected.receipts().len(), lower.receipts().len());
    }
}

#[test]
fn traced_loops_numeric_corrections_and_calls_keep_frame_state_at_all_joins() {
    complete("fn identity(a:i64)->i64 {return a;} fn main()->i64 {var n:i64=3; var sum:i64=0; while(n>0) {sum=sum+identity(n)/n; n=n-1;} return sum;}", true, |context, lower| {
        let selected = select(context, lower).unwrap();
        assert!(selected.analysis().is_ok());
    });
}
