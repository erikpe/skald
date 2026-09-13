//! Private facade for cross-process determinism cases and their fixtures.

mod arrays;
mod checkpoints;
mod fixture;
mod function_values;
mod generics;
mod harness;
mod iteration;
mod modules;
mod normalization;
mod objects;
mod ownership;
mod primitives;
mod produced;
mod ranges;
mod source;
mod standard_services;
mod statics;

pub(super) use arrays::{
    array_element_list_phase_dump, array_phase_dump, indexed_array_frontend_phase_dump,
};
pub(super) use checkpoints::mir_pipeline_checkpoint_dump;
pub(super) use function_values::function_value_composition_phase_dump;
pub(super) use generics::{
    generic_interface_diagnostic_dump, generic_interface_module_phase_dump,
    generic_module_phase_dump, generic_operator_module_phase_dump,
};
pub(super) use harness::{assert_cross_process_determinism, assert_cross_process_variants};
pub(super) use iteration::{iteration_diagnostic_dump, iteration_module_phase_dump};
pub(super) use modules::{module_diagnostic_dump, module_phase_dump};
pub(super) use objects::{
    final_field_diagnostic_dump, final_field_phase_dump, object_phase_dump,
    polymorphism_phase_dump, private_cell_diagnostic_dump, private_cell_phase_dump,
    private_initializer_diagnostic_dump, private_initializer_phase_dump,
};
pub(super) use ownership::{optional_phase_dump, shared_ownership_phase_dump};
pub(super) use primitives::{
    eager_boolean_diagnostic_dump, eager_boolean_phase_dump, floating_comparison_diagnostic_dump,
    floating_comparison_phase_dump, floating_division_diagnostic_dump,
    floating_division_phase_dump, integer_bitwise_and_shift_diagnostic_dump,
    integer_bitwise_and_shift_phase_dump, integer_division_diagnostic_dump,
    integer_division_phase_dump, integer_operation_phase_dump, primitive_cast_diagnostic_dump,
    primitive_cast_phase_dump, primitive_operator_profile_phase_dump,
    short_circuit_source_phase_dump,
};
pub(super) use produced::{
    produced_alias_phase_dump, produced_field_phase_dump, produced_receiver_phase_dump,
};
pub(super) use ranges::{range_module_phase_dump, range_syntax_diagnostic_dump};
pub(super) use standard_services::{
    io_diagnostic_dump, io_phase_dump, string_diagnostic_dump, string_phase_dump,
};
pub(super) use statics::{
    imported_unused_static_phase_dump, static_field_diagnostic_dump,
    static_field_module_phase_dump, static_field_phase_dump,
    static_initializer_lifecycle_phase_dump, static_lifetime_cycle_diagnostic_dump,
};
