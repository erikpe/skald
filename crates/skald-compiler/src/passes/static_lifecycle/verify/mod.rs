//! Verification boundary for planned static-lifecycle MIR.

mod authority;
mod final_coordinator;
mod lifecycle;
mod realization;

use crate::mir::{
    verify_mir, verify_preliminary_mir, MirProgram, MirProgramLifecycle, MirStaticInitializerBody,
    MirVerificationError, MirVerificationErrors,
};

use super::plan::PlannedMirProgram;

#[derive(Clone, Copy)]
pub(super) struct LifecycleMirView<'mir> {
    pub(super) program: &'mir MirProgram,
    pub(super) lifecycle: &'mir MirProgramLifecycle,
    pub(super) initializers: &'mir [MirStaticInitializerBody],
}

/// Verifies the explicit lifecycle schema and compact authority without
/// solving effects, strongly connected components, or a new lifecycle order.
pub fn verify_planned_mir(program: &PlannedMirProgram) -> Result<(), MirVerificationErrors> {
    verify_preliminary_mir(program.preliminary())?;

    let mut errors = Vec::new();
    lifecycle::verify(program, &mut errors);
    authority::verify(program, &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(MirVerificationErrors::new(errors))
    }
}

/// Re-verifies final coordinator structure, all moved bodies, and their
/// monotone realization of baseline authority using only backend-consumable
/// `MirProgram`.
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
        realization::verify(view, &mut errors);
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

pub(super) fn debug_assert_exact_synthesized_realization(program: &MirProgram) {
    #[cfg(debug_assertions)]
    {
        let coordinator = program
            .static_lifecycle
            .as_ref()
            .expect("synthesis must install its lifecycle coordinator");
        let view = LifecycleMirView {
            program,
            lifecycle: coordinator.lifecycle(),
            initializers: coordinator.initializers(),
        };
        let realized = realization::analyze(view)
            .expect("unmodified synthesis must retain every issued lifecycle root");
        debug_assert_eq!(
            realized,
            *coordinator.lifecycle().proof().authority(),
            "unmodified synthesis must exactly realize baseline authority"
        );
    }
}

#[cfg(test)]
mod realization_tests;
#[cfg(test)]
mod tests;
