use crate::{
    backend::{
        graph::{GraphFailure, GraphIdentity, GraphLocation},
        lir::ProgramError,
        plan::PlanError,
    },
    source::Span,
};
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum SelectedReason {
    Context(PlanError),
    Inventory(ProgramError),
    Graph(Box<GraphFailure>),
    Representation,
    Resource,
    Tie,
    Timing,
    Flow,
    Abi,
    Effect,
    Reference,
    Bundle,
    Target(&'static str),
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct SelectedFailure {
    pub identity: Box<GraphIdentity>,
    pub location: GraphLocation,
    pub reason: SelectedReason,
    pub origin: Option<Span>,
}
