//! Local trace ownership and immediate operation associations; recipes own native parity.
use super::super::*;
use crate::backend::plan::ArtifactId;
pub(super) fn plan(draft: &CallableDraft<'_>) -> Result<(), BuildError> {
    let Some(plan) = &draft.trace_plan else {
        return if draft
            .objects
            .iter()
            .any(|(_, object)| object.role == ObjectRole::TraceRecord)
        {
            Err(BuildError::InvalidTrace)
        } else {
            Ok(())
        };
    };
    DraftChecks { draft }.check_trace_plan(plan)?;
    for (object, record) in draft.objects.iter() {
        if record.role == ObjectRole::TraceRecord && Some(object) != plan.record {
            return Err(BuildError::InvalidTrace);
        }
    }
    Ok(())
}
fn location(call: &Call) -> Option<ArtifactId> {
    if let CallAttribution::SourceOperation { location, .. } = call.attribution {
        location
    } else {
        None
    }
}
pub(super) fn block(
    draft: &CallableDraft<'_>,
    id: crate::backend::graph::LoweredBlockId,
    block: &Block,
) -> Result<(), BuildError> {
    if draft
        .trace_plan
        .as_ref()
        .is_some_and(|plan| plan.frame_eligible)
    {
        if Some(id) == draft.entry
            && !matches!(
                block.instructions.first().map(|i| &i.operation),
                Some(Operation::Trace(TraceAction::PushFrame { .. }))
            )
        {
            return Err(BuildError::InvalidTrace);
        }
        if matches!(block.terminator, Some(Terminator::Return(_)))
            && !matches!(
                block.instructions.last().map(|i| &i.operation),
                Some(Operation::Trace(TraceAction::PopFrame { .. }))
            )
        {
            return Err(BuildError::InvalidTrace);
        }
    }
    for (ordinal, instruction) in block.instructions.iter().enumerate() {
        match &instruction.operation {
            Operation::Trace(TraceAction::ReplaceLocation {
                location: update,
                site,
                ..
            }) => {
                let target = match site {
                    TraceSite::Instruction {
                        block: target,
                        ordinal: next,
                    } if *target == id && *next == ordinal + 1 => {
                        block.instructions.get(*next).and_then(|i| {
                            if let Operation::Call(c) = &i.operation {
                                Some(c)
                            } else {
                                None
                            }
                        })
                    }
                    TraceSite::Terminator(target)
                        if *target == id && ordinal + 1 == block.instructions.len() =>
                    {
                        match block.terminator.as_ref() {
                            Some(
                                Terminator::ReportFailure { call, .. }
                                | Terminator::NonReturningCall(call),
                            ) => Some(call),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if target.and_then(location) != Some(*update) {
                    return Err(BuildError::InvalidTrace);
                }
            }
            Operation::Call(call) if location(call).is_some() => {
                association(id, block, ordinal, location(call).unwrap())?;
            }
            _ => {}
        }
    }
    if let Some(Terminator::ReportFailure { call, .. } | Terminator::NonReturningCall(call)) =
        &block.terminator
    {
        if let Some(location) = location(call) {
            association(id, block, block.instructions.len(), location)?;
        }
    }
    // Push/pop local consistency; cross-block frame/path parity is a native recipe obligation.
    let mut pushed = false;
    let mut popped = false;
    for instruction in &block.instructions {
        match instruction.operation {
            Operation::Trace(TraceAction::PushFrame { .. }) => {
                if pushed || popped || id != draft.entry.unwrap() {
                    return Err(BuildError::InvalidTrace);
                }
                pushed = true;
            }
            Operation::Trace(TraceAction::PopFrame { .. }) => {
                if popped || !matches!(block.terminator, Some(Terminator::Return(_))) {
                    return Err(BuildError::InvalidTrace);
                }
                popped = true;
            }
            Operation::Trace(TraceAction::ReplaceLocation { .. }) if popped => {
                return Err(BuildError::InvalidTrace)
            }
            _ => {}
        }
    }
    Ok(())
}
fn association(
    id: crate::backend::graph::LoweredBlockId,
    block: &Block,
    ordinal: usize,
    location: ArtifactId,
) -> Result<(), BuildError> {
    let Some(previous) = ordinal
        .checked_sub(1)
        .and_then(|p| block.instructions.get(p))
    else {
        return Err(BuildError::InvalidTrace);
    };
    let expected = if ordinal == block.instructions.len() {
        TraceSite::Terminator(id)
    } else {
        TraceSite::Instruction { block: id, ordinal }
    };
    if !matches!(&previous.operation,Operation::Trace(TraceAction::ReplaceLocation{location:actual,site,..})if *actual==location&&*site==expected)
    {
        return Err(BuildError::InvalidTrace);
    }
    Ok(())
}
