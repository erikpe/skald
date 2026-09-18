//! Representation tests and an independent specification oracle, not a placement seal.
mod oracle;
mod representation;

pub(in crate::backend) use oracle::check_native_resource_events;
