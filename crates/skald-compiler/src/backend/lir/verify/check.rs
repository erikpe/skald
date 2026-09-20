//! Compose all mandatory lowered checks before consuming publication.
use super::super::*;
use super::{
    domains,
    failure::{VerificationFailure as Failure, VerificationReason as Reason},
    lift::References,
    memory, publication, trace,
};
use crate::backend::effects::{Effect, Effects, MemoryRegion};
use crate::backend::graph::{check_graph, GraphLocation, GraphView};
use crate::backend::plan::{ArtifactCategory, ArtifactId, DataKey};
use std::collections::BTreeSet;
pub(in crate::backend) fn verify_callable(
    mut draft: CallableDraft<'_>,
) -> Result<publication::VerifiedCallable<'_>, Vec<Failure>> {
    let session = check_graph(&draft)
        .map_err(|f| f.into_iter().map(Failure::from_graph).collect::<Vec<_>>())?;
    let checks = DraftChecks { draft: &draft };
    let refs = References { draft: &draft };
    let identity = draft.identity();
    let mut failures = Vec::new();
    let mut requirements = BTreeSet::new();
    let mut record = |location, reason, result: Result<(), BuildError>| {
        if let Err(error) = result {
            let reason = match error {
                BuildError::SizeOverflow
                | BuildError::Plan(crate::backend::plan::PlanError::SizeOverflow) => {
                    Reason::SizeOverflow
                }
                BuildError::Plan(crate::backend::plan::PlanError::UnsupportedCapability) => {
                    Reason::UnsupportedCapability
                }
                _ => reason,
            };
            let mut failure = Failure::new(identity, location, reason);
            failure.origin = super::failure::origin(&draft, location);
            failures.push(failure);
        }
    };
    for (value, definition) in draft.values.iter() {
        record(
            GraphLocation::Value(value.index()),
            Reason::Schema,
            checks.check_type(definition.ty),
        );
    }
    for (object, definition) in draft.objects.iter() {
        record(
            GraphLocation::Object(object.index()),
            Reason::InvalidObject,
            checks.check_object(definition),
        );
    }
    record(
        GraphLocation::Entry,
        Reason::InvalidTrace,
        trace::plan(&draft),
    );
    let facts = match memory::provenance(&draft) {
        Ok(facts) => facts,
        Err(()) => {
            return Err(vec![Failure::new(
                identity,
                GraphLocation::Entry,
                Reason::InvalidProvenance,
            )]);
        }
    };
    for (value, definition) in draft.values.iter() {
        if definition.provenance != AddressProvenance::Unknown
            && definition.provenance != facts[value.index()]
        {
            record(
                GraphLocation::Value(value.index()),
                Reason::InvalidProvenance,
                Err(BuildError::InvalidMemory),
            );
        }
    }
    let mut paths = domains::GuardPaths::new(&session);
    for (id, block) in draft.blocks.iter() {
        record(
            GraphLocation::Block(id.index()),
            Reason::InvalidTrace,
            trace::block(&draft, id, block),
        );
        for (ordinal, instruction) in block.instructions.iter().enumerate() {
            let location = GraphLocation::Instruction {
                block: id.index(),
                ordinal,
                operand: 0,
            };
            record(
                location,
                Reason::Schema,
                refs.operation(&instruction.operation)
                    .and_then(|op| checks.normalize(op))
                    .map(|_| ()),
            );
            if let Some((relation, evidence)) = domains::obligation(&instruction.operation, &draft)
            {
                record(
                    location,
                    Reason::GuardProtection,
                    if paths.evidence(&draft, id, relation, &evidence) {
                        Ok(())
                    } else {
                        Err(BuildError::InvalidEvidence)
                    },
                );
            }
            if let Operation::Load {
                address,
                representation,
            }
            | Operation::Store {
                address,
                representation,
                ..
            } = &instruction.operation
            {
                record(
                    location,
                    Reason::InvalidMemory,
                    memory::access(&draft, facts[address.index()], *representation),
                );
            }
            let required = checks
                .operation_effects_for(&instruction.operation, |value| Ok(facts[value.index()]));
            record(
                location,
                Reason::NarrowedEffects,
                required.and_then(|required| {
                    if instruction.effects.covers(&required) {
                        Ok(())
                    } else {
                        Err(BuildError::NarrowedEffects)
                    }
                }),
            );
            record(
                location,
                Reason::InvalidReference,
                effect_references(&draft, &instruction.effects, &mut requirements),
            );
            match &instruction.operation {
                Operation::SymbolAddress { symbol, .. } => {
                    requirements.insert(*symbol);
                    if let ArtifactId::Data(DataKey::Static(field)) = symbol {
                        record(
                            location,
                            Reason::InvalidReference,
                            if draft.owner.context().permits_static_reference(*field) {
                                Ok(())
                            } else {
                                Err(BuildError::InvalidMemory)
                            },
                        );
                    }
                }
                Operation::Call(call) => call_references(call, &mut requirements),
                _ => {}
            }
        }
        let location = GraphLocation::Terminator {
            block: id.index(),
            operand: 0,
        };
        let required = match block.terminator.as_ref().unwrap() {
            Terminator::ScalarCheck { relation, .. } => {
                record(
                    location,
                    Reason::Schema,
                    refs.relation(relation)
                        .and_then(|r| checks.check_relation(r))
                        .map(|_| ()),
                );
                Ok(Effects::default())
            }
            Terminator::ReportFailure { call, reason } => {
                let valid = matches!(
                    call.target,
                    CallTarget::Direct(ArtifactId::Runtime(
                        crate::backend::plan::RuntimeService::Panic
                    ))
                );
                record(
                    location,
                    Reason::Schema,
                    if valid {
                        checks.check_failure_message(call, *reason)
                    } else {
                        Err(BuildError::InvalidCall)
                    },
                );
                record(
                    location,
                    Reason::Schema,
                    refs.call(call)
                        .and_then(|c| checks.normalize_call(c, true))
                        .map(|_| ()),
                );
                call_references(call, &mut requirements);
                checks.call_effects(call)
            }
            Terminator::NonReturningCall(call) => {
                record(
                    location,
                    Reason::Schema,
                    refs.call(call)
                        .and_then(|c| checks.normalize_call(c, true))
                        .map(|_| ()),
                );
                call_references(call, &mut requirements);
                checks.call_effects(call)
            }
            Terminator::HardTrap => Ok(Effects::new([Effect::HardTrap])),
            _ => Ok(Effects::default()),
        };
        record(
            location,
            Reason::NarrowedEffects,
            required.and_then(|required| {
                if block
                    .terminal_effects
                    .as_ref()
                    .is_some_and(|e| e.covers(&required))
                {
                    Ok(())
                } else {
                    Err(BuildError::NarrowedEffects)
                }
            }),
        );
        if let Some(effects) = &block.terminal_effects {
            record(
                location,
                Reason::InvalidReference,
                effect_references(&draft, effects, &mut requirements),
            );
        }
    }
    if let Some(plan) = &draft.trace_plan {
        requirements.insert(plan.context);
        requirements.insert(ArtifactId::TraceTls);
        requirements.extend(plan.locations.iter().copied());
    }
    for artifact in &requirements {
        let view = draft.owner.context();
        record(
            GraphLocation::Entry,
            Reason::InvalidReference,
            view.artifact_id(*artifact)
                .and_then(|id| view.artifact(id, artifact.category()))
                .and_then(|_| {
                    if let ArtifactId::Callable(key) = artifact {
                        view.callable(*key).map(|_| ())
                    } else {
                        Ok(())
                    }
                })
                .map_err(BuildError::from),
        );
    }
    if !failures.is_empty() {
        failures.sort_by_key(|f| (f.location.sort_key(), f.reason));
        failures.dedup();
        return Err(failures);
    }
    // All borrowed sessions end before verified derived metadata is installed.
    let handles = draft.values.handles().collect::<Vec<_>>();
    for handle in handles {
        draft.values.get_mut(handle).unwrap().provenance = facts[handle.id().index()];
    }
    Ok(publication::publish(draft, requirements))
}
#[cfg_attr(not(test), allow(dead_code))]
fn call_references(call: &Call, references: &mut BTreeSet<ArtifactId>) {
    if let CallTarget::Direct(target) = call.target {
        references.insert(target);
    }
    match call.attribution {
        CallAttribution::SourceOperation {
            location: Some(location),
            ..
        } => {
            references.insert(location);
        }
        CallAttribution::InheritedOperation { boundary }
        | CallAttribution::SourceBodyFromOmittedHelper { boundary } => {
            references.insert(ArtifactId::Callable(boundary));
        }
        _ => {}
    }
}
#[cfg_attr(not(test), allow(dead_code))]
fn effect_references(
    draft: &CallableDraft<'_>,
    effects: &Effects<crate::backend::graph::LoweredObjectId>,
    references: &mut BTreeSet<ArtifactId>,
) -> Result<(), BuildError> {
    for effect in effects.iter() {
        match effect {
            Effect::Read(MemoryRegion::Object(object))
            | Effect::Write(MemoryRegion::Object(object)) => {
                draft.objects.get_id(*object)?;
            }
            Effect::Read(MemoryRegion::Static(field))
            | Effect::Write(MemoryRegion::Static(field)) => {
                let key = ArtifactId::Data(DataKey::Static(*field));
                let view = draft.owner.context();
                view.artifact(view.artifact_id(key)?, ArtifactCategory::Data)?;
                if !view.permits_static_reference(*field) {
                    return Err(BuildError::InvalidMemory);
                }
                references.insert(key);
            }
            _ => {}
        }
    }
    Ok(())
}
