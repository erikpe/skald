//! Deterministic policy for selecting target-independent final-MIR passes.
//!
//! This module owns compiler-known identity and scheduling. It deliberately
//! owns no MIR transformation, request parsing, reporting, or file I/O.

mod descriptor;
mod error;
mod identity;
mod profile;
mod registry;
mod schedule;

pub(crate) use error::MirPassScheduleError;
pub use identity::MirPassIdentity;
pub use profile::MirOptimizationProfile;
pub(crate) use schedule::{MirPassOccurrence, MirPassSchedule};

#[cfg(test)]
pub(in crate::passes::pipeline) use descriptor::{
    MirPassDescriptor, MirPassImplementation, MirPassRegistration,
};

use registry::production_registry;

/// Resolves one supported profile and a set of stable-name exclusions.
pub(crate) fn resolve_mir_pass_schedule<'a>(
    profile: MirOptimizationProfile,
    disabled_names: impl IntoIterator<Item = &'a str>,
) -> Result<MirPassSchedule, MirPassScheduleError> {
    schedule::resolve_profile(production_registry(), profile, disabled_names)
}

/// Resolves an exact compiler-internal pass order.
///
/// The driver does not expose this surface. It exists for focused pass tests,
/// composition checks, and compiler-owned tools.
pub(crate) fn resolve_exact_mir_pass_schedule(
    identities: &[MirPassIdentity],
) -> Result<MirPassSchedule, MirPassScheduleError> {
    schedule::resolve_exact(production_registry(), identities)
}

/// Returns every registered stable pass name in lexical order.
pub(crate) fn registered_mir_pass_names() -> Vec<&'static str> {
    production_registry().known_names()
}

#[cfg(test)]
pub(in crate::passes::pipeline) fn resolve_test_mir_pass_schedule(
    registrations: &'static [MirPassRegistration],
    identities: &[MirPassIdentity],
) -> Result<MirPassSchedule, MirPassScheduleError> {
    schedule::resolve_exact(registry::MirPassRegistry::new(registrations), identities)
}

#[cfg(test)]
mod tests;
