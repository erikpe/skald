//! Private facade for cross-process determinism cases and their fixtures.

mod fixture;
mod generics;
mod harness;
mod iteration;
mod modules;
mod normalization;
mod ranges;

pub(super) use fixture::{link_directory, write_source, ModuleFixture};
pub(super) use generics::{
    generic_interface_diagnostic_dump, generic_interface_module_phase_dump,
    generic_module_phase_dump, generic_operator_module_phase_dump,
};
pub(super) use harness::{assert_cross_process_determinism, assert_cross_process_variants};
pub(super) use iteration::{iteration_diagnostic_dump, iteration_module_phase_dump};
pub(super) use modules::{module_diagnostic_dump, module_phase_dump};
pub(super) use normalization::normalize_fixture_paths;
pub(super) use ranges::{range_module_phase_dump, range_syntax_diagnostic_dump};
