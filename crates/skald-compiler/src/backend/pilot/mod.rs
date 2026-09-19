//! Whole-program admission and immutable facts for the private native pilot.
//! No executable product is constructed until admission and plan checking succeed.

mod admission;
mod facts;
mod layouts;
mod lower;
mod projection;
mod signatures;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use facts::{
    AdmittedPilot, PilotError, TraceContext, TraceFacts, TraceLocation, TraceRequest,
    UnsupportedPilot,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use projection::admit;

#[cfg(test)]
mod tests;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use lower::{lower_next, lower_program, lower_program_with, LowerError};
