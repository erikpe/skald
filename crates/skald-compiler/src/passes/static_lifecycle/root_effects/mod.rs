//! Checker-oriented normalized effects reachable from lifecycle roots.

mod closure;
mod model;

pub(crate) use closure::{analyze, analyze_final, project_solved_analysis};
pub(crate) use model::{
    dependency_pairs, dependency_pairs_for_definitions, StaticLifecycleRootEffectError,
};

#[cfg(test)]
mod tests;
