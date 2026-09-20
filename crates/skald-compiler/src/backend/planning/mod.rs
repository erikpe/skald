//! Whole-program checked planning and immutable final-MIR fact projection.
//!
//! Final-MIR queries stop at this boundary. Lowering consumes the planned
//! program and its checked plan without consulting frontend state.

mod facts;
mod layouts;
mod projection;
mod resources;
mod signatures;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use facts::{
    PlannedProgram, PlanningError, TraceContext, TraceFacts, TraceLocation, TraceRequest,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use projection::plan_program;
#[cfg(test)]
pub(in crate::backend) use projection::project_resource_catalog;

#[cfg(test)]
mod tests;
