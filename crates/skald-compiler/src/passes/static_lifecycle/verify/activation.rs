//! Independent issuance of the semantic static-activation authority.

use crate::mir::{MirVerificationError, StaticActivationAuthority};

use super::{
    super::{activation::analyze_static_activation, plan::PlannedMirProgram},
    program_error,
};

/// Re-solves activation from verified preliminary MIR and checks both the
/// compact certificate and the source-rich report against that exact result.
/// Report witnesses and summaries never participate in authority issuance.
pub(super) fn verify(program: &PlannedMirProgram, errors: &mut Vec<MirVerificationError>) {
    let recomputed = match analyze_static_activation(program.preliminary()) {
        Ok(analysis) => analysis,
        Err(error) => {
            program_error(
                errors,
                format!("cannot independently recompute static activation: {error}"),
            );
            return;
        }
    };
    let expected = StaticActivationAuthority::new(
        recomputed
            .active_fields()
            .iter()
            .map(|active| active.field())
            .collect(),
    );
    let reported = StaticActivationAuthority::new(
        program
            .planning_report()
            .activation()
            .active_fields()
            .iter()
            .map(|active| active.field())
            .collect(),
    );

    if &expected != program.activation_authority() {
        report_difference(
            errors,
            "static activation certificate",
            &expected,
            program.activation_authority(),
        );
    }

    if expected != reported {
        report_difference(errors, "planning report activation", &expected, &reported);
    }
}

fn report_difference(
    errors: &mut Vec<MirVerificationError>,
    subject: &str,
    expected: &StaticActivationAuthority,
    actual: &StaticActivationAuthority,
) {
    let missing = expected
        .fields()
        .iter()
        .copied()
        .filter(|field| !actual.contains(*field))
        .collect::<Vec<_>>();
    let extra = actual
        .fields()
        .iter()
        .copied()
        .filter(|field| !expected.contains(*field))
        .collect::<Vec<_>>();
    program_error(
        errors,
        format!(
            "{subject} disagrees with independent preliminary-MIR activation; missing {missing:?}; extra {extra:?}"
        ),
    );
}
