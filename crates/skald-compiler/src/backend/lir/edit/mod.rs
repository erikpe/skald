//! Consuming transformation owner; full verification is the only way back to publication.
mod editor;
mod rebuild;
mod references;
mod split;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use editor::{LoweredEditFailure, LoweredEditor};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use rebuild::LoweredRemap;

#[cfg(test)]
mod tests;
