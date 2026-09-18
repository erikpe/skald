use super::{context::Lowerer, LowerError};
use crate::backend::pilot::AdmittedPilot;
use crate::backend::{
    lir::{ProgramBuilder, VerifiedCallable},
    plan::LirCallableId,
};

/// Construct one declared body and register its exact verified receipt.
pub(in crate::backend) fn lower_next<'plan>(
    admitted: &'plan AdmittedPilot<'_>,
    worklist: &mut ProgramBuilder<'plan>,
) -> Result<Option<VerifiedCallable<'plan>>, LowerError> {
    let Some(key) = worklist.next() else {
        return Ok(None);
    };
    let owner = worklist.begin(key)?;
    let body = match key {
        LirCallableId::Source(_) => Lowerer::new(admitted, owner)?.finish()?,
        LirCallableId::Entry => super::entry::lower(admitted, owner)?,
        _ => return Err(crate::backend::plan::PlanError::InvalidDomain.into()),
    };
    worklist.complete(&body, &body.receipt())?;
    Ok(Some(body))
}

/// Stream verified bodies to their consumer; retain only exact completion receipts.
/// This witness closes shared lowering, not native emission or executable authority.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn lower_program<'plan>(
    admitted: &'plan AdmittedPilot<'_>,
    mut consume: impl FnMut(VerifiedCallable<'plan>) -> Result<(), LowerError>,
) -> Result<crate::backend::lir::VerifiedProgram<'plan>, LowerError> {
    let mut worklist = ProgramBuilder::new(admitted.plan().view());
    while let Some(body) = lower_next(admitted, &mut worklist)? {
        consume(body)?;
    }
    super::data::define(admitted, &mut worklist)?;
    Ok(worklist.finish()?)
}
