//! Verification boundary for planned static-lifecycle MIR.

mod activation;
mod authority;
mod final_coordinator;
mod lifecycle;
mod realization;

use crate::mir::{
    check_normalized_mir, check_preliminary_mir, verify_mir, MirProgram, MirProgramLifecycle,
    MirStaticInitializerBody, MirVerificationError, MirVerificationErrors,
};

use super::plan::PlannedMirProgram;

/// Planned lifecycle MIR whose exact authority issuance has been verified.
///
/// The private representation prevents callers from asserting verification by
/// construction. Synthesis consumes this product and cannot accept draft
/// `PlannedMirProgram` values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPlannedMirProgram {
    program: PlannedMirProgram,
}

impl VerifiedPlannedMirProgram {
    pub fn program(&self) -> &PlannedMirProgram {
        &self.program
    }

    pub(super) fn into_program(self) -> PlannedMirProgram {
        self.program
    }
}

#[derive(Clone, Copy)]
pub(super) struct LifecycleMirView<'mir> {
    pub(super) program: &'mir MirProgram,
    pub(super) lifecycle: &'mir MirProgramLifecycle,
    pub(super) initializers: &'mir [MirStaticInitializerBody],
}

/// Verifies the explicit lifecycle schema and compact authority without
/// solving effects, strongly connected components, or a new lifecycle order.
pub fn verify_planned_mir(
    program: PlannedMirProgram,
) -> Result<VerifiedPlannedMirProgram, MirVerificationErrors> {
    check_preliminary_mir(program.preliminary())?;

    let mut errors = Vec::new();
    activation::verify(&program, &mut errors);
    lifecycle::verify(&program, &mut errors);
    authority::verify(&program, &mut errors);
    if errors.is_empty() {
        Ok(VerifiedPlannedMirProgram { program })
    } else {
        Err(MirVerificationErrors::new(errors))
    }
}

/// Re-verifies final coordinator structure, all moved bodies, and their
/// monotone realization of baseline authority using only backend-consumable
/// `MirProgram`.
pub fn verify_synthesized_mir(program: &MirProgram) -> Result<(), MirVerificationErrors> {
    verify_synthesized_mir_with(program, verify_mir(program))
}

/// Re-verifies lifecycle realization over executable MIR whose consumable
/// path and logical proof has already been normalized away.
pub(crate) fn verify_normalized_synthesized_mir(
    program: &MirProgram,
) -> Result<(), MirVerificationErrors> {
    verify_synthesized_mir_with(program, check_normalized_mir(program))
}

fn verify_synthesized_mir_with(
    program: &MirProgram,
    structural: Result<(), MirVerificationErrors>,
) -> Result<(), MirVerificationErrors> {
    let structurally_valid = structural.is_ok();
    let mut errors = structural
        .err()
        .map_or_else(Vec::new, |errors| errors.iter().cloned().collect());
    let Some(coordinator) = &program.static_lifecycle else {
        if program
            .classes
            .iter()
            .any(|class| !class.static_fields.is_empty())
        {
            program_error(
                &mut errors,
                "final MIR with static declarations has no lifecycle coordinator or activation authority",
            );
        }
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
    let _ = program;
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
