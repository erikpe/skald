//! Checker-oriented normalized effects reachable from lifecycle roots.

mod closure;
mod model;

pub(crate) use closure::{analyze, project_solved_analysis};
pub(crate) use model::StaticLifecycleRootEffectAnalysis;

#[cfg(test)]
mod tests;
