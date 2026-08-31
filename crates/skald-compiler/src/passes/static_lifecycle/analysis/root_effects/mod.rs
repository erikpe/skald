//! Checker-oriented normalized analysis effects reachable from lifecycle roots.

mod closure;
mod model;

#[cfg(test)]
pub(crate) use closure::analyze;
pub(crate) use closure::{analyze_final, analyze_for_fields};
pub(crate) use model::{dependency_pairs_for_definitions, StaticLifecycleRootEffectError};

#[cfg(test)]
use model::dependency_pairs;

#[cfg(test)]
mod tests;
