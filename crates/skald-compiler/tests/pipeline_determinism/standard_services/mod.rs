//! Determinism generators for standard-library service integration.

mod io;
mod strings;

pub(crate) use io::{io_diagnostic_dump, io_phase_dump};
pub(crate) use strings::{string_diagnostic_dump, string_phase_dump};
