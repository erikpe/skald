use super::*;
use crate::backend::effects::{Effect, Effects};
use crate::backend::graph::GraphLocation;
use crate::backend::plan::{ArtifactDeclaration, ArtifactId, DataKey};
use crate::backend::{RuntimeTracePolicy, Target};
fn reasons(draft: CallableDraft<'_>) -> Vec<VerificationReason> {
    verify_callable(draft)
        .err()
        .expect("invalid publication")
        .iter()
        .map(|e| e.reason)
        .collect()
}
fn finish<'p>(mut b: DraftBuilder<'p>, entry: BlockHandle<'p>) -> CallableDraft<'p> {
    b.terminate(entry, Terminator::Return(vec![])).unwrap();
    b.finish()
}
#[test]
fn publication_and_receipts_identify_exact_snapshots_and_live_contexts() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let equal = CheckedPlan::check(facts()).unwrap();
    fn publish(plan: &CheckedPlan) -> VerifiedCallable<'_> {
        let mut b = builder(plan);
        let e = entry_block(&mut b);
        verify_callable(finish(b, e)).unwrap()
    }
    let first: VerifiedCallable<'_> = publish(&plan);
    let receipt: CompletionReceipt<'_> = first.receipt();
    let second = publish(&plan);
    let foreign = publish(&equal);
    assert!(receipt.matches(&first));
    assert!(receipt.same_snapshot(&first.receipt()));
    assert!(!receipt.matches(&second));
    assert!(!receipt.matches(&foreign));
    assert_eq!(receipt.owner().key(), source(0));
    assert!(receipt.references().is_empty());
    assert_eq!(first.draft().blocks().len(), 1);
    drop(first);
    assert!(!receipt.same_snapshot(&second.receipt()));
}
fn constant_division<'p>(plan: &'p CheckedPlan, divisor: i64) -> CallableDraft<'p> {
    let mut b = builder(plan);
    let e = entry_block(&mut b);
    let dividend = constant(&mut b, e, Constant::I64(i64::MIN));
    let divisor = constant(&mut b, e, Constant::I64(divisor));
    b.append(
        e,
        Operation::Divide {
            result: DivisionResult::Quotient,
            dividend,
            divisor,
            evidence: ScalarDomainEvidence::ExactConstant(divisor),
        },
    )
    .unwrap();
    finish(b, e)
}
#[test]
fn exact_divisor_constants_are_checked_without_signed_overflow_arithmetic() {
    let plan = CheckedPlan::check(facts()).unwrap();
    assert!(verify_callable(constant_division(&plan, -1)).is_ok());
    assert!(reasons(constant_division(&plan, 0)).contains(&VerificationReason::GuardProtection));
}
#[test]
fn shift_constant_widths_and_exact_operand_identity_are_verified() {
    let plan = CheckedPlan::check(facts()).unwrap();
    for (width, good, bad) in [(8, 7, 8), (64, 63, 64)] {
        for (count, valid) in [(good, true), (bad, false)] {
            let mut b = builder(&plan);
            let e = entry_block(&mut b);
            let v = constant(
                &mut b,
                e,
                if width == 8 {
                    Constant::U8(1)
                } else {
                    Constant::U64(1)
                },
            );
            let c = constant(&mut b, e, Constant::U64(count));
            b.append(
                e,
                Operation::Shift {
                    direction: ShiftDirection::Left,
                    value: v,
                    count: c,
                    evidence: ScalarDomainEvidence::ExactConstant(c),
                },
            )
            .unwrap();
            assert_eq!(verify_callable(finish(b, e)).is_ok(), valid);
        }
    }
    let mut b = builder(&plan);
    let e = entry_block(&mut b);
    let v = constant(&mut b, e, Constant::U8(1));
    let c = constant(&mut b, e, Constant::U64(0));
    let other = constant(&mut b, e, Constant::U64(0));
    b.append(
        e,
        Operation::Shift {
            direction: ShiftDirection::Left,
            value: v,
            count: c,
            evidence: ScalarDomainEvidence::ExactConstant(other),
        },
    )
    .unwrap();
    assert!(reasons(finish(b, e)).contains(&VerificationReason::GuardProtection));
}
#[test]
fn float_range_evidence_uses_mathematical_truncation_and_exclusive_power_of_two_bounds() {
    let plan = CheckedPlan::check(facts()).unwrap();
    for (target, value, valid) in [
        (ScalarType::U8, -0.99, true),
        (ScalarType::U8, 255.99, true),
        (ScalarType::U8, -1.0, false),
        (ScalarType::U8, 256.0, false),
        (ScalarType::I64, -9223372036854775808.0, true),
        (ScalarType::I64, 9223372036854775808.0, false),
        (ScalarType::U64, 18446744073709549568.0, true),
        (ScalarType::U64, 18446744073709551616.0, false),
        (ScalarType::U64, -0.5, true),
        (ScalarType::I64, f64::NAN, false),
        (ScalarType::U8, f64::INFINITY, false),
        (ScalarType::I64, f64::NEG_INFINITY, false),
    ] {
        let mut b = builder(&plan);
        let e = entry_block(&mut b);
        let v = constant(&mut b, e, Constant::F64(value.to_bits()));
        b.append(
            e,
            Operation::Convert {
                conversion: Conversion::TruncateFloat,
                value: v,
                target,
                evidence: Some(ScalarDomainEvidence::ExactConstant(v)),
            },
        )
        .unwrap();
        assert_eq!(
            verify_callable(finish(b, e)).is_ok(),
            valid,
            "{target:?} {value:?}"
        );
    }
}
pub(super) fn guarded<'p>(
    plan: &'p CheckedPlan,
) -> (
    CallableDraft<'p>,
    BlockHandle<'p>,
    BlockHandle<'p>,
    BlockHandle<'p>,
) {
    let mut b = builder(plan);
    let e = entry_block(&mut b);
    let success = block(&mut b);
    let failure = block(&mut b);
    let input = b.inputs().next().unwrap();
    let dividend = constant(&mut b, e, Constant::I64(4));
    b.terminate(
        e,
        Terminator::ScalarCheck {
            relation: ScalarCheck::NonZeroDivisor {
                ty: ScalarType::I64,
                divisor: input,
            },
            success: empty_edge(success),
            failure: empty_edge(failure),
        },
    )
    .unwrap();
    b.append(
        success,
        Operation::Divide {
            result: DivisionResult::Remainder,
            dividend,
            divisor: input,
            evidence: ScalarDomainEvidence::SuccessCheck(e),
        },
    )
    .unwrap();
    b.terminate(success, Terminator::Return(vec![])).unwrap();
    b.terminate(failure, Terminator::HardTrap).unwrap();
    (b.finish(), e, success, failure)
}
pub(super) fn input_plan() -> CheckedPlan {
    let mut f = facts();
    for n in 0..2 {
        f.signatures[0].inputs.push(Component {
            ty: ScalarType::I64,
            role: ComponentRole::Parameter(n),
        });
    }
    CheckedPlan::check(f).unwrap()
}
#[test]
fn success_edge_protection_rejects_failure_bypass_and_secured_operand_substitution() {
    let plan = input_plan();
    let (draft, _, _, _) = guarded(&plan);
    assert!(verify_callable(draft).is_ok());
    let (mut draft, _, success, failure) = guarded(&plan);
    draft.blocks.get_mut(failure).unwrap().terminator = Some(Terminator::Jump(Edge {
        target: success.id(),
        arguments: vec![],
    }));
    assert!(reasons(draft).contains(&VerificationReason::GuardProtection));
    let (mut draft, _, success, _) = guarded(&plan);
    let other = draft.inputs[1];
    if let Operation::Divide { divisor, .. } =
        &mut draft.blocks.get_mut(success).unwrap().instructions[0].operation
    {
        *divisor = other;
    }
    assert!(reasons(draft).contains(&VerificationReason::GuardProtection));
}
#[test]
fn check_rooted_dead_regions_are_checked_and_disconnected_checks_are_rejected() {
    let plan = input_plan();
    for (failure_bypass, expected) in [(false, true), (true, false)] {
        let (mut draft, check, success, failure) = guarded(&plan);
        let new = draft
            .blocks
            .push(Block {
                parameters: Some(vec![]),
                terminator: Some(Terminator::Return(vec![])),
                terminal_effects: Some(Effects::default()),
                ..Block::default()
            })
            .unwrap();
        draft.entry = Some(new.id());
        // Entry inputs remain global; move the dividend into the dead check's scope
        // only where local order is independently valid.
        if failure_bypass {
            draft.blocks.get_mut(failure).unwrap().terminator = Some(Terminator::Jump(Edge {
                target: success.id(),
                arguments: vec![],
            }));
        }
        assert_eq!(
            verify_callable(draft).is_ok(),
            expected,
            "dead check {}",
            check.id().index()
        );
    }
    let (mut draft, check, _success, _) = guarded(&plan);
    if let Some(Terminator::ScalarCheck { success: edge, .. }) =
        &mut draft.blocks.get_mut(check).unwrap().terminator
    {
        edge.target = check.id();
    }
    // Entry predecessors are independently rejected before guard publication.
    assert!(reasons(draft).contains(&VerificationReason::Graph));
    let (mut draft, check, success, _) = guarded(&plan);
    let other = draft
        .blocks
        .push(Block {
            parameters: Some(vec![]),
            terminator: Some(Terminator::HardTrap),
            terminal_effects: Some(Effects::new([Effect::HardTrap])),
            ..Block::default()
        })
        .unwrap();
    if let Some(Terminator::ScalarCheck { success: edge, .. }) =
        &mut draft.blocks.get_mut(check).unwrap().terminator
    {
        edge.target = other.id();
    }
    assert!(reasons(draft).contains(&VerificationReason::GuardProtection));
    let _ = success;
}
#[test]
fn widths_relations_and_scalar_cast_cells_are_independently_checked() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut b = builder(&plan);
    let e = entry_block(&mut b);
    let v = constant(&mut b, e, Constant::U8(1));
    let c = constant(&mut b, e, Constant::U64(7));
    let s = block(&mut b);
    let f = block(&mut b);
    b.terminate(
        e,
        Terminator::ScalarCheck {
            relation: ScalarCheck::ShiftCountBelowWidth {
                count: c,
                width: 64,
            },
            success: empty_edge(s),
            failure: empty_edge(f),
        },
    )
    .unwrap();
    b.append(
        s,
        Operation::Shift {
            direction: ShiftDirection::Left,
            value: v,
            count: c,
            evidence: ScalarDomainEvidence::SuccessCheck(e),
        },
    )
    .unwrap();
    b.terminate(s, Terminator::Return(vec![])).unwrap();
    b.terminate(f, Terminator::HardTrap).unwrap();
    assert!(reasons(b.finish()).contains(&VerificationReason::GuardProtection));
    let mut b = builder(&plan);
    let e = entry_block(&mut b);
    let v = constant(&mut b, e, Constant::U8(1));
    let r = b
        .append(
            e,
            Operation::Convert {
                conversion: Conversion::ToBoolean,
                value: v,
                target: ScalarType::Bool,
                evidence: None,
            },
        )
        .unwrap()[0];
    let mut draft = finish(b, e);
    if let Operation::Convert { conversion, .. } =
        &mut draft.blocks.get_mut(e).unwrap().instructions[1].operation
    {
        *conversion = Conversion::FloatBits;
    }
    assert!(reasons(draft).contains(&VerificationReason::Schema));
    let _ = r;
}
#[test]
fn object_access_extent_alignment_width_and_forged_provenance_are_rejected() {
    let plan = CheckedPlan::check(facts()).unwrap();
    for (case, reason) in [
        (0, VerificationReason::InvalidMemory),
        (1, VerificationReason::InvalidMemory),
        (2, VerificationReason::InvalidProvenance),
        (3, VerificationReason::Schema),
    ] {
        let mut b = builder(&plan);
        let e = entry_block(&mut b);
        let o = b
            .declare_object(object(
                LayoutDisposition::Addressable,
                8,
                8,
                ObjectRole::SemanticStorage,
                LifetimeDisposition::WholeCallable,
            ))
            .unwrap();
        let a = b.append(e, Operation::ObjectAddress(o)).unwrap()[0];
        let offset = constant(
            &mut b,
            e,
            Constant::U64(if case == 0 {
                8
            } else if case == 1 {
                1
            } else {
                0
            }),
        );
        let p = b
            .append(e, Operation::ByteOffset { base: a, offset })
            .unwrap()[0];
        b.append(
            e,
            Operation::Load {
                address: p,
                representation: MemoryRepresentation {
                    scalar: ScalarType::U64,
                    bytes: 8,
                    alignment: 8,
                },
            },
        )
        .unwrap();
        let mut draft = finish(b, e);
        if case == 2 {
            draft.values.get_mut(p).unwrap().provenance = AddressProvenance::Object {
                object: o.id(),
                offset: 4,
            };
        }
        if case == 3 {
            if let Operation::Load { representation, .. } =
                &mut draft.blocks.get_mut(e).unwrap().instructions[3].operation
            {
                representation.bytes = 1;
            }
        }
        assert!(reasons(draft).contains(&reason));
    }
}
#[test]
fn mandatory_effects_are_recomputed_and_terminal_summaries_cannot_disappear() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut b = builder(&plan);
    let e = entry_block(&mut b);
    let o = b
        .declare_object(object(
            LayoutDisposition::Addressable,
            8,
            8,
            ObjectRole::SemanticStorage,
            LifetimeDisposition::WholeCallable,
        ))
        .unwrap();
    let p = b.append(e, Operation::ObjectAddress(o)).unwrap()[0];
    b.append(
        e,
        Operation::Load {
            address: p,
            representation: MemoryRepresentation {
                scalar: ScalarType::U64,
                bytes: 8,
                alignment: 8,
            },
        },
    )
    .unwrap();
    let mut draft = finish(b, e);
    draft.blocks.get_mut(e).unwrap().instructions[1].effects = Effects::default();
    assert!(reasons(draft).contains(&VerificationReason::NarrowedEffects));
    let mut b = builder(&plan);
    let e = entry_block(&mut b);
    b.terminate(e, Terminator::HardTrap).unwrap();
    let mut draft = b.finish();
    draft.blocks.get_mut(e).unwrap().terminal_effects = None;
    assert!(reasons(draft).contains(&VerificationReason::NarrowedEffects));
}
#[test]
fn static_domain_is_checked_in_complete_mode_and_receipts_collect_actual_references() {
    use crate::identity::{ClassId, StaticFieldId};
    let mut f = facts();
    let field = StaticFieldId::new(ClassId::new(0), 0);
    let layout = f
        .add_layout(LayoutFact {
            size: 8,
            alignment: 8,
            disposition: LayoutDisposition::Addressable,
        })
        .unwrap();
    let key = ArtifactId::Data(DataKey::Static(field));
    f.artifacts.push(ArtifactDeclaration {
        key,
        signature: None,
        layout: Some(layout),
    });
    f.active_statics.insert(field);
    let plan = CheckedPlan::check(f.clone()).unwrap();
    fn build(plan: &CheckedPlan, key: ArtifactId) -> CallableDraft<'_> {
        let mut b = builder(plan);
        let e = entry_block(&mut b);
        b.append(
            e,
            Operation::SymbolAddress {
                symbol: key,
                ty: ScalarType::DataAddress,
            },
        )
        .unwrap();
        finish(b, e)
    }
    let body = verify_callable(build(&plan, key)).unwrap();
    assert!(body.receipt().references().contains(&key));
    f.active_statics.clear();
    let inactive = CheckedPlan::check(f).unwrap();
    assert!(reasons(build(&inactive, key)).contains(&VerificationReason::InvalidReference));
}
#[test]
fn omitted_trace_records_and_enabled_orphan_records_are_not_published() {
    let mut f = facts();
    f.runtime_trace = RuntimeTracePolicy::Omitted;
    let plan = CheckedPlan::check(f).unwrap();
    let mut b = builder(&plan);
    let e = entry_block(&mut b);
    let o = b
        .declare_object(object(
            LayoutDisposition::Addressable,
            8,
            8,
            ObjectRole::SemanticStorage,
            LifetimeDisposition::WholeCallable,
        ))
        .unwrap();
    let mut draft = finish(b, e);
    draft.objects.get_mut(o).unwrap().role = ObjectRole::TraceRecord;
    assert!(reasons(draft).contains(&VerificationReason::InvalidObject));

    let mut f = facts();
    super::tracing::trace_catalog(&mut f);
    let enabled = CheckedPlan::check(f).unwrap();
    let mut b = builder(&enabled);
    let e = entry_block(&mut b);
    b.declare_object(object(
        LayoutDisposition::Addressable,
        32,
        8,
        ObjectRole::TraceRecord,
        LifetimeDisposition::WholeCallable,
    ))
    .unwrap();
    assert!(reasons(finish(b, e)).contains(&VerificationReason::InvalidTrace));
}
#[test]
fn invariant_conversion_preserves_source_identity_and_does_not_fabricate_generated_identity() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let errors: Vec<VerificationFailure> =
        verify_callable(constant_division(&plan, 0)).err().unwrap();
    let error = errors
        .iter()
        .find(|f| f.reason == VerificationReason::GuardProtection)
        .unwrap();
    assert_eq!(
        error.location,
        GraphLocation::Instruction {
            block: 0,
            ordinal: 2,
            operand: 0
        }
    );
    assert_eq!(
        error.backend_error(Target::X86_64SysV).callable(),
        Some(match source(0) {
            crate::backend::plan::LirCallableId::Source(c) => c,
            _ => unreachable!(),
        })
    );
    let mut generated = error.clone();
    generated.identity.callable = crate::backend::plan::LirCallableId::Coordinator(
        crate::backend::plan::Coordinator::Initializer,
    );
    let converted = generated.backend_error(Target::X86_64SysV);
    assert_eq!(converted.callable(), None);
    assert!(converted.message().contains("Coordinator"));
    assert_eq!(converted.target(), Target::X86_64SysV);
}

#[test]
fn entry_bypass_and_an_alias_reload_cannot_reuse_a_secured_divisor_check() {
    let plan = input_plan();
    let (mut draft, check, success, _) = guarded(&plan);
    if let Operation::Divide { dividend, .. } =
        &mut draft.blocks.get_mut(success).unwrap().instructions[0].operation
    {
        *dividend = draft.inputs[1];
    }
    let new = draft
        .blocks
        .push(Block {
            parameters: Some(vec![]),
            terminal_effects: Some(Effects::default()),
            ..Block::default()
        })
        .unwrap();
    let condition = draft
        .values
        .push(Value {
            ty: ScalarType::Bool,
            definition: Some(Definition::InstructionResult {
                instruction: InstructionLocation {
                    block: new.id(),
                    ordinal: 0,
                },
                ordinal: 0,
            }),
            origin: None,
            provenance: AddressProvenance::Unknown,
        })
        .unwrap();
    let added_block = draft.blocks.get_mut(new).unwrap();
    added_block.instructions.push(Instruction {
        operation: Operation::Constant(Constant::Bool(true)),
        results: vec![condition.id()],
        effects: Effects::default(),
    });
    added_block.terminator = Some(Terminator::Branch {
        condition: condition.id(),
        true_edge: Edge {
            target: check.id(),
            arguments: vec![],
        },
        false_edge: Edge {
            target: success.id(),
            arguments: vec![],
        },
    });
    draft.entry = Some(new.id());
    assert!(reasons(draft).contains(&VerificationReason::GuardProtection));
    let mut f = facts();
    f.signatures[0].inputs = vec![
        Component {
            ty: ScalarType::I64,
            role: ComponentRole::Parameter(0),
        },
        Component {
            ty: ScalarType::DataAddress,
            role: ComponentRole::Parameter(1),
        },
    ];
    let plan = CheckedPlan::check(f).unwrap();
    let mut b = builder(&plan);
    let e = entry_block(&mut b);
    let s = block(&mut b);
    let failure = block(&mut b);
    let inputs = b.inputs().collect::<Vec<_>>();
    b.terminate(
        e,
        Terminator::ScalarCheck {
            relation: ScalarCheck::NonZeroDivisor {
                ty: ScalarType::I64,
                divisor: inputs[0],
            },
            success: empty_edge(s),
            failure: empty_edge(failure),
        },
    )
    .unwrap();
    let reload = b
        .append(
            s,
            Operation::Load {
                address: inputs[1],
                representation: MemoryRepresentation {
                    scalar: ScalarType::I64,
                    bytes: 8,
                    alignment: 8,
                },
            },
        )
        .unwrap()[0];
    b.append(
        s,
        Operation::Divide {
            result: DivisionResult::Quotient,
            dividend: inputs[0],
            divisor: reload,
            evidence: ScalarDomainEvidence::SuccessCheck(e),
        },
    )
    .unwrap();
    b.terminate(s, Terminator::Return(vec![])).unwrap();
    b.terminate(failure, Terminator::HardTrap).unwrap();
    assert!(reasons(b.finish()).contains(&VerificationReason::GuardProtection));
}

#[test]
fn float_check_target_must_match_the_guarded_conversion() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut b = builder(&plan);
    let e = entry_block(&mut b);
    let s = block(&mut b);
    let f = block(&mut b);
    let value = constant(&mut b, e, Constant::F64(255.5f64.to_bits()));
    b.terminate(
        e,
        Terminator::ScalarCheck {
            relation: ScalarCheck::FiniteTruncatedF64InIntegerRange {
                source: value,
                target: ScalarType::I64,
            },
            success: empty_edge(s),
            failure: empty_edge(f),
        },
    )
    .unwrap();
    b.append(
        s,
        Operation::Convert {
            conversion: Conversion::TruncateFloat,
            value,
            target: ScalarType::U8,
            evidence: Some(ScalarDomainEvidence::SuccessCheck(e)),
        },
    )
    .unwrap();
    b.terminate(s, Terminator::Return(vec![])).unwrap();
    b.terminate(f, Terminator::HardTrap).unwrap();
    assert!(reasons(b.finish()).contains(&VerificationReason::GuardProtection));
}

#[test]
fn publication_rechecks_direct_target_signatures_and_reporting_service_effects() {
    use crate::backend::plan::{test_fixtures::runtime_declarations, RuntimeService};
    let mut f = facts();
    let services = runtime_declarations(&mut f);
    let plan = CheckedPlan::check(f).unwrap();
    let mut b = builder(&plan);
    let e = entry_block(&mut b);
    let n = constant(&mut b, e, Constant::U64(8));
    b.append(
        e,
        Operation::Call(Call {
            target: CallTarget::Direct(ArtifactId::Runtime(RuntimeService::Allocate)),
            signature: services[&RuntimeService::Allocate],
            arguments: vec![CallArgument {
                role: ComponentRole::RuntimeParameter(0),
                value: n,
            }],
            attribution: CallAttribution::ProcessBoundary,
        }),
    )
    .unwrap();
    let mut draft = finish(b, e);
    draft.blocks.get_mut(e).unwrap().instructions[1].effects = Effects::new([Effect::Call]);
    assert!(reasons(draft).contains(&VerificationReason::NarrowedEffects));
    let mut b = builder(&plan);
    let e = entry_block(&mut b);
    b.append(
        e,
        Operation::Call(Call {
            target: CallTarget::Direct(ArtifactId::Runtime(RuntimeService::AbiMarker)),
            signature: services[&RuntimeService::AbiMarker],
            arguments: vec![],
            attribution: CallAttribution::NonReporting,
        }),
    )
    .unwrap();
    let mut draft = finish(b, e);
    if let Operation::Call(call) = &mut draft.blocks.get_mut(e).unwrap().instructions[0].operation {
        call.target = CallTarget::Direct(ArtifactId::Runtime(RuntimeService::Free));
    }
    assert!(reasons(draft).contains(&VerificationReason::Schema));
}

#[test]
fn malformed_trace_sites_and_missing_entry_push_or_return_pop_are_rejected() {
    use super::tracing::{origin, trace_catalog};
    let mut f = facts();
    let (context, location) = trace_catalog(&mut f);
    let signature = f.callables[0].signature;
    let plan = CheckedPlan::check(f).unwrap();
    for case in 0..4 {
        let mut b = builder(&plan);
        let e = entry_block(&mut b);
        let record = b
            .declare_object(object(
                LayoutDisposition::Addressable,
                32,
                8,
                ObjectRole::TraceRecord,
                LifetimeDisposition::WholeCallable,
            ))
            .unwrap();
        b.declare_trace_plan(TracePlan {
            frame_eligible: true,
            record: Some(record),
            context,
            locations: vec![location],
        })
        .unwrap();
        b.append(e, Operation::Trace(TraceAction::PushFrame { record }))
            .unwrap();
        b.append(
            e,
            Operation::Trace(TraceAction::ReplaceLocation {
                record,
                location,
                site: TraceSite::Instruction {
                    block: e,
                    ordinal: 2,
                },
            }),
        )
        .unwrap();
        b.append(
            e,
            Operation::Call(Call {
                target: CallTarget::Direct(ArtifactId::Callable(source(1))),
                signature,
                arguments: vec![],
                attribution: CallAttribution::SourceOperation {
                    origin: origin(),
                    location: Some(location),
                },
            }),
        )
        .unwrap();
        b.append(e, Operation::Trace(TraceAction::PopFrame { record }))
            .unwrap();
        let mut draft = finish(b, e);
        match case {
            0 => {
                if let Operation::Trace(TraceAction::ReplaceLocation { site, .. }) =
                    &mut draft.blocks.get_mut(e).unwrap().instructions[1].operation
                {
                    *site = TraceSite::Instruction {
                        block: e.id(),
                        ordinal: 3,
                    };
                }
            }
            1 => {
                draft.blocks.get_mut(e).unwrap().instructions[0].operation = Operation::Lifetime {
                    marker: LifetimeMarker::Start,
                    object: record.id(),
                    site: 0,
                }
            }
            2 => {
                draft.blocks.get_mut(e).unwrap().instructions[3].operation = Operation::Lifetime {
                    marker: LifetimeMarker::End,
                    object: record.id(),
                    site: 0,
                }
            }
            3 => draft.trace_plan.as_mut().unwrap().locations.push(location),
            _ => unreachable!(),
        }
        assert!(reasons(draft).contains(&VerificationReason::InvalidTrace));
    }
}

#[test]
fn capability_rejection_and_value_origins_remain_structured() {
    let mut f = facts();
    f.profile.capabilities.binary64 = false;
    let plan = CheckedPlan::check(f).unwrap();
    let mut b = builder(&plan);
    let e = entry_block(&mut b);
    let v = constant(&mut b, e, Constant::I64(0));
    let mut draft = finish(b, e);
    draft.values.get_mut(v).unwrap().ty = ScalarType::F64;
    let span = super::tracing::origin();
    draft.values.get_mut(v).unwrap().origin = Some(span);
    draft.blocks.get_mut(e).unwrap().instructions[0].operation =
        Operation::Constant(Constant::F64(0));
    let errors = verify_callable(draft).err().unwrap();
    let first = errors
        .iter()
        .find(|f| f.reason == VerificationReason::UnsupportedCapability)
        .unwrap();
    assert_eq!(first.origin, Some(span));
}

#[test]
fn absent_source_addresses_do_not_acquire_execution_authority() {
    use crate::backend::plan::BodyDisposition;
    let mut f = facts();
    let signature = f.callables[1].signature;
    f.callables[1].body = BodyDisposition::Absent;
    let crate::backend::plan::LirCallableId::Source(absent) = source(1) else {
        unreachable!()
    };
    f.executable_sources.remove(&absent);
    let plan = CheckedPlan::check(f).unwrap();
    let mut b = builder(&plan);
    let e = entry_block(&mut b);
    b.append(
        e,
        Operation::SymbolAddress {
            symbol: ArtifactId::Callable(source(1)),
            ty: ScalarType::CodeAddress(signature),
        },
    )
    .unwrap();
    assert!(reasons(finish(b, e)).contains(&VerificationReason::InvalidReference));
}

#[test]
fn forward_address_definitions_are_recomputed_and_unknown_summaries_remain_conservative() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut b = builder(&plan);
    let e = entry_block(&mut b);
    let consumer = block(&mut b);
    let producer = block(&mut b);
    let o = b
        .declare_object(object(
            LayoutDisposition::Addressable,
            8,
            8,
            ObjectRole::SemanticStorage,
            LifetimeDisposition::WholeCallable,
        ))
        .unwrap();
    let address = b.reserve_value(ScalarType::DataAddress, None).unwrap();
    b.terminate(e, Terminator::Jump(empty_edge(producer)))
        .unwrap();
    b.append(
        consumer,
        Operation::Load {
            address,
            representation: MemoryRepresentation {
                scalar: ScalarType::U64,
                bytes: 8,
                alignment: 8,
            },
        },
    )
    .unwrap();
    b.terminate(consumer, Terminator::Return(vec![])).unwrap();
    b.append_into(producer, Operation::ObjectAddress(o), &[address])
        .unwrap();
    b.terminate(producer, Terminator::Jump(empty_edge(consumer)))
        .unwrap();
    let body = verify_callable(b.finish()).unwrap();
    assert_eq!(
        body.draft().value(address).unwrap().provenance,
        AddressProvenance::Object {
            object: o.id(),
            offset: 0
        }
    );
    assert!(body.draft().block(consumer).unwrap().instructions[0]
        .effects
        .contains(Effect::Read(crate::backend::effects::MemoryRegion::Unknown)));
}

#[test]
fn overflowing_object_layout_is_a_size_rejection_with_an_object_location() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut b = builder(&plan);
    let e = entry_block(&mut b);
    let object = b
        .declare_object(object(
            LayoutDisposition::Addressable,
            8,
            8,
            ObjectRole::SemanticStorage,
            LifetimeDisposition::WholeCallable,
        ))
        .unwrap();
    let mut draft = finish(b, e);
    draft.objects.get_mut(object).unwrap().layout.size = usize::MAX;
    let failures = verify_callable(draft).err().unwrap();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].reason, VerificationReason::SizeOverflow);
    assert_eq!(
        failures[0].location,
        GraphLocation::Object(object.id().index())
    );
    assert!(failures[0]
        .backend_error(crate::backend::Target::X86_64SysV)
        .message()
        .contains("size overflow"));
}
