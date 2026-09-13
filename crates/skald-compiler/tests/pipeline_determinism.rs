//! Cross-process determinism case registry.

#[path = "pipeline_determinism/mod.rs"]
mod determinism;
#[path = "../test_support/standard_library.rs"]
mod standard_library;
mod support;

use determinism::{
    array_element_list_phase_dump, array_phase_dump, assert_cross_process_determinism,
    assert_cross_process_variants, eager_boolean_diagnostic_dump, eager_boolean_phase_dump,
    final_field_diagnostic_dump, final_field_phase_dump, floating_comparison_diagnostic_dump,
    floating_comparison_phase_dump, floating_division_diagnostic_dump,
    floating_division_phase_dump, function_value_composition_phase_dump,
    generic_interface_diagnostic_dump, generic_interface_module_phase_dump,
    generic_module_phase_dump, generic_operator_module_phase_dump,
    imported_unused_static_phase_dump, indexed_array_frontend_phase_dump,
    integer_bitwise_and_shift_diagnostic_dump, integer_bitwise_and_shift_phase_dump,
    integer_division_diagnostic_dump, integer_division_phase_dump, integer_operation_phase_dump,
    io_diagnostic_dump, io_phase_dump, iteration_diagnostic_dump, iteration_module_phase_dump,
    mir_pipeline_checkpoint_dump, module_diagnostic_dump, module_phase_dump, object_phase_dump,
    optional_phase_dump, polymorphism_phase_dump, primitive_cast_diagnostic_dump,
    primitive_cast_phase_dump, primitive_operator_profile_phase_dump, private_cell_diagnostic_dump,
    private_cell_phase_dump, private_initializer_diagnostic_dump, private_initializer_phase_dump,
    produced_alias_phase_dump, produced_field_phase_dump, produced_receiver_phase_dump,
    range_module_phase_dump, range_syntax_diagnostic_dump, shared_ownership_phase_dump,
    short_circuit_source_phase_dump, static_field_diagnostic_dump, static_field_module_phase_dump,
    static_field_phase_dump, static_initializer_lifecycle_phase_dump,
    static_lifetime_cycle_diagnostic_dump, string_diagnostic_dump, string_phase_dump,
};

macro_rules! permutation_case {
    ($name:ident, $label:literal, $generator:path) => {
        #[test]
        fn $name() {
            assert_cross_process_variants($label, stringify!($name), $generator);
        }
    };
}

macro_rules! same_input_case {
    ($name:ident, $label:literal, $generator:path) => {
        #[test]
        fn $name() {
            assert_cross_process_determinism($label, stringify!($name), $generator);
        }
    };
}

// Objects, produced views, ownership, arrays, and statics.
same_input_case!(
    object_lifetime_phase_products_are_deterministic_across_processes,
    "object",
    object_phase_dump
);

same_input_case!(
    polymorphism_phase_products_are_deterministic_across_processes,
    "polymorphism",
    polymorphism_phase_dump
);
same_input_case!(
    produced_alias_phase_products_are_deterministic_across_processes,
    "produced-aliases",
    produced_alias_phase_dump
);
same_input_case!(
    produced_receiver_phase_products_are_deterministic_across_processes,
    "produced-receivers",
    produced_receiver_phase_dump
);
same_input_case!(
    produced_field_phase_products_are_deterministic_across_processes,
    "produced-fields",
    produced_field_phase_dump
);
same_input_case!(
    shared_ownership_phase_products_are_deterministic_across_processes,
    "shared-ownership",
    shared_ownership_phase_dump
);
same_input_case!(
    optional_value_phase_products_are_deterministic_across_processes,
    "optional-values",
    optional_phase_dump
);
same_input_case!(
    array_phase_products_are_deterministic_across_processes,
    "arrays",
    array_phase_dump
);
same_input_case!(
    array_element_list_phase_products_are_deterministic_across_processes,
    "array-element-lists",
    array_element_list_phase_dump
);
same_input_case!(
    indexed_array_frontend_products_are_deterministic_across_processes,
    "indexed-array-frontend",
    indexed_array_frontend_phase_dump
);
same_input_case!(
    static_field_phase_products_are_deterministic_across_processes,
    "static-fields",
    static_field_phase_dump
);
same_input_case!(
    static_initializer_lifecycle_products_are_deterministic_across_processes,
    "static-initializer-lifecycle",
    static_initializer_lifecycle_phase_dump
);
same_input_case!(
    static_lifetime_cycle_diagnostics_are_deterministic_across_processes,
    "static-lifetime-diagnostics",
    static_lifetime_cycle_diagnostic_dump
);
same_input_case!(
    static_field_diagnostics_are_deterministic_across_processes,
    "static-field-diagnostics",
    static_field_diagnostic_dump
);
permutation_case!(
    static_field_module_products_are_deterministic_across_processes,
    "static-field-modules",
    static_field_module_phase_dump
);
same_input_case!(
    imported_unused_static_products_are_deterministic_across_processes,
    "imported-unused-static-products",
    imported_unused_static_phase_dump
);

// Primitive numeric, cast, and boolean behavior.
same_input_case!(
    integer_operation_phase_products_are_deterministic_across_processes,
    "integer-operations",
    integer_operation_phase_dump
);
same_input_case!(
    integer_bitwise_and_shift_phase_products_are_deterministic_across_processes,
    "integer-bitwise-shifts",
    integer_bitwise_and_shift_phase_dump
);
same_input_case!(
    integer_bitwise_and_shift_diagnostics_are_deterministic_across_processes,
    "integer-bitwise-shift-diagnostics",
    integer_bitwise_and_shift_diagnostic_dump
);
same_input_case!(
    integer_division_phase_products_are_deterministic_across_processes,
    "integer-division",
    integer_division_phase_dump
);
same_input_case!(
    integer_division_diagnostics_are_deterministic_across_processes,
    "integer-division-diagnostics",
    integer_division_diagnostic_dump
);
same_input_case!(
    floating_division_phase_products_are_deterministic_across_processes,
    "floating-division",
    floating_division_phase_dump
);
same_input_case!(
    floating_division_diagnostics_are_deterministic_across_processes,
    "floating-division-diagnostics",
    floating_division_diagnostic_dump
);
same_input_case!(
    floating_comparison_phase_products_are_deterministic_across_processes,
    "floating-comparisons",
    floating_comparison_phase_dump
);
same_input_case!(
    floating_comparison_diagnostics_are_deterministic_across_processes,
    "floating-comparison-diagnostics",
    floating_comparison_diagnostic_dump
);
same_input_case!(
    primitive_operator_profile_phase_products_are_deterministic_across_processes,
    "primitive-operator-profile",
    primitive_operator_profile_phase_dump
);
same_input_case!(
    primitive_cast_phase_products_are_deterministic_across_processes,
    "primitive-casts",
    primitive_cast_phase_dump
);
same_input_case!(
    primitive_cast_diagnostics_are_deterministic_across_processes,
    "primitive-cast-diagnostics",
    primitive_cast_diagnostic_dump
);
same_input_case!(
    eager_boolean_phase_products_are_deterministic_across_processes,
    "eager-booleans",
    eager_boolean_phase_dump
);
same_input_case!(
    eager_boolean_diagnostics_are_deterministic_across_processes,
    "eager-boolean-diagnostics",
    eager_boolean_diagnostic_dump
);
same_input_case!(
    short_circuit_source_products_are_deterministic_across_processes,
    "short-circuit-source",
    short_circuit_source_phase_dump
);

// Standard-library string and I/O services.
permutation_case!(
    string_phase_products_are_deterministic_across_processes,
    "strings",
    string_phase_dump
);
permutation_case!(
    string_language_item_diagnostics_are_deterministic_across_processes,
    "string-diagnostics",
    string_diagnostic_dump
);
permutation_case!(
    io_phase_products_are_deterministic_across_processes,
    "io",
    io_phase_dump
);
permutation_case!(
    io_provider_diagnostics_are_deterministic_across_processes,
    "io-diagnostics",
    io_diagnostic_dump
);

// Private and final object state.
same_input_case!(
    private_initializer_phase_products_are_deterministic_across_processes,
    "private-initializers",
    private_initializer_phase_dump
);
same_input_case!(
    private_initializer_diagnostics_are_deterministic_across_processes,
    "private-initializer-diagnostics",
    private_initializer_diagnostic_dump
);
same_input_case!(
    private_cell_phase_products_are_deterministic_across_processes,
    "private-cells",
    private_cell_phase_dump
);
same_input_case!(
    private_cell_diagnostics_are_deterministic_across_processes,
    "private-cell-diagnostics",
    private_cell_diagnostic_dump
);
same_input_case!(
    final_field_phase_products_are_deterministic_across_processes,
    "final-fields",
    final_field_phase_dump
);
same_input_case!(
    final_field_diagnostics_are_deterministic_across_processes,
    "final-field-diagnostics",
    final_field_diagnostic_dump
);

// Modules, generics, ranges, and iteration.
permutation_case!(
    module_phase_products_are_deterministic_across_processes,
    "modules",
    module_phase_dump
);
permutation_case!(
    module_diagnostics_are_deterministic_across_processes,
    "module-diagnostics",
    module_diagnostic_dump
);
permutation_case!(
    generic_module_phase_products_are_deterministic_across_processes,
    "generic-modules",
    generic_module_phase_dump
);
permutation_case!(
    generic_interface_phase_products_are_deterministic_across_processes,
    "generic-interface-products",
    generic_interface_module_phase_dump
);
permutation_case!(
    generic_operator_phase_products_are_deterministic_across_processes,
    "generic-operator-products",
    generic_operator_module_phase_dump
);
permutation_case!(
    range_phase_products_are_deterministic_across_processes,
    "explicit-range-products",
    range_module_phase_dump
);
same_input_case!(
    range_syntax_diagnostics_are_deterministic_across_processes,
    "range-syntax-diagnostics",
    range_syntax_diagnostic_dump
);
same_input_case!(
    generic_interface_diagnostics_are_deterministic_across_processes,
    "generic-interface-diagnostics",
    generic_interface_diagnostic_dump
);
permutation_case!(
    general_iteration_phase_products_are_deterministic_across_processes,
    "general-iteration-products",
    iteration_module_phase_dump
);
permutation_case!(
    general_iteration_diagnostics_are_deterministic_across_processes,
    "general-iteration-diagnostics",
    iteration_diagnostic_dump
);

// Function values.
same_input_case!(
    function_value_composition_products_are_deterministic_across_processes,
    "function-value-composition",
    function_value_composition_phase_dump
);

// MIR pass observation.
same_input_case!(
    mir_pipeline_checkpoints_are_deterministic_across_processes,
    "mir-pipeline-checkpoints",
    mir_pipeline_checkpoint_dump
);
