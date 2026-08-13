//! Closed generic-class request discovery and deterministic identity ownership.

mod closed_types;
mod declarations;
mod owner;
mod requests;
mod validation;

pub(super) use declarations::specialize_declarations;
pub(super) use requests::{discover_specializations, SpecializationDiscoveryInput};
pub(super) use validation::validate_specialization_requirements;

#[cfg(test)]
mod declaration_tests;
#[cfg(test)]
mod tests;

use super::*;
use owner::SpecializationOwner;
