//! Verification boundary for planned static-lifecycle MIR.

mod authority;
mod certificate;
mod final_coordinator;
mod lifecycle;

use crate::mir::{
    verify_mir, verify_preliminary_mir, MirProgram, MirProgramLifecycle, MirStaticInitializerBody,
    MirVerificationError, MirVerificationErrors, PlannedMirProgram,
};

#[derive(Clone, Copy)]
pub(super) struct LifecycleMirView<'mir> {
    pub(super) program: &'mir MirProgram,
    pub(super) lifecycle: &'mir MirProgramLifecycle,
    pub(super) initializers: &'mir [MirStaticInitializerBody],
}

/// Verifies the explicit lifecycle schema and its certificate without solving
/// effects, strongly connected components, or a new lifecycle order.
pub fn verify_planned_mir(program: &PlannedMirProgram) -> Result<(), MirVerificationErrors> {
    verify_preliminary_mir(program.preliminary())?;

    let mut errors = Vec::new();
    lifecycle::verify(program, &mut errors);
    authority::verify(program, &mut errors);
    certificate::verify(
        LifecycleMirView {
            program: program.preliminary().program(),
            lifecycle: program.lifecycle_mir(),
            initializers: program.preliminary().static_initializer_bodies(),
        },
        &mut errors,
    );
    if errors.is_empty() {
        Ok(())
    } else {
        Err(MirVerificationErrors::new(errors))
    }
}

/// Re-verifies final coordinator structure, all moved bodies, and the effect
/// certificate using only the backend-consumable `MirProgram`.
pub fn verify_synthesized_mir(program: &MirProgram) -> Result<(), MirVerificationErrors> {
    let structural = verify_mir(program);
    let structurally_valid = structural.is_ok();
    let mut errors = structural
        .err()
        .map_or_else(Vec::new, |errors| errors.iter().cloned().collect());
    let Some(coordinator) = &program.static_lifecycle else {
        return if errors.is_empty() {
            Ok(())
        } else {
            Err(MirVerificationErrors::new(errors))
        };
    };
    let view = LifecycleMirView {
        program,
        lifecycle: coordinator.lifecycle(),
        initializers: coordinator.initializers(),
    };
    final_coordinator::verify(view, coordinator, &mut errors);
    if structurally_valid {
        certificate::verify(view, &mut errors);
    }
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
