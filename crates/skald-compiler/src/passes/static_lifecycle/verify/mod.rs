//! Verification boundary for planned static-lifecycle MIR.

mod certificate;
mod lifecycle;

use crate::mir::{
    verify_preliminary_mir, MirVerificationError, MirVerificationErrors, PlannedMirProgram,
};

/// Verifies the explicit lifecycle schema and its certificate without solving
/// effects, strongly connected components, or a new lifecycle order.
pub fn verify_planned_mir(program: &PlannedMirProgram) -> Result<(), MirVerificationErrors> {
    verify_preliminary_mir(program.preliminary())?;

    let mut errors = Vec::new();
    lifecycle::verify(program, &mut errors);
    certificate::verify(program, &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(MirVerificationErrors::new(errors))
    }
}

pub(super) fn program_error(errors: &mut Vec<MirVerificationError>, message: impl Into<String>) {
    errors.push(MirVerificationError {
        callable: None,
        block: None,
        message: message.into(),
    });
}

#[cfg(test)]
mod tests;
