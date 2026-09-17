//! Shared structural verification. Success is analysis, never phase authority.
mod analysis;
mod check;
mod model;
pub(in crate::backend) use analysis::GraphSession;
pub(in crate::backend) use check::check_graph;
pub(in crate::backend) use model::{
    BlockDescription, DefinitionSite, EdgeDescription, GraphDescription, GraphFailure,
    GraphIdentity, GraphLocation, GraphReason, GraphStage, GraphView, InstructionDescription,
    TypedUse, ValueDescription,
};
#[cfg(test)]
mod tests;
