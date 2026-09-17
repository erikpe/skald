use super::calls::runtime_call;
use super::*;
use crate::backend::effects::Effect;
use crate::backend::failure::FailureMessage;
use crate::backend::plan::{
    test_fixtures::runtime_declarations, ArtifactDeclaration, ArtifactId, DataKey, HelperFamily,
    HelperKey, LirCallableId, PlanFacts, RuntimeService,
};
use crate::backend::RuntimeTracePolicy;
use crate::source::{SourceDatabase, Span};

pub(super) fn trace_catalog(facts: &mut PlanFacts) -> (ArtifactId, ArtifactId) {
    facts.runtime_trace = RuntimeTracePolicy::Enabled;
    let layout = facts
        .add_layout(LayoutFact {
            size: 32,
            alignment: 8,
            disposition: LayoutDisposition::Addressable,
        })
        .unwrap();
    let context = ArtifactId::Data(DataKey::TraceContext(0));
    let location = ArtifactId::Data(DataKey::TraceLocation(0));
    for key in [context, location, ArtifactId::TraceTls] {
        facts.artifacts.push(ArtifactDeclaration {
            key,
            signature: None,
            layout: Some(layout),
        });
    }
    (context, location)
}
fn origin() -> Span {
    let mut sources = SourceDatabase::new();
    Span::empty(sources.add("call.ska", ""), 0)
}

#[test]
fn enabled_trace_actions_remain_ordered_and_associated_with_explicit_call_locations() {
    let mut supplied = facts();
    let (context, location) = trace_catalog(&mut supplied);
    let signature = supplied.callables[0].signature;
    let plan = CheckedPlan::check(supplied).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    let record = builder
        .declare_object(object(
            LayoutDisposition::Addressable,
            32,
            8,
            ObjectRole::TraceRecord,
            LifetimeDisposition::WholeCallable,
        ))
        .unwrap();
    builder
        .declare_trace_plan(TracePlan {
            frame_eligible: true,
            record: Some(record),
            context,
            locations: vec![location],
        })
        .unwrap();
    builder
        .append(entry, Operation::Trace(TraceAction::PushFrame { record }))
        .unwrap();
    builder
        .append(
            entry,
            Operation::Trace(TraceAction::ReplaceLocation {
                record,
                location,
                site: TraceSite::Instruction {
                    block: entry,
                    ordinal: 2,
                },
            }),
        )
        .unwrap();
    builder
        .append(
            entry,
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
    builder
        .append(entry, Operation::Trace(TraceAction::PopFrame { record }))
        .unwrap();
    builder
        .terminate(entry, Terminator::Return(vec![]))
        .unwrap();
    let draft = builder.finish();
    assert_eq!(draft.trace_plan().unwrap().record, Some(record.id()));
    let instructions = &draft.block(entry).unwrap().instructions;
    assert_eq!(instructions.len(), 4);
    assert!(matches!(
        instructions[0].operation,
        Operation::Trace(TraceAction::PushFrame { .. })
    ));
    assert!(matches!(
        instructions[1].operation,
        Operation::Trace(TraceAction::ReplaceLocation { .. })
    ));
    assert!(matches!(
        instructions[3].operation,
        Operation::Trace(TraceAction::PopFrame { .. })
    ));
    assert!(instructions[1].effects.contains(Effect::TraceState));
    assert_eq!(instructions[0].results.len(), 0);
    assert!(draft
        .block(entry)
        .unwrap()
        .terminal_effects
        .as_ref()
        .unwrap()
        .iter()
        .next()
        .is_none());
}

#[test]
fn trace_policy_and_local_plan_reject_omitted_foreign_records_unknown_locations_and_ineligible_frames(
) {
    let omitted = CheckedPlan::check(facts()).unwrap();
    let mut local = builder(&omitted);
    let entry = entry_block(&mut local);
    let object = local
        .declare_object(object(
            LayoutDisposition::Addressable,
            8,
            8,
            ObjectRole::SemanticStorage,
            LifetimeDisposition::WholeCallable,
        ))
        .unwrap();
    assert_eq!(
        err(local.append(
            entry,
            Operation::Trace(TraceAction::PushFrame { record: object })
        )),
        BuildError::Plan(PlanError::OmittedTrace)
    );
    assert_eq!(
        err(local.declare_trace_plan(TracePlan {
            frame_eligible: false,
            record: None,
            context: ArtifactId::Data(DataKey::TraceContext(0)),
            locations: vec![]
        })),
        BuildError::Plan(PlanError::OmittedTrace)
    );
    let mut supplied = facts();
    let (context, location) = trace_catalog(&mut supplied);
    let first = CheckedPlan::check(supplied.clone()).unwrap();
    let equal = CheckedPlan::check(supplied).unwrap();
    let mut first_builder = builder(&first);
    let entry = entry_block(&mut first_builder);
    let record = first_builder
        .declare_object(super::object(
            LayoutDisposition::Addressable,
            32,
            8,
            ObjectRole::TraceRecord,
            LifetimeDisposition::WholeCallable,
        ))
        .unwrap();
    let mut other = builder(&equal);
    let foreign = other
        .declare_object(super::object(
            LayoutDisposition::Addressable,
            32,
            8,
            ObjectRole::TraceRecord,
            LifetimeDisposition::WholeCallable,
        ))
        .unwrap();
    assert_eq!(
        err(first_builder.declare_trace_plan(TracePlan {
            frame_eligible: true,
            record: Some(foreign),
            context,
            locations: vec![location]
        })),
        BuildError::Plan(PlanError::WrongContext)
    );
    assert_eq!(
        err(first_builder.declare_trace_plan(TracePlan {
            frame_eligible: true,
            record: Some(record),
            context,
            locations: vec![location, location]
        })),
        BuildError::InvalidTrace
    );
    first_builder
        .declare_trace_plan(TracePlan {
            frame_eligible: false,
            record: None,
            context,
            locations: vec![location],
        })
        .unwrap();
    assert_eq!(
        err(first_builder.append(entry, Operation::Trace(TraceAction::PushFrame { record }))),
        BuildError::InvalidTrace
    );
    assert_eq!(
        err(first_builder.append(
            entry,
            Operation::Trace(TraceAction::ReplaceLocation {
                record,
                location: ArtifactId::Data(DataKey::TraceLocation(42)),
                site: TraceSite::Terminator(entry)
            })
        )),
        BuildError::InvalidTrace
    );
}

#[test]
fn six_attribution_meanings_preserve_effect_contracts_and_omitted_source_metadata() {
    let mut supplied = facts();
    let signatures = runtime_declarations(&mut supplied);
    let signature = supplied.callables[0].signature;
    let layout = supplied
        .add_layout(LayoutFact {
            size: 8,
            alignment: 8,
            disposition: LayoutDisposition::Addressable,
        })
        .unwrap();
    let helper = LirCallableId::Helper(HelperKey {
        family: HelperFamily::Release,
        layout,
        signature,
    });
    supplied
        .callables
        .push(crate::backend::plan::CallableDeclaration {
            key: helper,
            signature,
            body: crate::backend::plan::BodyDisposition::Required,
        });
    let plan = CheckedPlan::check(supplied).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    for attribution in [
        CallAttribution::SourceOperation {
            origin: origin(),
            location: None,
        },
        CallAttribution::InheritedOperation { boundary: helper },
        CallAttribution::SourceBodyFromOmittedHelper { boundary: helper },
        CallAttribution::ProcessBoundary,
    ] {
        builder
            .append(
                entry,
                Operation::Call(Call {
                    target: CallTarget::Direct(ArtifactId::Callable(source(1))),
                    signature,
                    arguments: vec![],
                    attribution,
                }),
            )
            .unwrap();
    }
    let pointer = constant(&mut builder, entry, Constant::Null(ScalarType::DataAddress));
    for attribution in [
        CallAttribution::NonReporting,
        CallAttribution::HardDefectOnly,
    ] {
        builder
            .append(
                entry,
                Operation::Call(runtime_call(
                    RuntimeService::Free,
                    signatures[&RuntimeService::Free],
                    &[pointer],
                    attribution,
                )),
            )
            .unwrap();
    }
    let size = constant(&mut builder, entry, Constant::U64(8));
    assert_eq!(
        err(builder.append(
            entry,
            Operation::Call(runtime_call(
                RuntimeService::Allocate,
                signatures[&RuntimeService::Allocate],
                &[size],
                CallAttribution::NonReporting
            ))
        )),
        BuildError::InvalidCall
    );
    assert_eq!(
        err(builder.append(
            entry,
            Operation::Call(Call {
                target: CallTarget::Direct(ArtifactId::Callable(source(1))),
                signature,
                arguments: vec![],
                attribution: CallAttribution::SourceOperation {
                    origin: origin(),
                    location: Some(ArtifactId::Data(DataKey::TraceLocation(0)))
                }
            })
        )),
        BuildError::Plan(PlanError::OmittedTrace)
    );
    assert!(builder.finish().trace_plan().is_none());
}

#[test]
fn reported_failure_and_nonreturning_service_calls_are_explicit_terminals_without_cleanup_edges() {
    let mut supplied = facts();
    let signatures = runtime_declarations(&mut supplied);
    let reason = FailureMessage::PrimitiveCastOutOfRange;
    let message_key = ArtifactId::Data(DataKey::FailureMessage(reason));
    let layout = supplied
        .add_layout(LayoutFact {
            size: reason.bytes().len(),
            alignment: 1,
            disposition: LayoutDisposition::Addressable,
        })
        .unwrap();
    supplied.artifacts.push(ArtifactDeclaration {
        key: message_key,
        signature: None,
        layout: Some(layout),
    });
    let plan = CheckedPlan::check(supplied).unwrap();
    for reported in [true, false] {
        let mut builder = builder(&plan);
        let entry = entry_block(&mut builder);
        let message = builder
            .append(
                entry,
                Operation::SymbolAddress {
                    symbol: message_key,
                    ty: ScalarType::DataAddress,
                },
            )
            .unwrap()[0];
        let length = constant(
            &mut builder,
            entry,
            Constant::U64(reason.bytes().len() as u64),
        );
        let call = runtime_call(
            RuntimeService::Panic,
            signatures[&RuntimeService::Panic],
            &[message, length],
            CallAttribution::ProcessBoundary,
        );
        assert_eq!(
            err(builder.append(entry, Operation::Call(call.clone()))),
            BuildError::InvalidCall
        );
        if reported {
            assert_eq!(
                err(builder.terminate(
                    entry,
                    Terminator::ReportFailure {
                        call: call.clone(),
                        reason: FailureMessage::IntegerDivisionByZero
                    }
                )),
                BuildError::InvalidCall
            );
            let mut wrong_length = call.clone();
            wrong_length.arguments[1].value = constant(&mut builder, entry, Constant::U64(0));
            assert_eq!(
                err(builder.terminate(
                    entry,
                    Terminator::ReportFailure {
                        call: wrong_length,
                        reason
                    }
                )),
                BuildError::InvalidCall
            );
        }
        let terminal = if reported {
            Terminator::ReportFailure {
                call,
                reason: FailureMessage::PrimitiveCastOutOfRange,
            }
        } else {
            Terminator::NonReturningCall(call)
        };
        builder.terminate(entry, terminal).unwrap();
        let draft = builder.finish();
        let block = draft.block(entry).unwrap();
        assert_eq!(
            block.terminator.as_ref().unwrap().edges(entry.id()).count(),
            0
        );
        assert!(block
            .terminal_effects
            .as_ref()
            .unwrap()
            .contains(Effect::Report));
    }
    assert_eq!(
        FailureMessage::PrimitiveCastOutOfRange.bytes(),
        b"floating-point cast out of range"
    );
    assert_eq!(
        FailureMessage::IntegerDivisionByZero.bytes(),
        b"integer division by zero"
    );
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    builder.terminate(entry, Terminator::HardTrap).unwrap();
    let draft = builder.finish();
    let effects = draft
        .block(entry)
        .unwrap()
        .terminal_effects
        .as_ref()
        .unwrap();
    assert!(effects.contains(Effect::HardTrap));
    assert!(!effects.contains(Effect::Report));
}
