//! Representation/checker regressions and a separate independent specification oracle.
use super::requirements::Requirements;
use super::*;

mod baseline;
mod checking;
mod equivalence;
mod fixtures;
mod frame;
mod oracle;
mod representation;

pub(in crate::backend) use oracle::check_native_resource_events;

pub(super) fn check_with_round_oracle<'s, 'p, P: crate::backend::selected::Payload>(
    draft: PlacementDraft<'s, 'p, P>,
    target: &impl PlacementTarget,
) -> Result<CheckedPlacement<'s, 'p, P>, CheckFailure> {
    if draft.validate_structure().is_err() {
        return check_placement(draft, target);
    }
    let mut requirements = match Requirements::collect(&draft, target) {
        Ok(requirements) => requirements,
        Err(_) => return check_placement(draft, target),
    };
    if requirements.validate(&draft, target).is_err() || requirements.seed(&draft).is_err() {
        return check_placement(draft, target);
    }
    let production = requirements.observe_solver(&draft);
    let oracle = oracle::observe_round_solver(&requirements, &draft);
    assert_eq!(production, oracle, "production/round-oracle divergence");
    check_placement(draft, target)
}
