//! Independent issuance of the semantic static-activation authority.

use crate::mir::{MirVerificationError, StaticActivationAuthority};

use super::{
    super::{activation::analyze_static_activation, plan::PlannedMirProgram},
    program_error,
};

/// Re-solves activation from verified preliminary MIR and binds the exact
/// field set into the verified phase product. The planning report is compared
/// only as an untrusted claimed result; its witnesses and summaries do not
/// participate in authority issuance.
pub(super) fn verify(
    program: &PlannedMirProgram,
    errors: &mut Vec<MirVerificationError>,
) -> Option<StaticActivationAuthority> {
    let recomputed = match analyze_static_activation(program.preliminary()) {
        Ok(analysis) => analysis,
        Err(error) => {
            program_error(
                errors,
                format!("cannot independently recompute static activation: {error}"),
            );
            return None;
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

    if expected != reported {
        let missing = expected
            .fields()
            .iter()
            .copied()
            .filter(|field| !reported.contains(*field))
            .collect::<Vec<_>>();
        let extra = reported
            .fields()
            .iter()
            .copied()
            .filter(|field| !expected.contains(*field))
            .collect::<Vec<_>>();
        program_error(
            errors,
            format!(
                "planning report shadow activation disagrees with independent preliminary-MIR activation; missing {missing:?}; extra {extra:?}"
            ),
        );
    }

    Some(expected)
}
