//! Representation/checker regressions and a separate independent specification oracle.
mod checking;
mod fixtures;
mod oracle;
mod representation;

pub(in crate::backend) use oracle::check_native_resource_events;
