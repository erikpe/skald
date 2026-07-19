//! Structured diagnostics shared by pipeline phases.
//!
//! User errors are data, not Rust panics. Rendering is deterministic and kept
//! separate from diagnostic construction so tests and future IDE consumers can
//! inspect structure directly.

mod model;
mod render;

pub use model::{Diagnostic, Diagnostics, Label, LabelStyle, Severity};
pub use render::{render_diagnostic, render_diagnostics};

#[cfg(test)]
mod tests;
