//! Independent concrete-cell legality and semantic CFG associations.
mod fields;
mod graph;
pub(super) use fields::check as fields;
pub(super) use graph::graph;
