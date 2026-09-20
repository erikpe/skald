//! Process boundary: ABI marker, static lifecycle, and preserved language result.
use super::LowerError;
use crate::backend::{
    lir::{
        verify_callable, Call, CallAttribution, CallTarget, DraftBuilder, Operation, Terminator,
        VerifiedCallable,
    },
    plan::{ArtifactId, CallableBinding, LirCallableId, PlanError, RuntimeService},
    planning::PlannedProgram,
};

pub(super) fn lower<'plan>(
    planned: &'plan PlannedProgram<'_>,
    owner: CallableBinding<'plan>,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let plan = planned.plan().view();
    plan.require_same_context(owner.context())?;
    let mut builder = DraftBuilder::new(owner)?;
    let entry = builder.reserve_block()?;
    builder.define_block(entry, &[])?;
    builder.set_entry(entry)?;
    call(
        plan,
        &mut builder,
        entry,
        ArtifactId::Runtime(RuntimeService::AbiMarker),
    )?;
    let initializer = ArtifactId::Callable(LirCallableId::Coordinator(
        crate::backend::plan::Coordinator::Initializer,
    ));
    if plan.artifact_id(initializer).is_ok() {
        call(plan, &mut builder, entry, initializer)?;
    }
    let results = call(
        plan,
        &mut builder,
        entry,
        ArtifactId::Callable(LirCallableId::Source(
            planned.program().entry_function.into(),
        )),
    )?;
    let finalizer = ArtifactId::Callable(LirCallableId::Coordinator(
        crate::backend::plan::Coordinator::Finalizer,
    ));
    if plan.artifact_id(finalizer).is_ok() {
        call(plan, &mut builder, entry, finalizer)?;
    }
    builder.terminate(entry, Terminator::Return(results))?;
    verify_callable(builder.finish()).map_err(LowerError::Verification)
}

fn call<'plan>(
    plan: crate::backend::plan::PlanView<'plan>,
    builder: &mut DraftBuilder<'plan>,
    block: crate::backend::lir::BlockHandle<'plan>,
    target: ArtifactId,
) -> Result<Vec<crate::backend::lir::ValueHandle<'plan>>, LowerError> {
    let signature = plan
        .artifact(plan.artifact_id(target)?, target.category())?
        .signature
        .ok_or(PlanError::InvalidSignature)?;
    Ok(builder.append(
        block,
        Operation::Call(Call {
            target: CallTarget::Direct(target),
            signature,
            arguments: vec![],
            attribution: CallAttribution::ProcessBoundary,
        }),
    )?)
}
