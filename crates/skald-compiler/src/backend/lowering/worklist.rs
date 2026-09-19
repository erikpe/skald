use super::{context::Lowerer, LowerError};
use crate::backend::planning::AdmittedProgram;
use crate::backend::{
    lir::{ProgramBuilder, VerifiedCallable},
    plan::LirCallableId,
};

/// Construct one declared body and register its exact verified receipt.
pub(in crate::backend) fn lower_next<'plan>(
    admitted: &'plan AdmittedProgram<'_>,
    worklist: &mut ProgramBuilder<'plan>,
) -> Result<Option<VerifiedCallable<'plan>>, LowerError> {
    let Some(key) = worklist.next() else {
        return Ok(None);
    };
    let owner = worklist.begin(key)?;
    let body = match key {
        LirCallableId::Source(_) => Lowerer::new(admitted, owner)?.finish()?,
        LirCallableId::Entry => super::entry::lower(admitted, owner)?,
        LirCallableId::Helper(key)
            if key.family == crate::backend::plan::HelperFamily::ClassFinalizer =>
        {
            super::generated::lower_class_finalizer(admitted, owner, key.layout)?
        }
        LirCallableId::Helper(key)
            if key.family == crate::backend::plan::HelperFamily::OptionalBoxFinalizer =>
        {
            super::generated::lower_optional_box_finalizer(admitted, owner, key.layout)?
        }
        LirCallableId::Helper(key) if key.family == crate::backend::plan::HelperFamily::Retain => {
            super::generated_ownership::lower_retain(admitted, owner)?
        }
        LirCallableId::Helper(key) if key.family == crate::backend::plan::HelperFamily::Release => {
            super::generated_ownership::lower_release(admitted, owner)?
        }
        _ => return Err(crate::backend::plan::PlanError::InvalidDomain.into()),
    };
    worklist.complete(&body, &body.receipt())?;
    Ok(Some(body))
}

/// Stream verified bodies to their consumer; retain only exact completion receipts.
/// This witness closes shared lowering, not native emission or executable authority.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn lower_program<'plan>(
    admitted: &'plan AdmittedProgram<'_>,
    mut consume: impl FnMut(VerifiedCallable<'plan>) -> Result<(), LowerError>,
) -> Result<crate::backend::lir::VerifiedProgram<'plan>, LowerError> {
    lower_program_with(admitted, &mut consume)
}

/// Stream a complete lowered program into a fallible downstream pipeline.
///
/// The generic error keeps shared lowering independent of any target phase while
/// preserving the rule that consumer failure cannot publish lower-program closure.
pub(in crate::backend) fn lower_program_with<'plan, E>(
    admitted: &'plan AdmittedProgram<'_>,
    mut consume: impl FnMut(VerifiedCallable<'plan>) -> Result<(), E>,
) -> Result<crate::backend::lir::VerifiedProgram<'plan>, E>
where
    E: From<LowerError>,
{
    let mut worklist = ProgramBuilder::new(admitted.plan().view());
    while let Some(body) = lower_next(admitted, &mut worklist).map_err(E::from)? {
        consume(body)?;
    }
    super::data::define(admitted, &mut worklist).map_err(E::from)?;
    worklist.finish().map_err(LowerError::from).map_err(E::from)
}
