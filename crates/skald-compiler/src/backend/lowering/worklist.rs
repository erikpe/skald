use super::{context::Lowerer, LowerError};
use crate::backend::planning::PlannedProgram;
use crate::backend::{
    lir::{ProgramBuilder, VerifiedCallable},
    plan::LirCallableId,
};

/// Construct one declared body and register its exact verified receipt.
pub(in crate::backend) fn lower_next<'plan>(
    planned: &'plan PlannedProgram<'_>,
    worklist: &mut ProgramBuilder<'plan>,
) -> Result<Option<VerifiedCallable<'plan>>, LowerError> {
    let Some(key) = worklist.next() else {
        return Ok(None);
    };
    let owner = worklist.begin(key)?;
    let body = match key {
        LirCallableId::Source(_) => Lowerer::new(planned, owner)?.finish()?,
        LirCallableId::Entry => super::entry::lower(planned, owner)?,
        LirCallableId::Coordinator(coordinator) => {
            super::static_lifecycle::lower(planned, owner, coordinator)?
        }
        LirCallableId::Helper(key)
            if key.family == crate::backend::plan::HelperFamily::ClassFinalizer =>
        {
            super::generated::lower_class_finalizer(planned, owner, key.layout)?
        }
        LirCallableId::Helper(key)
            if key.family == crate::backend::plan::HelperFamily::OptionalBoxFinalizer =>
        {
            super::generated::lower_optional_box_finalizer(planned, owner, key.layout)?
        }
        LirCallableId::Helper(key)
            if matches!(
                key.family,
                crate::backend::plan::HelperFamily::ArrayElementInitializer
                    | crate::backend::plan::HelperFamily::ArrayElementCopier
                    | crate::backend::plan::HelperFamily::ArrayClone
                    | crate::backend::plan::HelperFamily::ArraySliceClone
                    | crate::backend::plan::HelperFamily::ArrayPrimitiveSliceAssign
                    | crate::backend::plan::HelperFamily::ArrayElementDestroyer
                    | crate::backend::plan::HelperFamily::ArrayRelease
                    | crate::backend::plan::HelperFamily::ArraySharedFinalizer
                    | crate::backend::plan::HelperFamily::RawClassCopy
            ) =>
        {
            super::generated_array::lower(planned, owner, key)?
        }
        LirCallableId::Helper(key) if key.family == crate::backend::plan::HelperFamily::Retain => {
            super::generated_ownership::lower_retain(planned, owner)?
        }
        LirCallableId::Helper(key) if key.family == crate::backend::plan::HelperFamily::Release => {
            super::generated_ownership::lower_release(planned, owner)?
        }
        _ => return Err(crate::backend::plan::PlanError::InvalidDomain.into()),
    };
    worklist.complete(&body, &body.receipt())?;
    Ok(Some(body))
}

/// Stream verified bodies to their consumer; retain only exact completion receipts.
/// This witness closes shared lowering, not native emission or executable authority.
#[cfg(test)]
pub(in crate::backend) fn lower_program<'plan>(
    planned: &'plan PlannedProgram<'_>,
    mut consume: impl FnMut(VerifiedCallable<'plan>) -> Result<(), LowerError>,
) -> Result<crate::backend::lir::VerifiedProgram<'plan>, LowerError> {
    lower_program_with(planned, &mut consume)
}

/// Stream a complete lowered program into a fallible downstream pipeline.
///
/// The generic error keeps shared lowering independent of any target phase while
/// preserving the rule that consumer failure cannot publish lower-program closure.
pub(in crate::backend) fn lower_program_with<'plan, E>(
    planned: &'plan PlannedProgram<'_>,
    mut consume: impl FnMut(VerifiedCallable<'plan>) -> Result<(), E>,
) -> Result<crate::backend::lir::VerifiedProgram<'plan>, E>
where
    E: From<LowerError>,
{
    let mut worklist = ProgramBuilder::new(planned.plan().view());
    while let Some(body) = lower_next(planned, &mut worklist).map_err(E::from)? {
        consume(body)?;
    }
    super::data::define(planned, &mut worklist).map_err(E::from)?;
    // Typed data edges can discover generated finalizers after source bodies
    // have been released. Close those edges through the same ordinary worklist.
    while let Some(body) = lower_next(planned, &mut worklist).map_err(E::from)? {
        consume(body)?;
    }
    worklist.finish().map_err(LowerError::from).map_err(E::from)
}
