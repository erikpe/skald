use super::{
    context::{definition, Lowerer},
    LowerError, PendingFeature,
};
use crate::backend::pilot::AdmittedPilot;
use crate::backend::{
    lir::{ProgramBuilder, VerifiedCallable},
    plan::{LirCallableId, PlanError},
    RuntimeTracePolicy,
};
use crate::mir::{MirInstruction, MirRvalueKind, MirTerminator};

/// Construct and verify one declared source body, then register its exact receipt.
/// Later features reject before beginning work; no temporary trap or fake body.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn lower_next<'plan>(
    admitted: &'plan AdmittedPilot<'_>,
    worklist: &mut ProgramBuilder<'plan>,
) -> Result<Option<VerifiedCallable<'plan>>, LowerError> {
    let Some(key) = worklist.next() else {
        return Ok(None);
    };
    preflight(admitted, key)?;
    let owner = worklist.begin(key)?;
    let body = Lowerer::new(admitted, owner)?.finish()?;
    worklist.complete(&body, &body.receipt())?;
    Ok(Some(body))
}

fn preflight(admitted: &AdmittedPilot<'_>, callable: LirCallableId) -> Result<(), LowerError> {
    let pending = |feature| LowerError::Pending { callable, feature };
    let LirCallableId::Source(source) = callable else {
        return Err(pending(PendingFeature::Entry));
    };
    if admitted.plan().view().runtime_trace() == RuntimeTracePolicy::Enabled {
        return Err(pending(PendingFeature::RuntimeTrace));
    }
    let definition = definition(admitted, source)?;
    for block in &definition.body().blocks {
        for instruction in &block.instructions {
            match instruction {
                MirInstruction::Assign(assign) => match assign.rvalue.kind {
                    MirRvalueKind::IntegerDivision { .. }
                    | MirRvalueKind::Shift { .. }
                    | MirRvalueKind::PrimitiveCast { .. }
                    | MirRvalueKind::CheckedF64ToInteger { .. } => {
                        return Err(pending(PendingFeature::GuardedNumeric))
                    }
                    _ => {}
                },
                MirInstruction::Call(_) => return Err(pending(PendingFeature::Calls)),
                _ => {}
            }
        }
        match block
            .terminator
            .as_ref()
            .expect("verified final MIR terminator")
        {
            MirTerminator::Return { .. }
            | MirTerminator::Goto { .. }
            | MirTerminator::Branch { .. } => {}
            MirTerminator::ShiftCountCheck { .. }
            | MirTerminator::IntegerDivisorCheck { .. }
            | MirTerminator::PrimitiveCastRangeCheck { .. }
            | MirTerminator::Terminate { .. } => {
                return Err(pending(PendingFeature::GuardedNumeric))
            }
            _ => return Err(PlanError::InvalidDomain.into()),
        }
    }
    Ok(())
}
