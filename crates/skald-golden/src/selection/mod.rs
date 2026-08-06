//! Canonical-ID, path-glob, exact, and variant selection.

mod error;
mod glob;
mod model;

pub use error::SelectionError;
pub use model::{select, SelectedPlan, SelectionOptions};
