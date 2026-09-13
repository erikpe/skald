//! Structured diagnostics shared by pipeline phases.
//!
//! User errors are data, not Rust panics. Rendering is deterministic and kept
//! separate from diagnostic construction so tests and future IDE consumers can
//! inspect structure directly.

mod model;
mod render;
mod wording;

pub use model::{Diagnostic, Diagnostics, Label, LabelStyle, Severity};
pub(crate) use render::render_diagnostics_from_iter;
pub use render::{render_diagnostic, render_diagnostics};
pub(crate) use wording::format_type_list;

#[cfg(test)]
mod tests;
