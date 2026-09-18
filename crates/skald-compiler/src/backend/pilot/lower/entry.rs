//! Process boundary: runtime ABI marker, language entry, preserved scalar result.
use super::LowerError;
use crate::backend::{
    lir::{
        verify_callable, Call, CallAttribution, CallTarget, DraftBuilder, Operation, Terminator,
        VerifiedCallable,
    },
    pilot::AdmittedPilot,
    plan::{ArtifactId, CallableBinding, LirCallableId, PlanError, RuntimeService},
};

pub(super) fn lower<'plan>(
    admitted: &'plan AdmittedPilot<'_>,
    owner: CallableBinding<'plan>,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let plan = admitted.plan().view();
    plan.require_same_context(owner.context())?;
    let mut builder = DraftBuilder::new(owner)?;
    let entry = builder.reserve_block()?;
    builder.define_block(entry, &[])?;
    builder.set_entry(entry)?;
    // Admission excludes statics; neither lifecycle coordinator is required.
    for target in [
        ArtifactId::Runtime(RuntimeService::AbiMarker),
        ArtifactId::Callable(LirCallableId::Source(
            admitted.program().entry_function.into(),
        )),
    ] {
        let signature = plan
            .artifact(plan.artifact_id(target)?, target.category())?
            .signature
            .ok_or(PlanError::InvalidSignature)?;
        let results = builder.append(
            entry,
            Operation::Call(Call {
                target: CallTarget::Direct(target),
                signature,
                arguments: vec![],
                attribution: CallAttribution::ProcessBoundary,
            }),
        )?;
        if matches!(target, ArtifactId::Callable(_)) {
            builder.terminate(entry, Terminator::Return(results))?;
        }
    }
    verify_callable(builder.finish()).map_err(LowerError::Verification)
}
