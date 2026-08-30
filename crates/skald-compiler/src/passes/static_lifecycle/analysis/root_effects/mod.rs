//! Checker-oriented normalized analysis effects reachable from lifecycle roots.

mod closure;
mod model;

pub(crate) use closure::{analyze, analyze_final};
pub(crate) use model::{dependency_pairs_for_definitions, StaticLifecycleRootEffectError};

#[cfg(test)]
use model::dependency_pairs;

#[cfg(test)]
mod tests;
