//! Whole-program admission and immutable final-MIR fact projection.
//!
//! Final-MIR queries stop at this boundary. Lowering consumes the admitted
//! program and its checked plan without consulting frontend state.

mod admission;
mod facts;
mod layouts;
mod projection;
mod resources;
mod signatures;

pub(in crate::backend) use admission::trivial_cleanup;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use facts::{
    AdmissionError, AdmittedProgram, TraceContext, TraceFacts, TraceLocation, TraceRequest,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use projection::admit;
#[cfg(test)]
pub(in crate::backend) use projection::project_resource_catalog;

#[cfg(test)]
mod tests;
