//! Closed generic-class request discovery and deterministic identity ownership.

mod closed_types;
mod owner;
mod requests;

pub(super) use requests::{discover_specializations, SpecializationDiscoveryInput};

#[cfg(test)]
mod tests;

use super::*;
use owner::SpecializationOwner;
