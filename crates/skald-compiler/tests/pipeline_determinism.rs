//! Cross-process determinism coverage for representative complete pipelines.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use skald_compiler::{
    backend::{emit_assembly, BackendInput, Target},
    diagnostics::render_diagnostics,
    driver::EntrySelector,
    hir::{dump_hir, HirProgram},
    lexer::{dump_tokens, lex},
    mir::{dump_mir, dump_preliminary_mir, lower_hir, lower_preliminary_hir},
    module::{
        dump_module_graph, load_module_graph, normalize_provider_roots, ProviderRootConfiguration,
    },
    passes::{
        run_mir_pipeline, run_mir_pipeline_inspected,
        static_lifecycle::{
            dump_planned_mir, plan_static_lifetimes, synthesize_static_lifecycle,
            verify_planned_mir,
        },
        MirPipelineCheckpoint, VerifiedFinalMirProgram,
    },
    resolve::{dump_resolved, resolve, resolve_module_graph},
    source::SourceDatabase,
    syntax::{dump_ast, parse},
    typeck::type_check,
};

#[path = "../test_support/standard_library.rs"]
mod standard_library;
use standard_library::{canonical_standard_library_sources, CANONICAL_IO_SOURCE};

const OBJECT_HELPER_OUTPUT: &str = "SKALD_OBJECT_DETERMINISM_OUTPUT";
const OBJECT_TEST_NAME: &str = "object_lifetime_phase_products_are_deterministic_across_processes";
const POLYMORPHISM_HELPER_OUTPUT: &str = "SKALD_POLYMORPHISM_DETERMINISM_OUTPUT";
const POLYMORPHISM_TEST_NAME: &str =
    "polymorphism_phase_products_are_deterministic_across_processes";
const PRODUCED_ALIAS_HELPER_OUTPUT: &str = "SKALD_PRODUCED_ALIAS_DETERMINISM_OUTPUT";
const PRODUCED_ALIAS_TEST_NAME: &str =
    "produced_alias_phase_products_are_deterministic_across_processes";
const PRODUCED_RECEIVER_HELPER_OUTPUT: &str = "SKALD_PRODUCED_RECEIVER_DETERMINISM_OUTPUT";
const PRODUCED_RECEIVER_TEST_NAME: &str =
    "produced_receiver_phase_products_are_deterministic_across_processes";
const PRODUCED_FIELD_HELPER_OUTPUT: &str = "SKALD_PRODUCED_FIELD_DETERMINISM_OUTPUT";
const PRODUCED_FIELD_TEST_NAME: &str =
    "produced_field_phase_products_are_deterministic_across_processes";
const SHARED_HELPER_OUTPUT: &str = "SKALD_SHARED_DETERMINISM_OUTPUT";
const SHARED_TEST_NAME: &str = "shared_ownership_phase_products_are_deterministic_across_processes";
const OPTIONAL_HELPER_OUTPUT: &str = "SKALD_OPTIONAL_DETERMINISM_OUTPUT";
const OPTIONAL_TEST_NAME: &str = "optional_value_phase_products_are_deterministic_across_processes";
const ARRAY_HELPER_OUTPUT: &str = "SKALD_ARRAY_DETERMINISM_OUTPUT";
const ARRAY_TEST_NAME: &str = "array_phase_products_are_deterministic_across_processes";
const ARRAY_ELEMENT_LIST_HELPER_OUTPUT: &str = "SKALD_ARRAY_ELEMENT_LIST_DETERMINISM_OUTPUT";
const ARRAY_ELEMENT_LIST_TEST_NAME: &str =
    "array_element_list_phase_products_are_deterministic_across_processes";
const INTEGER_OPERATION_HELPER_OUTPUT: &str = "SKALD_INTEGER_OPERATION_DETERMINISM_OUTPUT";
const INTEGER_OPERATION_TEST_NAME: &str =
    "integer_operation_phase_products_are_deterministic_across_processes";
const INTEGER_BITWISE_SHIFT_HELPER_OUTPUT: &str = "SKALD_INTEGER_BITWISE_SHIFT_DETERMINISM_OUTPUT";
const INTEGER_BITWISE_SHIFT_TEST_NAME: &str =
    "integer_bitwise_and_shift_phase_products_are_deterministic_across_processes";
const INTEGER_BITWISE_SHIFT_DIAGNOSTIC_HELPER_OUTPUT: &str =
    "SKALD_INTEGER_BITWISE_SHIFT_DIAGNOSTIC_DETERMINISM_OUTPUT";
const INTEGER_BITWISE_SHIFT_DIAGNOSTIC_TEST_NAME: &str =
    "integer_bitwise_and_shift_diagnostics_are_deterministic_across_processes";
const INTEGER_DIVISION_HELPER_OUTPUT: &str = "SKALD_INTEGER_DIVISION_DETERMINISM_OUTPUT";
const INTEGER_DIVISION_TEST_NAME: &str =
    "integer_division_phase_products_are_deterministic_across_processes";
const INTEGER_DIVISION_DIAGNOSTIC_HELPER_OUTPUT: &str =
    "SKALD_INTEGER_DIVISION_DIAGNOSTIC_DETERMINISM_OUTPUT";
const INTEGER_DIVISION_DIAGNOSTIC_TEST_NAME: &str =
    "integer_division_diagnostics_are_deterministic_across_processes";
const FLOATING_DIVISION_HELPER_OUTPUT: &str = "SKALD_FLOATING_DIVISION_DETERMINISM_OUTPUT";
const FLOATING_DIVISION_TEST_NAME: &str =
    "floating_division_phase_products_are_deterministic_across_processes";
const FLOATING_DIVISION_DIAGNOSTIC_HELPER_OUTPUT: &str =
    "SKALD_FLOATING_DIVISION_DIAGNOSTIC_DETERMINISM_OUTPUT";
const FLOATING_DIVISION_DIAGNOSTIC_TEST_NAME: &str =
    "floating_division_diagnostics_are_deterministic_across_processes";
const FLOATING_COMPARISON_HELPER_OUTPUT: &str = "SKALD_FLOATING_COMPARISON_DETERMINISM_OUTPUT";
const FLOATING_COMPARISON_TEST_NAME: &str =
    "floating_comparison_phase_products_are_deterministic_across_processes";
const FLOATING_COMPARISON_DIAGNOSTIC_HELPER_OUTPUT: &str =
    "SKALD_FLOATING_COMPARISON_DIAGNOSTIC_DETERMINISM_OUTPUT";
const FLOATING_COMPARISON_DIAGNOSTIC_TEST_NAME: &str =
    "floating_comparison_diagnostics_are_deterministic_across_processes";
const PRIMITIVE_OPERATOR_PROFILE_HELPER_OUTPUT: &str =
    "SKALD_PRIMITIVE_OPERATOR_PROFILE_DETERMINISM_OUTPUT";
const PRIMITIVE_OPERATOR_PROFILE_TEST_NAME: &str =
    "primitive_operator_profile_phase_products_are_deterministic_across_processes";
const PRIMITIVE_CAST_HELPER_OUTPUT: &str = "SKALD_PRIMITIVE_CAST_DETERMINISM_OUTPUT";
const PRIMITIVE_CAST_TEST_NAME: &str =
    "primitive_cast_phase_products_are_deterministic_across_processes";
const PRIMITIVE_CAST_DIAGNOSTIC_HELPER_OUTPUT: &str =
    "SKALD_PRIMITIVE_CAST_DIAGNOSTIC_DETERMINISM_OUTPUT";
const PRIMITIVE_CAST_DIAGNOSTIC_TEST_NAME: &str =
    "primitive_cast_diagnostics_are_deterministic_across_processes";
const EAGER_BOOLEAN_HELPER_OUTPUT: &str = "SKALD_EAGER_BOOLEAN_DETERMINISM_OUTPUT";
const EAGER_BOOLEAN_TEST_NAME: &str =
    "eager_boolean_phase_products_are_deterministic_across_processes";
const EAGER_BOOLEAN_DIAGNOSTIC_HELPER_OUTPUT: &str =
    "SKALD_EAGER_BOOLEAN_DIAGNOSTIC_DETERMINISM_OUTPUT";
const EAGER_BOOLEAN_DIAGNOSTIC_TEST_NAME: &str =
    "eager_boolean_diagnostics_are_deterministic_across_processes";
const SHORT_CIRCUIT_SOURCE_HELPER_OUTPUT: &str = "SKALD_SHORT_CIRCUIT_SOURCE_DETERMINISM_OUTPUT";
const SHORT_CIRCUIT_SOURCE_TEST_NAME: &str =
    "short_circuit_source_products_are_deterministic_across_processes";
const STRING_HELPER_OUTPUT: &str = "SKALD_STRING_DETERMINISM_OUTPUT";
const STRING_TEST_NAME: &str = "string_phase_products_are_deterministic_across_processes";
const STRING_DIAGNOSTIC_HELPER_OUTPUT: &str = "SKALD_STRING_DIAGNOSTIC_DETERMINISM_OUTPUT";
const STRING_DIAGNOSTIC_TEST_NAME: &str =
    "string_language_item_diagnostics_are_deterministic_across_processes";
const IO_HELPER_OUTPUT: &str = "SKALD_IO_DETERMINISM_OUTPUT";
const IO_TEST_NAME: &str = "io_phase_products_are_deterministic_across_processes";
const IO_DIAGNOSTIC_HELPER_OUTPUT: &str = "SKALD_IO_DIAGNOSTIC_DETERMINISM_OUTPUT";
const IO_DIAGNOSTIC_TEST_NAME: &str = "io_provider_diagnostics_are_deterministic_across_processes";
const PRIVATE_INITIALIZER_HELPER_OUTPUT: &str = "SKALD_PRIVATE_INITIALIZER_DETERMINISM_OUTPUT";
const PRIVATE_INITIALIZER_TEST_NAME: &str =
    "private_initializer_phase_products_are_deterministic_across_processes";
const PRIVATE_INITIALIZER_DIAGNOSTIC_HELPER_OUTPUT: &str =
    "SKALD_PRIVATE_INITIALIZER_DIAGNOSTIC_DETERMINISM_OUTPUT";
const PRIVATE_INITIALIZER_DIAGNOSTIC_TEST_NAME: &str =
    "private_initializer_diagnostics_are_deterministic_across_processes";
const PRIVATE_CELL_HELPER_OUTPUT: &str = "SKALD_PRIVATE_CELL_DETERMINISM_OUTPUT";
const PRIVATE_CELL_TEST_NAME: &str =
    "private_cell_phase_products_are_deterministic_across_processes";
const PRIVATE_CELL_DIAGNOSTIC_HELPER_OUTPUT: &str =
    "SKALD_PRIVATE_CELL_DIAGNOSTIC_DETERMINISM_OUTPUT";
const PRIVATE_CELL_DIAGNOSTIC_TEST_NAME: &str =
    "private_cell_diagnostics_are_deterministic_across_processes";
const FINAL_FIELD_HELPER_OUTPUT: &str = "SKALD_FINAL_FIELD_DETERMINISM_OUTPUT";
const FINAL_FIELD_TEST_NAME: &str = "final_field_phase_products_are_deterministic_across_processes";
const FINAL_FIELD_DIAGNOSTIC_HELPER_OUTPUT: &str =
    "SKALD_FINAL_FIELD_DIAGNOSTIC_DETERMINISM_OUTPUT";
const FINAL_FIELD_DIAGNOSTIC_TEST_NAME: &str =
    "final_field_diagnostics_are_deterministic_across_processes";
const MODULE_HELPER_OUTPUT: &str = "SKALD_MODULE_DETERMINISM_OUTPUT";
const PERMUTATION_HELPER_VARIANT: &str = "SKALD_DETERMINISM_VARIANT";
const MODULE_TEST_NAME: &str = "module_phase_products_are_deterministic_across_processes";
const MODULE_DIAGNOSTIC_HELPER_OUTPUT: &str = "SKALD_MODULE_DIAGNOSTIC_DETERMINISM_OUTPUT";
const MODULE_DIAGNOSTIC_TEST_NAME: &str = "module_diagnostics_are_deterministic_across_processes";
const GENERIC_MODULE_HELPER_OUTPUT: &str = "SKALD_GENERIC_MODULE_DETERMINISM_OUTPUT";
const GENERIC_MODULE_TEST_NAME: &str =
    "generic_module_phase_products_are_deterministic_across_processes";
const GENERIC_INTERFACE_HELPER_OUTPUT: &str = "SKALD_GENERIC_INTERFACE_DETERMINISM_OUTPUT";
const GENERIC_INTERFACE_TEST_NAME: &str =
    "generic_interface_phase_products_are_deterministic_across_processes";
const GENERIC_OPERATOR_HELPER_OUTPUT: &str = "SKALD_GENERIC_OPERATOR_DETERMINISM_OUTPUT";
const GENERIC_OPERATOR_TEST_NAME: &str =
    "generic_operator_phase_products_are_deterministic_across_processes";
const RANGE_HELPER_OUTPUT: &str = "SKALD_RANGE_DETERMINISM_OUTPUT";
const RANGE_TEST_NAME: &str = "range_phase_products_are_deterministic_across_processes";
const GENERIC_INTERFACE_DIAGNOSTIC_HELPER_OUTPUT: &str =
    "SKALD_GENERIC_INTERFACE_DIAGNOSTIC_DETERMINISM_OUTPUT";
const GENERIC_INTERFACE_DIAGNOSTIC_TEST_NAME: &str =
    "generic_interface_diagnostics_are_deterministic_across_processes";
const ITERATION_HELPER_OUTPUT: &str = "SKALD_ITERATION_DETERMINISM_OUTPUT";
const ITERATION_TEST_NAME: &str =
    "general_iteration_phase_products_are_deterministic_across_processes";
const ITERATION_DIAGNOSTIC_HELPER_OUTPUT: &str = "SKALD_ITERATION_DIAGNOSTIC_DETERMINISM_OUTPUT";
const ITERATION_DIAGNOSTIC_TEST_NAME: &str =
    "general_iteration_diagnostics_are_deterministic_across_processes";
const FUNCTION_VALUE_COMPOSITION_HELPER_OUTPUT: &str =
    "SKALD_FUNCTION_VALUE_COMPOSITION_DETERMINISM_OUTPUT";
const FUNCTION_VALUE_COMPOSITION_TEST_NAME: &str =
    "function_value_composition_products_are_deterministic_across_processes";
const STATIC_FIELD_HELPER_OUTPUT: &str = "SKALD_STATIC_FIELD_DETERMINISM_OUTPUT";
const STATIC_FIELD_TEST_NAME: &str =
    "static_field_phase_products_are_deterministic_across_processes";
const STATIC_INITIALIZER_LIFECYCLE_HELPER_OUTPUT: &str =
    "SKALD_STATIC_INITIALIZER_LIFECYCLE_DETERMINISM_OUTPUT";
const STATIC_INITIALIZER_LIFECYCLE_TEST_NAME: &str =
    "static_initializer_lifecycle_products_are_deterministic_across_processes";
const STATIC_LIFETIME_DIAGNOSTIC_HELPER_OUTPUT: &str =
    "SKALD_STATIC_LIFETIME_DIAGNOSTIC_DETERMINISM_OUTPUT";
const STATIC_LIFETIME_DIAGNOSTIC_TEST_NAME: &str =
    "static_lifetime_cycle_diagnostics_are_deterministic_across_processes";
const STATIC_FIELD_DIAGNOSTIC_HELPER_OUTPUT: &str =
    "SKALD_STATIC_FIELD_DIAGNOSTIC_DETERMINISM_OUTPUT";
const STATIC_FIELD_DIAGNOSTIC_TEST_NAME: &str =
    "static_field_diagnostics_are_deterministic_across_processes";
const STATIC_FIELD_MODULE_HELPER_OUTPUT: &str = "SKALD_STATIC_FIELD_MODULE_DETERMINISM_OUTPUT";
const STATIC_FIELD_MODULE_TEST_NAME: &str =
    "static_field_module_products_are_deterministic_across_processes";
const MIR_CHECKPOINT_HELPER_OUTPUT: &str = "SKALD_MIR_CHECKPOINT_DETERMINISM_OUTPUT";
const MIR_CHECKPOINT_TEST_NAME: &str =
    "mir_pipeline_checkpoints_are_deterministic_across_processes";

#[test]
fn object_lifetime_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "object",
        OBJECT_HELPER_OUTPUT,
        OBJECT_TEST_NAME,
        object_phase_dump,
    );
}

#[test]
fn mir_pipeline_checkpoints_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "mir-pipeline-checkpoints",
        MIR_CHECKPOINT_HELPER_OUTPUT,
        MIR_CHECKPOINT_TEST_NAME,
        mir_pipeline_checkpoint_dump,
    );
}

#[test]
fn polymorphism_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "polymorphism",
        POLYMORPHISM_HELPER_OUTPUT,
        POLYMORPHISM_TEST_NAME,
        polymorphism_phase_dump,
    );
}

#[test]
fn produced_alias_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "produced-aliases",
        PRODUCED_ALIAS_HELPER_OUTPUT,
        PRODUCED_ALIAS_TEST_NAME,
        produced_alias_phase_dump,
    );
}

#[test]
fn produced_receiver_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "produced-receivers",
        PRODUCED_RECEIVER_HELPER_OUTPUT,
        PRODUCED_RECEIVER_TEST_NAME,
        produced_receiver_phase_dump,
    );
}

#[test]
fn produced_field_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "produced-fields",
        PRODUCED_FIELD_HELPER_OUTPUT,
        PRODUCED_FIELD_TEST_NAME,
        produced_field_phase_dump,
    );
}

#[test]
fn shared_ownership_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "shared-ownership",
        SHARED_HELPER_OUTPUT,
        SHARED_TEST_NAME,
        shared_ownership_phase_dump,
    );
}

#[test]
fn optional_value_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "optional-values",
        OPTIONAL_HELPER_OUTPUT,
        OPTIONAL_TEST_NAME,
        optional_phase_dump,
    );
}

#[test]
fn array_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "arrays",
        ARRAY_HELPER_OUTPUT,
        ARRAY_TEST_NAME,
        array_phase_dump,
    );
}

#[test]
fn array_element_list_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "array-element-lists",
        ARRAY_ELEMENT_LIST_HELPER_OUTPUT,
        ARRAY_ELEMENT_LIST_TEST_NAME,
        array_element_list_phase_dump,
    );
}

#[test]
fn static_field_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "static-fields",
        STATIC_FIELD_HELPER_OUTPUT,
        STATIC_FIELD_TEST_NAME,
        static_field_phase_dump,
    );
}

#[test]
fn static_initializer_lifecycle_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "static-initializer-lifecycle",
        STATIC_INITIALIZER_LIFECYCLE_HELPER_OUTPUT,
        STATIC_INITIALIZER_LIFECYCLE_TEST_NAME,
        static_initializer_lifecycle_phase_dump,
    );
}

#[test]
fn static_lifetime_cycle_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "static-lifetime-diagnostics",
        STATIC_LIFETIME_DIAGNOSTIC_HELPER_OUTPUT,
        STATIC_LIFETIME_DIAGNOSTIC_TEST_NAME,
        static_lifetime_cycle_diagnostic_dump,
    );
}

#[test]
fn static_field_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "static-field-diagnostics",
        STATIC_FIELD_DIAGNOSTIC_HELPER_OUTPUT,
        STATIC_FIELD_DIAGNOSTIC_TEST_NAME,
        static_field_diagnostic_dump,
    );
}

#[test]
fn static_field_module_products_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(STATIC_FIELD_MODULE_HELPER_OUTPUT) {
        let variant = env::var(PERMUTATION_HELPER_VARIANT)
            .unwrap()
            .parse()
            .unwrap();
        fs::write(output, static_field_module_phase_dump(variant)).unwrap();
        return;
    }

    assert_cross_process_variants(
        "static-field-modules",
        STATIC_FIELD_MODULE_HELPER_OUTPUT,
        STATIC_FIELD_MODULE_TEST_NAME,
        PERMUTATION_HELPER_VARIANT,
    );
}

#[test]
fn integer_operation_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "integer-operations",
        INTEGER_OPERATION_HELPER_OUTPUT,
        INTEGER_OPERATION_TEST_NAME,
        integer_operation_phase_dump,
    );
}

#[test]
fn integer_bitwise_and_shift_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "integer-bitwise-shifts",
        INTEGER_BITWISE_SHIFT_HELPER_OUTPUT,
        INTEGER_BITWISE_SHIFT_TEST_NAME,
        integer_bitwise_and_shift_phase_dump,
    );
}

#[test]
fn integer_bitwise_and_shift_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "integer-bitwise-shift-diagnostics",
        INTEGER_BITWISE_SHIFT_DIAGNOSTIC_HELPER_OUTPUT,
        INTEGER_BITWISE_SHIFT_DIAGNOSTIC_TEST_NAME,
        integer_bitwise_and_shift_diagnostic_dump,
    );
}

#[test]
fn integer_division_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "integer-division",
        INTEGER_DIVISION_HELPER_OUTPUT,
        INTEGER_DIVISION_TEST_NAME,
        integer_division_phase_dump,
    );
}

#[test]
fn integer_division_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "integer-division-diagnostics",
        INTEGER_DIVISION_DIAGNOSTIC_HELPER_OUTPUT,
        INTEGER_DIVISION_DIAGNOSTIC_TEST_NAME,
        integer_division_diagnostic_dump,
    );
}

#[test]
fn floating_division_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "floating-division",
        FLOATING_DIVISION_HELPER_OUTPUT,
        FLOATING_DIVISION_TEST_NAME,
        floating_division_phase_dump,
    );
}

#[test]
fn floating_division_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "floating-division-diagnostics",
        FLOATING_DIVISION_DIAGNOSTIC_HELPER_OUTPUT,
        FLOATING_DIVISION_DIAGNOSTIC_TEST_NAME,
        floating_division_diagnostic_dump,
    );
}

#[test]
fn floating_comparison_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "floating-comparisons",
        FLOATING_COMPARISON_HELPER_OUTPUT,
        FLOATING_COMPARISON_TEST_NAME,
        floating_comparison_phase_dump,
    );
}

#[test]
fn floating_comparison_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "floating-comparison-diagnostics",
        FLOATING_COMPARISON_DIAGNOSTIC_HELPER_OUTPUT,
        FLOATING_COMPARISON_DIAGNOSTIC_TEST_NAME,
        floating_comparison_diagnostic_dump,
    );
}

#[test]
fn primitive_operator_profile_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "primitive-operator-profile",
        PRIMITIVE_OPERATOR_PROFILE_HELPER_OUTPUT,
        PRIMITIVE_OPERATOR_PROFILE_TEST_NAME,
        primitive_operator_profile_phase_dump,
    );
}

#[test]
fn primitive_cast_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "primitive-casts",
        PRIMITIVE_CAST_HELPER_OUTPUT,
        PRIMITIVE_CAST_TEST_NAME,
        primitive_cast_phase_dump,
    );
}

#[test]
fn primitive_cast_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "primitive-cast-diagnostics",
        PRIMITIVE_CAST_DIAGNOSTIC_HELPER_OUTPUT,
        PRIMITIVE_CAST_DIAGNOSTIC_TEST_NAME,
        primitive_cast_diagnostic_dump,
    );
}

#[test]
fn eager_boolean_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "eager-booleans",
        EAGER_BOOLEAN_HELPER_OUTPUT,
        EAGER_BOOLEAN_TEST_NAME,
        eager_boolean_phase_dump,
    );
}

#[test]
fn eager_boolean_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "eager-boolean-diagnostics",
        EAGER_BOOLEAN_DIAGNOSTIC_HELPER_OUTPUT,
        EAGER_BOOLEAN_DIAGNOSTIC_TEST_NAME,
        eager_boolean_diagnostic_dump,
    );
}

#[test]
fn short_circuit_source_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "short-circuit-source",
        SHORT_CIRCUIT_SOURCE_HELPER_OUTPUT,
        SHORT_CIRCUIT_SOURCE_TEST_NAME,
        short_circuit_source_phase_dump,
    );
}

#[test]
fn string_phase_products_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(STRING_HELPER_OUTPUT) {
        let variant = env::var(PERMUTATION_HELPER_VARIANT)
            .unwrap()
            .parse()
            .unwrap();
        fs::write(output, string_phase_dump(variant)).unwrap();
        return;
    }

    assert_cross_process_variants(
        "strings",
        STRING_HELPER_OUTPUT,
        STRING_TEST_NAME,
        PERMUTATION_HELPER_VARIANT,
    );
}

#[test]
fn string_language_item_diagnostics_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(STRING_DIAGNOSTIC_HELPER_OUTPUT) {
        let variant = env::var(PERMUTATION_HELPER_VARIANT)
            .unwrap()
            .parse()
            .unwrap();
        fs::write(output, string_diagnostic_dump(variant)).unwrap();
        return;
    }

    assert_cross_process_variants(
        "string-diagnostics",
        STRING_DIAGNOSTIC_HELPER_OUTPUT,
        STRING_DIAGNOSTIC_TEST_NAME,
        PERMUTATION_HELPER_VARIANT,
    );
}

#[test]
fn io_phase_products_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(IO_HELPER_OUTPUT) {
        let variant = env::var(PERMUTATION_HELPER_VARIANT)
            .unwrap()
            .parse()
            .unwrap();
        fs::write(output, io_phase_dump(variant, false)).unwrap();
        return;
    }
    assert_cross_process_variants(
        "io",
        IO_HELPER_OUTPUT,
        IO_TEST_NAME,
        PERMUTATION_HELPER_VARIANT,
    );
}

#[test]
fn io_provider_diagnostics_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(IO_DIAGNOSTIC_HELPER_OUTPUT) {
        let variant = env::var(PERMUTATION_HELPER_VARIANT)
            .unwrap()
            .parse()
            .unwrap();
        fs::write(output, io_phase_dump(variant, true)).unwrap();
        return;
    }
    assert_cross_process_variants(
        "io-diagnostics",
        IO_DIAGNOSTIC_HELPER_OUTPUT,
        IO_DIAGNOSTIC_TEST_NAME,
        PERMUTATION_HELPER_VARIANT,
    );
}

#[test]
fn private_initializer_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "private-initializers",
        PRIVATE_INITIALIZER_HELPER_OUTPUT,
        PRIVATE_INITIALIZER_TEST_NAME,
        private_initializer_phase_dump,
    );
}

#[test]
fn private_initializer_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "private-initializer-diagnostics",
        PRIVATE_INITIALIZER_DIAGNOSTIC_HELPER_OUTPUT,
        PRIVATE_INITIALIZER_DIAGNOSTIC_TEST_NAME,
        private_initializer_diagnostic_dump,
    );
}

#[test]
fn private_cell_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "private-cells",
        PRIVATE_CELL_HELPER_OUTPUT,
        PRIVATE_CELL_TEST_NAME,
        private_cell_phase_dump,
    );
}

#[test]
fn private_cell_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "private-cell-diagnostics",
        PRIVATE_CELL_DIAGNOSTIC_HELPER_OUTPUT,
        PRIVATE_CELL_DIAGNOSTIC_TEST_NAME,
        private_cell_diagnostic_dump,
    );
}

#[test]
fn final_field_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "final-fields",
        FINAL_FIELD_HELPER_OUTPUT,
        FINAL_FIELD_TEST_NAME,
        final_field_phase_dump,
    );
}

#[test]
fn final_field_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "final-field-diagnostics",
        FINAL_FIELD_DIAGNOSTIC_HELPER_OUTPUT,
        FINAL_FIELD_DIAGNOSTIC_TEST_NAME,
        final_field_diagnostic_dump,
    );
}

#[test]
fn module_phase_products_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(MODULE_HELPER_OUTPUT) {
        let variant = env::var(PERMUTATION_HELPER_VARIANT)
            .unwrap()
            .parse()
            .unwrap();
        fs::write(output, module_phase_dump(variant)).unwrap();
        return;
    }

    assert_cross_process_variants(
        "modules",
        MODULE_HELPER_OUTPUT,
        MODULE_TEST_NAME,
        PERMUTATION_HELPER_VARIANT,
    );
}

#[test]
fn module_diagnostics_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(MODULE_DIAGNOSTIC_HELPER_OUTPUT) {
        let variant = env::var(PERMUTATION_HELPER_VARIANT)
            .unwrap()
            .parse()
            .unwrap();
        fs::write(output, module_diagnostic_dump(variant)).unwrap();
        return;
    }

    assert_cross_process_variants(
        "module-diagnostics",
        MODULE_DIAGNOSTIC_HELPER_OUTPUT,
        MODULE_DIAGNOSTIC_TEST_NAME,
        PERMUTATION_HELPER_VARIANT,
    );
}

#[test]
fn generic_module_phase_products_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(GENERIC_MODULE_HELPER_OUTPUT) {
        let variant = env::var(PERMUTATION_HELPER_VARIANT)
            .unwrap()
            .parse()
            .unwrap();
        fs::write(output, generic_module_phase_dump(variant)).unwrap();
        return;
    }

    assert_cross_process_variants(
        "generic-modules",
        GENERIC_MODULE_HELPER_OUTPUT,
        GENERIC_MODULE_TEST_NAME,
        PERMUTATION_HELPER_VARIANT,
    );
}

#[test]
fn generic_interface_phase_products_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(GENERIC_INTERFACE_HELPER_OUTPUT) {
        let variant = env::var(PERMUTATION_HELPER_VARIANT)
            .unwrap()
            .parse()
            .unwrap();
        fs::write(output, generic_interface_module_phase_dump(variant)).unwrap();
        return;
    }

    assert_cross_process_variants(
        "generic-interface-products",
        GENERIC_INTERFACE_HELPER_OUTPUT,
        GENERIC_INTERFACE_TEST_NAME,
        PERMUTATION_HELPER_VARIANT,
    );
}

#[test]
fn generic_operator_phase_products_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(GENERIC_OPERATOR_HELPER_OUTPUT) {
        let variant = env::var(PERMUTATION_HELPER_VARIANT)
            .unwrap()
            .parse()
            .unwrap();
        fs::write(output, generic_operator_module_phase_dump(variant)).unwrap();
        return;
    }

    assert_cross_process_variants(
        "generic-operator-products",
        GENERIC_OPERATOR_HELPER_OUTPUT,
        GENERIC_OPERATOR_TEST_NAME,
        PERMUTATION_HELPER_VARIANT,
    );
}

#[test]
fn range_phase_products_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(RANGE_HELPER_OUTPUT) {
        let variant = env::var(PERMUTATION_HELPER_VARIANT)
            .unwrap()
            .parse()
            .unwrap();
        fs::write(output, range_module_phase_dump(variant)).unwrap();
        return;
    }

    assert_cross_process_variants(
        "explicit-range-products",
        RANGE_HELPER_OUTPUT,
        RANGE_TEST_NAME,
        PERMUTATION_HELPER_VARIANT,
    );
}

#[test]
fn generic_interface_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "generic-interface-diagnostics",
        GENERIC_INTERFACE_DIAGNOSTIC_HELPER_OUTPUT,
        GENERIC_INTERFACE_DIAGNOSTIC_TEST_NAME,
        generic_interface_diagnostic_dump,
    );
}

#[test]
fn general_iteration_phase_products_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(ITERATION_HELPER_OUTPUT) {
        let variant = env::var(PERMUTATION_HELPER_VARIANT)
            .unwrap()
            .parse()
            .unwrap();
        fs::write(output, iteration_module_phase_dump(variant)).unwrap();
        return;
    }

    assert_cross_process_variants(
        "general-iteration-products",
        ITERATION_HELPER_OUTPUT,
        ITERATION_TEST_NAME,
        PERMUTATION_HELPER_VARIANT,
    );
}

#[test]
fn general_iteration_diagnostics_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(ITERATION_DIAGNOSTIC_HELPER_OUTPUT) {
        let variant = env::var(PERMUTATION_HELPER_VARIANT)
            .unwrap()
            .parse()
            .unwrap();
        fs::write(output, iteration_diagnostic_dump(variant)).unwrap();
        return;
    }

    assert_cross_process_variants(
        "general-iteration-diagnostics",
        ITERATION_DIAGNOSTIC_HELPER_OUTPUT,
        ITERATION_DIAGNOSTIC_TEST_NAME,
        PERMUTATION_HELPER_VARIANT,
    );
}

#[test]
fn function_value_composition_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "function-value-composition",
        FUNCTION_VALUE_COMPOSITION_HELPER_OUTPUT,
        FUNCTION_VALUE_COMPOSITION_TEST_NAME,
        function_value_composition_phase_dump,
    );
}

fn assert_cross_process_determinism(
    label: &str,
    helper_output: &str,
    test_name: &str,
    generate: fn() -> String,
) {
    if let Some(output) = env::var_os(helper_output) {
        fs::write(output, generate()).unwrap();
        return;
    }

    let artifacts = TemporaryArtifacts::new(label);
    run_helper_process(&artifacts.first, helper_output, test_name);
    run_helper_process(&artifacts.second, helper_output, test_name);

    assert_eq!(
        fs::read(&artifacts.first).unwrap(),
        fs::read(&artifacts.second).unwrap(),
        "{label} phase products changed across independent compiler processes"
    );
}

fn run_helper_process(output: &Path, helper_output: &str, test_name: &str) {
    let result = Command::new(env::current_exe().unwrap())
        .args(["--exact", test_name, "--nocapture"])
        .env(helper_output, output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "determinism helper failed:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn assert_cross_process_variants(
    label: &str,
    helper_output: &str,
    test_name: &str,
    variant_environment: &str,
) {
    let artifacts = TemporaryArtifacts::new(label);
    run_variant_helper_process(
        &artifacts.first,
        helper_output,
        test_name,
        variant_environment,
        0,
    );
    run_variant_helper_process(
        &artifacts.second,
        helper_output,
        test_name,
        variant_environment,
        1,
    );

    assert_eq!(
        fs::read(&artifacts.first).unwrap(),
        fs::read(&artifacts.second).unwrap(),
        "{label} products changed across independent compiler processes and input permutations"
    );
}

fn run_variant_helper_process(
    output: &Path,
    helper_output: &str,
    test_name: &str,
    variant_environment: &str,
    variant: usize,
) {
    let result = Command::new(env::current_exe().unwrap())
        .args(["--exact", test_name, "--nocapture"])
        .env(helper_output, output)
        .env(variant_environment, variant.to_string())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "determinism helper failed:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn module_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("module-products", variant);
    let application = fixture.path.join("application");
    let dependencies = fixture.path.join("dependencies");
    let application_alias = fixture.path.join("application-alias");
    link_directory(&application, &application_alias);

    let imports = if variant == 0 {
        "import first;\nimport second;\nfrom second import Item as SecondItem;\n"
    } else {
        "from second import Item as SecondItem;\nimport second;\nimport first;\n"
    };
    let sources = [
        (
            application.join("app.ska"),
            format!(
                "{imports}\n{}",
                source_body_after_imports(include_str!(
                    "../../../tests/golden/modules/cases/cycle/modules/app.ska"
                ))
            ),
        ),
        (
            dependencies.join("first.ska"),
            include_str!("../../../tests/golden/modules/cases/cycle/modules/first.ska").to_owned(),
        ),
        (
            dependencies.join("second.ska"),
            include_str!("../../../tests/golden/modules/cases/cycle/modules/second.ska").to_owned(),
        ),
    ];
    for index in if variant == 0 { [0, 1, 2] } else { [2, 1, 0] } {
        write_source(&sources[index].0, &sources[index].1);
    }

    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::module_root(PathBuf::from("application-alias")),
            ProviderRootConfiguration::module_root(PathBuf::from("./dependencies")),
            ProviderRootConfiguration::module_root(PathBuf::from("application")),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(PathBuf::from("application")),
            ProviderRootConfiguration::module_root(PathBuf::from("dependencies/.")),
            ProviderRootConfiguration::module_root(PathBuf::from("./application-alias")),
        ]
    };
    let providers = normalize_provider_roots(&fixture.path, &configurations).unwrap();
    let entry = if variant == 0 {
        EntrySelector::Module("app".parse().unwrap())
    } else {
        EntrySelector::File(application_alias.join("app.ska"))
    };
    let graph = load_module_graph(&entry, &fixture.path, &providers).unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty());
    let hir = checked.hir.unwrap();
    let mir = run_mir_pipeline(lower_hir(&hir)).unwrap();
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&mir),
    )
    .unwrap();

    normalize_fixture_paths(
        &fixture.path,
        format!(
            "GRAPH\n{}DIAGNOSTICS\n{}RESOLVED\n{}HIR\n{}MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            render_diagnostics(graph.sources(), &resolved.diagnostics),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
            dump_mir(&mir),
            assembly,
        ),
    )
}

fn module_diagnostic_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("module-diagnostics", variant);
    let modules = fixture.path.join("modules");
    let modules_alias = fixture.path.join("modules-alias");
    let sources = [
        (
            modules.join("app.ska"),
            include_str!("../../../tests/golden/modules/cases/cycle_diagnostics/modules/app.ska"),
        ),
        (
            modules.join("left.ska"),
            include_str!("../../../tests/golden/modules/cases/cycle_diagnostics/modules/left.ska"),
        ),
        (
            modules.join("right.ska"),
            include_str!("../../../tests/golden/modules/cases/cycle_diagnostics/modules/right.ska"),
        ),
    ];
    for index in if variant == 0 { [0, 1, 2] } else { [2, 1, 0] } {
        write_source(&sources[index].0, sources[index].1);
    }
    link_directory(&modules, &modules_alias);
    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::module_root(modules_alias.clone()),
            ProviderRootConfiguration::module_root(modules.clone()),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(modules.clone()),
            ProviderRootConfiguration::module_root(modules_alias),
        ]
    };
    let providers = normalize_provider_roots(&fixture.path, &configurations).unwrap();
    let entry = EntrySelector::Module("app".parse().unwrap());
    let graph = load_module_graph(&entry, &fixture.path, &providers).unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.has_errors());

    normalize_fixture_paths(
        &fixture.path,
        render_diagnostics(graph.sources(), &resolved.diagnostics),
    )
}

fn generic_module_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("generic-module-products", variant);
    let modules = fixture.path.join("modules");
    let modules_alias = fixture.path.join("modules-alias");
    link_directory(&modules, &modules_alias);
    let sources = [
        (
            modules.join("app.ska"),
            "import model;\n\
             from wrapper import Envelope;\n\
             fn accept(\n\
               ref cache: model::Cache<model::Item>,\n\
               ref envelope: Envelope<model::Item>\n\
             ) -> unit {}\n\
             fn main() -> i64 {\n\
               model::Cache<model::Item>.count = 42;\n\
               return model::Cache<model::Item>.count;\n\
             }\n",
        ),
        (
            modules.join("model.ska"),
            "public class Item {\n\
               value: i64;\n\
               init(value: i64) { self.value = value; }\n\
               copy(ref source: Item) { self.value = source.value; }\n\
               assign(ref source: Item) { self.value = source.value; }\n\
             }\n\
             public class Cache<T> {\n\
               static cached: T?;\n\
               static count: i64 = 0;\n\
               value: T;\n\
               init(ref value: T) { self.value = value; }\n\
             }\n",
        ),
        (
            modules.join("wrapper.ska"),
            "import model;\n\
             public class Envelope<T> {\n\
               value: model::Cache<T>;\n\
               init(ref value: model::Cache<T>) { self.value = value; }\n\
             }\n",
        ),
    ];
    for index in if variant == 0 { [0, 1, 2] } else { [2, 0, 1] } {
        write_source(&sources[index].0, sources[index].1);
    }

    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::module_root(PathBuf::from("modules-alias")),
            ProviderRootConfiguration::module_root(PathBuf::from("modules")),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(PathBuf::from("modules")),
            ProviderRootConfiguration::module_root(PathBuf::from(
                "modules-alias/..//modules-alias",
            )),
        ]
    };
    let providers = normalize_provider_roots(&fixture.path, &configurations).unwrap();
    let entry = EntrySelector::Module("app".parse().unwrap());
    let graph = load_module_graph(&entry, &fixture.path, &providers).unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let preliminary = lower_preliminary_hir(&hir);
    let planned = plan_static_lifetimes(preliminary).unwrap();
    let planned_dump = dump_planned_mir(&planned);
    let final_mir = synthesize_static_lifecycle(verify_planned_mir(planned).unwrap());

    normalize_fixture_paths(
        &fixture.path,
        format!(
            "GRAPH\n{}DIAGNOSTICS\n{}RESOLVED\n{}HIR\n{}PLANNED MIR\n{}FINAL MIR\n{}",
            dump_module_graph(&graph),
            render_diagnostics(graph.sources(), &resolved.diagnostics),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
            planned_dump,
            dump_mir(&final_mir),
        ),
    )
}

fn generic_interface_module_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("generic-interface-products", variant);
    let modules = fixture.path.join("modules");
    let modules_alias = fixture.path.join("modules-alias");
    link_directory(&modules, &modules_alias);
    let sources = [
        (
            modules.join("app.ska"),
            "import api;\n\
             import model;\n\
             from api import Value as ImportedValue;\n\
             from model import Both as RenamedBoth;\n\
             class Value { init() {} }\n\
             fn inspect(ref value: Obj) -> i64 {\n\
               var exact: bool = value is ImportedValue<i64>;\n\
               var other: bool = value is api::Value<u64>;\n\
               return ((ImportedValue<i64>) value).value();\n\
             }\n\
             fn nested(ref value: api::Value<model::Box<i64>>) -> unit {}\n\
             fn cycles(ref left: api::Left<i64>, ref right: api::Right<i64>) -> unit {}\n\
             fn main() -> i64 {\n\
               var value: RenamedBoth = RenamedBoth(42);\n\
               var reader: model::Reader<RenamedBoth> = model::Reader<RenamedBoth>();\n\
               return reader.read(value) + value.name() - inspect(value) - 7;\n\
             }\n",
        ),
        (
            modules.join("api.ska"),
            "public interface Value<T> { fn value() -> T; }\n\
             public interface Named<T> { fn name() -> i64; }\n\
             public interface Left<T> { fn cross(ref value: Right<T>) -> T; }\n\
             public interface Right<T> { fn cross(ref value: Left<T>) -> T; }\n",
        ),
        (
            modules.join("model.ska"),
            "import api;\n\
             public class Box<T> { value: T; init(value: T) { self.value = value; } }\n\
             public class Both implements api::Value<i64>, api::Named<i64>, api::Named<u64> {\n\
               amount: i64;\n\
               init(amount: i64) { self.amount = amount; }\n\
               fn value() -> i64 { return self.amount; }\n\
               fn name() -> i64 { return 7; }\n\
             }\n\
             public class Reader<Source> where Source: api::Value<i64> {\n\
               init() {}\n\
               fn read(ref source: Source) -> i64 { return source.value(); }\n\
             }\n",
        ),
    ];
    for index in if variant == 0 { [0, 1, 2] } else { [2, 0, 1] } {
        write_source(&sources[index].0, sources[index].1);
    }

    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::module_root(PathBuf::from("modules-alias")),
            ProviderRootConfiguration::module_root(PathBuf::from("modules")),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(PathBuf::from("modules")),
            ProviderRootConfiguration::module_root(PathBuf::from(
                "modules-alias/..//modules-alias",
            )),
        ]
    };
    let providers = normalize_provider_roots(&fixture.path, &configurations).unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        &fixture.path,
        &providers,
    )
    .unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let preliminary = lower_preliminary_hir(&hir);
    let preliminary_dump = dump_preliminary_mir(&preliminary);
    let planned = plan_static_lifetimes(preliminary).unwrap();
    let planned_dump = dump_planned_mir(&planned);
    let mir = run_mir_pipeline(synthesize_static_lifecycle(
        verify_planned_mir(planned).unwrap(),
    ))
    .unwrap();
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::with_runtime_trace(&mir, graph.sources()),
    )
    .unwrap();

    normalize_fixture_paths(
        &fixture.path,
        format!(
            "GRAPH\n{}RESOLVED\n{}HIR\n{}PRELIMINARY MIR\n{}PLANNED MIR\n{}FINAL MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
            preliminary_dump,
            planned_dump,
            dump_mir(&mir),
            assembly,
        ),
    )
}

fn generic_operator_module_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("generic-operator-products", variant);
    let application = fixture.path.join("application");
    let standard_library = fixture.path.join("standard-library");
    let mut sources = vec![
        (
            application.join("app.ska"),
            "import model;\n\
             fn main() -> i64 {\n\
               var primitive: model::Adder<u64> = model::Adder<u64>();\n\
               var object_adder: model::Adder<model::Number> = model::Adder<model::Number>();\n\
               var left: model::Number = model::Number(17);\n\
               var right: model::Number = model::Number(25);\n\
               var result: model::Number = object_adder.add(left, right);\n\
               return (i64) primitive.add(17u, 25u) + result.value - 42;\n\
             }\n",
        ),
        (
            application.join("model.ska"),
            "from std::ops import OpAdd;\n\
             public class Number implements OpAdd<Number, Number> {\n\
               value: i64;\n\
               init(value: i64) { self.value = value; }\n\
               fn op_add(ref rhs: Number) -> Number { return Number(self.value + rhs.value); }\n\
             }\n\
             public class Adder<T> where T: OpAdd<T, T> {\n\
               init() {}\n\
               fn add(ref left: T, ref right: T) -> T { return left + right; }\n\
             }\n",
        ),
    ];
    sources.extend(
        canonical_standard_library_sources(&[])
            .into_iter()
            .map(|(relative, source)| (standard_library.join(relative), source)),
    );
    if variant != 0 {
        sources.reverse();
    }
    for (path, source) in sources {
        write_source(&path, source);
    }

    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::standard_library(standard_library.clone()),
            ProviderRootConfiguration::module_root(application.clone()),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(application.clone()),
            ProviderRootConfiguration::standard_library(standard_library.clone()),
        ]
    };
    let providers = normalize_provider_roots(&fixture.path, &configurations).unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        &fixture.path,
        &providers,
    )
    .unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let resolved_dump = dump_resolved(&resolved.program);
    assert!(
        resolved_dump.contains("primitive-intrinsic AddU64"),
        "{resolved_dump}"
    );
    assert!(resolved_dump.contains("class-witness"), "{resolved_dump}");

    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let hir_dump = dump_hir(&hir);
    assert!(hir_dump.contains("AddU64"), "{hir_dump}");
    assert!(hir_dump.contains("ObjectCall interface"), "{hir_dump}");
    assert!(!hir_dump.contains("OperatorSelection"), "{hir_dump}");

    let preliminary = lower_preliminary_hir(&hir);
    let preliminary_dump = dump_preliminary_mir(&preliminary);
    let planned = plan_static_lifetimes(preliminary).unwrap();
    let planned_dump = dump_planned_mir(&planned);
    let final_mir = run_mir_pipeline(synthesize_static_lifecycle(
        verify_planned_mir(planned).unwrap(),
    ))
    .unwrap();
    let final_dump = dump_mir(&final_mir);
    assert!(!final_dump.contains("Operator"), "{final_dump}");
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&final_mir),
    )
    .unwrap();
    assert!(!assembly.contains("ska_rt_operator"), "{assembly}");
    let public_symbols = assembly
        .lines()
        .filter(|line| line.starts_with(".globl "))
        .collect::<Vec<_>>();
    let pre_operator_public_symbol_baseline = [".globl main"];
    assert_eq!(
        public_symbols, pre_operator_public_symbol_baseline,
        "operator protocols must not change the public symbol surface:\n{assembly}"
    );
    let runtime_references = assembly
        .lines()
        .filter_map(|line| line.trim().strip_prefix("call ska_rt_"))
        .collect::<Vec<_>>();
    let pre_operator_runtime_reference_baseline = ["abi_v9"];
    assert_eq!(
        runtime_references, pre_operator_runtime_reference_baseline,
        "operator protocols must not add a runtime ABI service:\n{assembly}"
    );
    assert!(assembly.contains(".method.op_add."), "{assembly}");

    normalize_fixture_paths(
        &fixture.path,
        format!(
            "GRAPH\n{}RESOLVED\n{}HIR\n{}PRELIMINARY MIR\n{}PLANNED MIR\n{}FINAL MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            resolved_dump,
            hir_dump,
            preliminary_dump,
            planned_dump,
            final_dump,
            assembly,
        ),
    )
}

fn range_module_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("range-products", variant);
    let application = fixture.path.join("application");
    let standard_library = fixture.path.join("standard-library");
    let mut sources = vec![
        (
            application.join("app.ska"),
            "import model;\n\
             from std::range import Range;\n\
             fn main() -> i64 {\n\
               var primitive: model::Advance<u64> = model::Advance<u64>();\n\
               var objects: model::Advance<model::Value> = model::Advance<model::Value>();\n\
               var total: i64 = (i64) primitive.next(16u) + (i64) objects.next(model::Value(24u)).get();\n\
               for (value in Range<u64>(2u, 5u)) { total = total + (i64) value; }\n\
               for (value in Range<model::Value>(model::Value(6u), model::Value(8u))) { total = total + (i64) value.get(); }\n\
               for (value in 8u .. 10u) { total = total + (i64) value; }\n\
               for (value in model::Value(10u) .. model::Value(12u)) { total = total + (i64) value.get(); }\n\
               return total;\n\
             }\n",
        ),
        (
            application.join("model.ska"),
            "from std::ops import OpLess;\n\
             from std::range import Successor;\n\
             public class Value implements OpLess<Value>, Successor<Value> {\n\
               private value: u64;\n\
               init(value: u64) { self.value = value; }\n\
               fn op_less(ref rhs: Value) -> bool { return self.value < rhs.value; }\n\
               fn successor() -> Value { return Value(self.value + 1u); }\n\
               fn get() -> u64 { return self.value; }\n\
             }\n\
             public class Advance<T> where T: Successor<T> {\n\
               init() {}\n\
               fn next(value: T) -> T { return value.successor(); }\n\
             }\n",
        ),
    ];
    sources.extend(
        canonical_standard_library_sources(&[])
            .into_iter()
            .map(|(relative, source)| (standard_library.join(relative), source)),
    );
    if variant != 0 {
        sources.reverse();
    }
    for (path, source) in sources {
        write_source(&path, source);
    }

    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::standard_library(standard_library),
            ProviderRootConfiguration::module_root(application),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(application),
            ProviderRootConfiguration::standard_library(standard_library),
        ]
    };
    let providers = normalize_provider_roots(&fixture.path, &configurations).unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        &fixture.path,
        &providers,
    )
    .unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let resolved_dump = dump_resolved(&resolved.program);
    assert!(
        resolved_dump.contains("RangeConstruction template"),
        "{resolved_dump}"
    );
    assert!(resolved_dump.contains("AddOneU64"), "{resolved_dump}");
    assert!(
        resolved_dump.contains("ClosedBoundSelection 0 class-witness"),
        "{resolved_dump}"
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let hir_dump = dump_hir(&hir);
    assert!(hir_dump.contains("CanonicalRangeSyntax"));
    assert!(hir_dump.contains("PrimitiveRange endpoint=u64"));
    assert!(hir_dump.contains("Protocol interface="));
    let preliminary = lower_preliminary_hir(&hir);
    let preliminary_dump = dump_preliminary_mir(&preliminary);
    let planned = plan_static_lifetimes(preliminary).unwrap();
    let planned_dump = dump_planned_mir(&planned);
    let final_mir = run_mir_pipeline(synthesize_static_lifecycle(
        verify_planned_mir(planned).unwrap(),
    ))
    .unwrap();
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&final_mir),
    )
    .unwrap();
    assert!(!assembly.contains("skald_rt_range"), "{assembly}");

    normalize_fixture_paths(
        &fixture.path,
        format!(
            "GRAPH\n{}RESOLVED\n{}HIR\n{}PRELIMINARY MIR\n{}PLANNED MIR\n{}FINAL MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            resolved_dump,
            hir_dump,
            preliminary_dump,
            planned_dump,
            dump_mir(&final_mir),
            assembly,
        ),
    )
}

fn iteration_module_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("general-iteration-products", variant);
    let application = fixture.path.join("application");
    let standard_library = fixture.path.join("standard-library");
    let mut sources = vec![
        (
            application.join("app.ska"),
            "import model;\n\
             fn concrete(ref values: model::Values) -> i64 {\n\
               var sum: i64 = 0;\n\
               for (item in values) { sum = sum + item; }\n\
               return sum;\n\
             }\n\
             fn generic(ref scanner: model::Scanner<model::Values>, ref values: model::Values) -> i64 {\n\
               return scanner.scan(values);\n\
             }\n\
             fn main() -> i64 {\n\
               var values: model::Values = model::Values();\n\
               var scanner: model::Scanner<model::Values> = model::Scanner<model::Values>();\n\
               return concrete(values) + generic(scanner, values);\n\
             }\n",
        ),
        (
            application.join("model.ska"),
            "from std::iter import Iterable;\n\
             public class Values implements Iterable<i64, u64> {\n\
               init() {}\n\
               fn iter_state() -> u64 { return 0u; }\n\
               fn iter_next(mut ref state: u64) -> i64? { return none; }\n\
             }\n\
             public class Scanner<Source> where Source: Iterable<i64, u64> {\n\
               init() {}\n\
               fn scan(ref values: Source) -> i64 {\n\
                 var sum: i64 = 0;\n\
                 for (item in values) { sum = sum + item; }\n\
                 return sum;\n\
               }\n\
             }\n",
        ),
    ];
    sources.extend(
        canonical_standard_library_sources(&[])
            .into_iter()
            .map(|(relative, source)| (standard_library.join(relative), source)),
    );
    if variant != 0 {
        sources.reverse();
    }
    for (path, source) in sources {
        write_source(&path, source);
    }

    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::standard_library(standard_library),
            ProviderRootConfiguration::module_root(application),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(application),
            ProviderRootConfiguration::standard_library(standard_library),
        ]
    };
    let providers = normalize_provider_roots(&fixture.path, &configurations).unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        &fixture.path,
        &providers,
    )
    .unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let preliminary = lower_preliminary_hir(&hir);
    let preliminary_dump = dump_preliminary_mir(&preliminary);
    let planned = plan_static_lifetimes(preliminary).unwrap();
    let planned_dump = dump_planned_mir(&planned);
    let final_mir = run_mir_pipeline(synthesize_static_lifecycle(
        verify_planned_mir(planned).unwrap(),
    ))
    .unwrap();
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&final_mir),
    )
    .unwrap();

    normalize_fixture_paths(
        &fixture.path,
        format!(
            "GRAPH\n{}RESOLVED\n{}HIR\n{}PRELIMINARY MIR\n{}PLANNED MIR\n{}FINAL MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
            preliminary_dump,
            planned_dump,
            dump_mir(&final_mir),
            assembly,
        ),
    )
}

fn iteration_diagnostic_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("general-iteration-diagnostics", variant);
    let application = fixture.path.join("application");
    let standard_library = fixture.path.join("standard-library");
    let claims = if variant == 0 {
        "Iterable<i64, u64>, Iterable<f64, i64>"
    } else {
        "Iterable<f64, i64>, Iterable<i64, u64>"
    };
    let app = format!(
        "from std::iter import Iterable;\n\
         class Both implements {claims} {{ init() {{}} }}\n\
         fn scan(ref values: Both) -> unit {{ for (item in values) {{}} }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    );
    write_source(&application.join("app.ska"), &app);
    for (relative, source) in canonical_standard_library_sources(&[]) {
        write_source(&standard_library.join(relative), source);
    }
    let providers = normalize_provider_roots(
        &fixture.path,
        &[
            ProviderRootConfiguration::module_root(application),
            ProviderRootConfiguration::standard_library(standard_library),
        ],
    )
    .unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        &fixture.path,
        &providers,
    )
    .unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.has_errors());
    let rendered = render_diagnostics(graph.sources(), &resolved.diagnostics).replace(
        &format!("class Both implements {claims} {{ init() {{}} }}"),
        "class Both implements <first-claim>, <second-claim> { init() {} }",
    );
    normalize_fixture_paths(&fixture.path, rendered)
}

fn generic_interface_diagnostic_dump() -> String {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add(
        "generic-interface-specialization.ska",
        "interface Chain<T> { fn next() -> Chain<T>; }\n\
         interface Expand<T> { fn next() -> Expand<T[]>; }\n\
         fn first(ref chain: Chain<(shared Item)?>, ref failed: Expand<i64>) -> unit {}\n\
         fn second(ref chain: Chain<shared? Item>, ref failed: Expand<i64>) -> unit {}\n\
         class Item {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);

    format!(
        "TOKENS\n{}AST\n{}RESOLVED\n{}DIAGNOSTICS\n{}",
        dump_tokens(source, &lexed.tokens),
        dump_ast(&parsed.ast),
        dump_resolved(&resolved.program),
        render_diagnostics(&sources, &resolved.diagnostics),
    )
}

fn source_body_after_imports(source: &str) -> &str {
    source
        .split_once("\n\n")
        .expect("a reusable module fixture must separate imports from its body")
        .1
}

fn write_source(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[cfg(unix)]
fn link_directory(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(windows)]
fn link_directory(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_dir(target, link).unwrap();
}

fn normalize_fixture_paths(fixture: &Path, output: String) -> String {
    let path_normalized = output.replace(fixture.to_str().unwrap(), "<fixture>");
    path_normalized
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("display ") {
                format!(
                    "{}display <spelling>",
                    &line[..line.len() - line.trim_start().len()]
                )
            } else {
                normalize_spans(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn normalize_spans(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut output = String::with_capacity(line.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'@' {
            let start = index;
            index += 1;
            let first_digits = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index > first_digits && bytes.get(index..index + 2) == Some(b"..") {
                index += 2;
                let second_digits = index;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                if index > second_digits {
                    output.push_str("@<span>");
                    continue;
                }
            }
            output.push_str(&line[start..index]);
        } else {
            let character = line[index..].chars().next().unwrap();
            output.push(character);
            index += character.len_utf8();
        }
    }
    output
}

fn object_phase_dump() -> String {
    let text = concat!(
        "class Box { value: i64; init(value: i64) { self.value = value; } ",
        "copy(ref other: Box) { self.value = other.value; } ",
        "assign(ref other: Box) { self.value = other.value; } ",
        "mut fn set(value: i64) -> unit { self.value = value; } ",
        "fn get() -> i64 { return self.value; } destroy {} }\n",
        "class Snapshot { box: Box; init(ref source: Box) { self.box = Box(read(source)); } ",
        "destroy {} }\n",
        "fn read(ref value: Box) -> i64 { return value.get(); }\n",
        "fn write(mut ref value: Box, amount: i64) -> unit { value.set(amount); }\n",
        "fn forward(mut ref value: Box) -> unit { write(value, read(value) + 1); }\n",
        "fn produce(value: i64) -> Box { return Box(value); }\n",
        "fn choose(ref source: Box, first: bool) -> Box { ",
        "if (first) { return source; } else { return (Box(source.get())); } }\n",
        "fn consume(value: Box, ref alias: Box) -> i64 { ",
        "value = produce(alias.get()); return value.get(); }\n",
        "fn main() -> i64 { var value: Box = Box(1); forward(value); ",
        "var grouped: Box = (Box(2)); grouped = produce(read(value)); ",
        "var copied: Box = value; var result: Box = choose(copied, false); ",
        "var snapshot: Snapshot = Snapshot(result); ",
        "return consume(produce(snapshot.box.get()), grouped); }\n",
    );
    complete_phase_dump(text)
}

fn polymorphism_phase_dump() -> String {
    complete_golden_phase_dump(include_str!(
        "../../../tests/golden/polymorphism/polymorphism.ska"
    ))
}

fn produced_alias_phase_dump() -> String {
    complete_golden_phase_dump(include_str!(
        "../../../tests/golden/aliases/produced_alias_arguments.ska"
    ))
}

fn produced_receiver_phase_dump() -> String {
    complete_phase_dump(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } ",
        "fn next(amount: i64) -> Item { return Item(self.value + amount); } ",
        "fn read() -> i64 { return self.value; } }\n",
        "fn produce(value: i64) -> Item { return Item(value); }\n",
        "fn main() -> i64 { return produce(40).next(2).read(); }\n",
    ))
}

fn produced_field_phase_dump() -> String {
    complete_phase_dump(include_str!(
        "../../../tests/golden/produced_fields/primitive_and_inline_consumers.ska"
    ))
}

fn shared_ownership_phase_dump() -> String {
    complete_golden_phase_dump(include_str!(
        "../../../tests/golden/shared_ownership/shared_copy_allocation.ska"
    ))
}

fn optional_phase_dump() -> String {
    format!(
        "BASELINE\n{}COMPOSITIONAL\n{}SHARED OPTIONAL BOXES\n{}",
        complete_golden_phase_dump(include_str!(
            "../../../tests/golden/optionals/optional_shared_profile.ska"
        )),
        complete_phase_dump(concat!(
            "interface Forward { fn forward(values: i64[]?) -> i64[]?; }\n",
            "class Holder implements Forward {\n",
            "  static current: i64[]?; values: i64[]?;\n",
            "  init(values: i64[]?) { self.values = values; }\n",
            "  fn forward(values: i64[]?) -> i64[]? { return values; }\n",
            "}\n",
            "fn mutate(mut ref values: i64[]) -> unit { values[0] = 42; }\n",
            "fn inspect(deep: i64?????, values: i64[]?[]) -> unit {}\n",
            "fn main() -> i64 {\n",
            "  var holder: Holder = Holder(i64[]{1});\n",
            "  var nested: i64[]?[] = i64[]?[]{none, holder.values};\n",
            "  mutate(holder.values!); Holder.current = nested[1]; return holder.values![0];\n",
            "}\n",
        )),
        planned_lifecycle_phase_dump(include_str!(
            "../../../tests/golden/optionals/optional_boxes_profile.ska"
        ))
    )
}

fn array_phase_dump() -> String {
    complete_golden_phase_dump(include_str!("../../../tests/golden/arrays/array_views.ska"))
}

fn array_element_list_phase_dump() -> String {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add(
        "array-element-lists.ska",
        concat!(
            "class Item { value: i64; init(value: i64) { self.value = value; } ",
            "copy(ref other: Item) { self.value = other.value; } }\n",
            "class Holder { values: i64[]; init(first: i64, values: i64[]) { self.values = values; } }\n",
            "fn consume(values: i64[]) -> i64 { return values[0]; }\n",
            "fn produce() -> i64[] { return i64[]{8, 9}; }\n",
            "fn main() -> i64 {\n",
            "  var primitives: i64[] = i64[]{};\n",
            "  primitives = i64[]{1, 2};\n",
            "  var objects: Item[] = Item[]{Item(1), Item(2)};\n",
            "  var optional_values: i64?[] = i64?[]{none, 3};\n",
            "  var optional_objects: Item?[] = Item?[]{none, Item(4)};\n",
            "  var rows: i64[][] = i64[][]{i64[]{1, 2}, i64[]{3}};\n",
            "  var owners: (shared Item)[] = (shared Item)[]{new Item(5)};\n",
            "  var optional_owners: (shared? Item)[] = (shared? Item)[]{none, new Item(6)};\n",
            "  var shared_outer: shared i64[] = new i64[]{10, 11};\n",
            "  var holder: Holder = Holder(12, i64[]{6, 7});\n",
            "  return consume(produce()) + i64[]{4, 5}[0] + shared_outer->[0];\n",
            "}\n",
        ),
    );
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let mir = lower_hir(&hir);

    format!(
        "TOKENS\n{}AST\n{}RESOLVED\n{}HIR\n{}MIR\n{}",
        dump_tokens(source, &lexed.tokens),
        dump_ast(&parsed.ast),
        dump_resolved(&resolved.program),
        dump_hir(&hir),
        dump_mir(&mir),
    )
}

fn static_field_phase_dump() -> String {
    complete_phase_dump(concat!(
        "fn increment(mut ref value: i64) -> unit { value = value + 1; }\n",
        "class Base { static count: i64; static maybe: i64?; static values: i64[]; init() {} }\n",
        "class Derived extends Base { static owner: shared? Item; init() { super(); } }\n",
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn main() -> i64 { Derived.count = 40; increment(Base.count); ",
        "Derived.maybe = Base.count; Derived.values = i64[](1u); ",
        "Derived.values[0] = Derived.maybe!; Derived.owner = new Item(1); ",
        "return Base.values[0] + Derived.owner!->value; }\n",
    ))
}

fn static_initializer_lifecycle_phase_dump() -> String {
    planned_lifecycle_phase_dump(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class State {\n",
        "  static count: i64 = combine(20, 22);\n",
        "  static item: Item = Item(State.count);\n",
        "  static owner: shared Item = new Item(1);\n",
        "  static owner_copy: shared Item = State.owner;\n",
        "  static values: i64[] = i64[]{1, 2};\n",
        "  init() {}\n",
        "}\n",
        "fn combine(left: i64, right: i64) -> i64 { return left + right; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
}

fn static_lifetime_cycle_diagnostic_dump() -> String {
    let text = concat!(
        "fn read_left() -> i64 { return State.left; }\n",
        "fn read_right() -> i64 { return State.right; }\n",
        "class State {\n",
        "  static left: i64 = read_right();\n",
        "  static right: i64 = read_left();\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("static-lifetime-diagnostics.ska", text);
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let preliminary = lower_preliminary_hir(&checked.hir.unwrap());
    let diagnostics = plan_static_lifetimes(preliminary)
        .unwrap_err()
        .into_diagnostics();

    render_diagnostics(&sources, &diagnostics)
}

fn static_field_diagnostic_dump() -> String {
    type_error_phase_dump(
        "static-field-diagnostics.ska",
        concat!(
            "class Item { init() {} }\n",
            "class Invalid {\n",
            "  private static seed: i64 = 40;\n",
            "  static answer: i64 = add(Invalid.seed, 2);\n",
            "  static item: Item;\n",
            "  static owner: shared Item;\n",
            "  init() {}\n",
            "}\n",
            "fn add(left: i64, right: i64) -> i64 { return left + right; }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
    )
}

fn static_field_module_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("static-field-modules", variant);
    let modules = fixture.path.join("modules");
    let modules_alias = fixture.path.join("modules-alias");
    let sources = [
        (
            modules.join("app.ska"),
            concat!(
                "import state;\n",
                "fn main() -> i64 { state::Derived.count = 42; ",
                "return state::Base.count; }\n",
            ),
        ),
        (
            modules.join("state.ska"),
            concat!(
                "import helper;\n",
                "public class Base { static count: i64; init() {} }\n",
                "public class Derived extends Base { init() { super(); } }\n",
                "public fn helper_value() -> i64 { return helper::value(); }\n",
            ),
        ),
        (
            modules.join("helper.ska"),
            concat!(
                "import state;\n",
                "public fn value() -> i64 { return state::Base.count; }\n",
            ),
        ),
    ];
    for index in if variant == 0 { [0, 1, 2] } else { [2, 1, 0] } {
        write_source(&sources[index].0, sources[index].1);
    }
    link_directory(&modules, &modules_alias);
    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::module_root(modules_alias),
            ProviderRootConfiguration::module_root(modules),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(modules),
            ProviderRootConfiguration::module_root(modules_alias),
        ]
    };
    let providers = normalize_provider_roots(&fixture.path, &configurations).unwrap();
    let entry = if variant == 0 {
        EntrySelector::Module("app".parse().unwrap())
    } else {
        EntrySelector::File(fixture.path.join("modules-alias/app.ska"))
    };
    let graph = load_module_graph(&entry, &fixture.path, &providers).unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty());
    let hir = checked.hir.unwrap();
    let mir = run_mir_pipeline(lower_hir(&hir)).unwrap();
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&mir),
    )
    .unwrap();

    normalize_fixture_paths(
        &fixture.path,
        format!(
            "GRAPH\n{}RESOLVED\n{}HIR\n{}MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
            dump_mir(&mir),
            assembly,
        ),
    )
}

fn integer_operation_phase_dump() -> String {
    complete_golden_phase_dump(include_str!(
        "../../../tests/golden/primitives/integer_string_range_guards.ska"
    ))
}

fn integer_bitwise_and_shift_phase_dump() -> String {
    complete_phase_dump(concat!(
        "class Bits { value: u8; count: u64; ",
        "init(value: u8, count: u64) { self.value = value; self.count = count; } }\n",
        "class Trace { value: u64; init(value: u64) { self.value = value; } ",
        "fn read() -> u64 { return self.value; } destroy {} }\n",
        "fn make(value: u64) -> shared Trace { return new Trace(value); }\n",
        "fn mix(ref bits: Bits, optional: u8?, values: u8[]) -> bool { ",
        "return (((~bits.value + 0x01u8 << bits.count) >> 1u) & values[0] ",
        "^ optional! | 0x01u8) == 0x07u8 && true; }\n",
        "fn cleanup() -> u64 { return make(0x10u)->read() >> make(2u)->read(); }\n",
        "fn main() -> i64 { var bits: Bits = Bits(0x03u8, 2u); ",
        "var optional: u8? = 0x04u8; var values: u8[] = u8[](1u); values[0] = 0x07u8; ",
        "if (mix(bits, optional, values) || cleanup() == 0x04u) { return 0; } return 1; }\n",
    ))
}

fn integer_bitwise_and_shift_diagnostic_dump() -> String {
    type_error_phase_dump(
        "integer-bitwise-shift-diagnostics.ska",
        concat!(
            "class Item { init() {} }\n",
            "fn invalid(flag: bool, count: i64, owner: shared Item) -> i64 {\n",
            "  var complement: i64 = ~flag;\n",
            "  var bitwise: i64 = 1 | flag;\n",
            "  var shifted: i64 = 1 << count;\n",
            "  var owner_count: i64 = 1 >> owner;\n",
            "  return complement + bitwise + shifted + owner_count;\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ),
    )
}

fn integer_division_phase_dump() -> String {
    complete_golden_phase_dump(include_str!(
        "../../../tests/golden/operators/integer_division_operators.ska"
    ))
}

fn integer_division_diagnostic_dump() -> String {
    type_error_phase_dump(
        "integer-division-diagnostics.ska",
        include_str!("../../../tests/golden/operators/integer_division_operator_types.ska"),
    )
}

fn floating_division_phase_dump() -> String {
    complete_golden_phase_dump(include_str!(
        "../../../tests/golden/operators/floating_division.ska"
    ))
}

fn floating_division_diagnostic_dump() -> String {
    type_error_phase_dump(
        "floating-division-diagnostics.ska",
        concat!(
            "class Item { init() {} }\n",
            "fn invalid(left: f64, integer: i64, flag: bool, owner: shared Item) -> f64 {\n",
            "  var mixed: f64 = left / integer;\n",
            "  var boolean: f64 = left / flag;\n",
            "  return left / owner;\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ),
    )
}

fn floating_comparison_phase_dump() -> String {
    complete_golden_phase_dump(include_str!(
        "../../../tests/golden/operators/floating_comparisons.ska"
    ))
}

fn floating_comparison_diagnostic_dump() -> String {
    type_error_phase_dump(
        "floating-comparison-diagnostics.ska",
        concat!(
            "fn invalid(left: f64, integer: i64, flag: bool, optional: f64?) -> bool {\n",
            "  var mixed: bool = left < integer;\n",
            "  var boolean: bool = left == flag;\n",
            "  return left >= optional;\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ),
    )
}

fn primitive_operator_profile_phase_dump() -> String {
    complete_golden_phase_dump(include_str!(
        "../../../tests/golden/operators/primitive_operator_profile.ska"
    ))
}

fn primitive_cast_phase_dump() -> String {
    complete_golden_phase_dump(include_str!(
        "../../../tests/golden/primitives/primitive_cast_matrix.ska"
    ))
}

fn primitive_cast_diagnostic_dump() -> String {
    type_error_phase_dump(
        "primitive-cast-diagnostics.ska",
        concat!(
            "fn invalid(values: i64[]) -> u64 { return (u64) values; }\n",
            "fn implicit(value: f64) -> i64 { return value; }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
    )
}

fn eager_boolean_phase_dump() -> String {
    complete_golden_phase_dump(include_str!(
        "../../../tests/golden/operators/eager_boolean_operators.ska"
    ))
}

fn eager_boolean_diagnostic_dump() -> String {
    type_error_phase_dump(
        "eager-boolean-diagnostics.ska",
        include_str!("../../../tests/golden/operators/eager_boolean_operator_types.ska"),
    )
}

fn short_circuit_source_phase_dump() -> String {
    complete_phase_dump(concat!(
        "fn selected(a: bool, b: bool, c: bool) -> bool { return (a || b) && !c; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
}

fn private_initializer_phase_dump() -> String {
    complete_phase_dump(concat!(
        "class Secret { value: i64; init(value: i64) { self.value = value; } ",
        "private init(flag: bool) { self.value = 42; } ",
        "static fn make(flag: bool) -> Secret { return Secret(flag); } ",
        "fn reveal() -> i64 { return self.value; } }\n",
        "fn main() -> i64 { var public: Secret = Secret(1); ",
        "var private: Secret = Secret.make(true); return public.reveal() + private.reveal(); }\n",
    ))
}

fn private_initializer_diagnostic_dump() -> String {
    let text = concat!(
        "interface Named {}\n",
        "class Key implements Named { init() {} }\n",
        "class Choice { init(ref value: Obj) {} private init(ref value: Named) {} }\n",
        "fn main() -> i64 { var key: Key = Key(); ",
        "var choice: Choice = Choice(key); return 0; }\n",
    );
    type_error_phase_dump("private-initializer-diagnostic.ska", text)
}

fn private_cell_phase_dump() -> String {
    complete_golden_phase_dump(include_str!(
        "../../../tests/golden/objects/private_cell_dispatch_composition.ska"
    ))
}

fn private_cell_diagnostic_dump() -> String {
    type_error_phase_dump(
        "private-cell-diagnostics.ska",
        include_str!("../../../tests/golden/objects/private_cell_exclusions.ska"),
    )
}

fn final_field_phase_dump() -> String {
    complete_golden_phase_dump(include_str!(
        "../../../tests/golden/objects/final_field_composition.ska"
    ))
}

fn final_field_diagnostic_dump() -> String {
    type_error_phase_dump(
        "final-field-diagnostics.ska",
        include_str!("../../../tests/golden/objects/final_field_alias_rebinding.ska"),
    )
}

fn type_error_phase_dump(name: &str, text: &str) -> String {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add(name, text);
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.hir.is_none());

    format!(
        "AST\n{}RESOLVED\n{}DIAGNOSTICS\n{}",
        dump_ast(&parsed.ast),
        dump_resolved(&resolved.program),
        render_diagnostics(&sources, &checked.diagnostics),
    )
}

fn string_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("string-products", variant);
    let application = fixture.path.join("application");
    let standard_library = fixture.path.join("standard-library");
    let mut sources = vec![(
        application.join("app.ska"),
        include_str!("../../../tests/golden/primitive_strings/string_values.ska"),
    )];
    sources.extend(
        canonical_standard_library_sources(&[])
            .into_iter()
            .map(|(relative, source)| (standard_library.join(relative), source)),
    );
    if variant != 0 {
        sources.reverse();
    }
    for (path, source) in sources {
        write_source(&path, source);
    }
    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::standard_library(standard_library),
            ProviderRootConfiguration::module_root(application),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(application),
            ProviderRootConfiguration::standard_library(standard_library),
        ]
    };
    let providers = normalize_provider_roots(&fixture.path, &configurations).unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        &fixture.path,
        &providers,
    )
    .unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty());
    let hir = checked.hir.unwrap();
    let mir = lower_final_hir(&hir);
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&mir),
    )
    .unwrap();

    normalize_fixture_paths(
        &fixture.path,
        format!(
            "GRAPH\n{}DIAGNOSTICS\n{}RESOLVED\n{}HIR\n{}MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            render_diagnostics(graph.sources(), &resolved.diagnostics),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
            dump_mir(&mir),
            assembly,
        ),
    )
}

fn io_phase_dump(variant: usize, malformed: bool) -> String {
    let fixture = ModuleFixture::new("io-products", variant);
    let application = fixture.path.join("application");
    let standard_library = fixture.path.join("standard-library");
    let mut io_source = CANONICAL_IO_SOURCE.to_owned();
    if malformed {
        io_source = io_source.replace("intrinsic fn _io_close", "public intrinsic fn _io_close");
    }
    let mut sources = vec![(
        application.join("app.ska"),
        concat!(
            "import std::io;\n",
            "from std::str import Str;\n",
            "fn main() -> i64 {\n",
            "  var path: Str = \"input.bin\";\n",
            "  var stdin: Str = std::io::read_stdin();\n",
            "  var file: Str = std::io::read_file(path);\n",
            "  std::io::write_stdout(stdin);\n",
            "  std::io::write_stderr(file);\n",
            "  return 0;\n",
            "}\n",
        ),
    )];
    sources.extend(
        canonical_standard_library_sources(&[("std/io.ska", io_source.as_str())])
            .into_iter()
            .map(|(relative, source)| (standard_library.join(relative), source)),
    );
    if variant != 0 {
        sources.reverse();
    }
    for (path, source) in sources {
        write_source(&path, source);
    }
    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::standard_library(standard_library),
            ProviderRootConfiguration::module_root(application),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(application),
            ProviderRootConfiguration::standard_library(standard_library),
        ]
    };
    let providers = normalize_provider_roots(&fixture.path, &configurations).unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        &fixture.path,
        &providers,
    )
    .unwrap();
    let resolved = resolve_module_graph(&graph);

    let phases = if malformed {
        assert!(resolved.diagnostics.has_errors());
        format!(
            "GRAPH\n{}DIAGNOSTICS\n{}RESOLVED\n{}",
            dump_module_graph(&graph),
            render_diagnostics(graph.sources(), &resolved.diagnostics),
            dump_resolved(&resolved.program),
        )
    } else {
        assert!(
            resolved.diagnostics.is_empty(),
            "{:?}",
            resolved.diagnostics
        );
        let checked = type_check(&resolved.program);
        assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
        let hir = checked.hir.unwrap();
        let mir = lower_final_hir(&hir);
        let assembly = emit_assembly(
            Target::X86_64SysV,
            BackendInput::without_runtime_trace(&mir),
        )
        .unwrap();
        format!(
            "GRAPH\n{}RESOLVED\n{}HIR\n{}MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
            dump_mir(&mir),
            assembly,
        )
    };
    normalize_fixture_paths(&fixture.path, phases)
}

fn string_diagnostic_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("string-diagnostics", variant);
    let application = fixture.path.join("application");
    let standard_library = fixture.path.join("standard-library");
    let malformed_string = concat!(
        "public class Str {\n",
        "  private _storage: shared u64[];\n",
        "  private _start: u8;\n",
        "  private _length: i64;\n",
        "  private _extra: u64;\n",
        "  init() {\n",
        "    self._storage = new u64[]();\n",
        "    self._start = 0u8;\n",
        "    self._length = 0;\n",
        "    self._extra = 0u;\n",
        "  }\n",
        "}\n",
    );
    let mut sources = vec![
        (
            application.join("app.ska"),
            "import feature;\nfn main() -> i64 { \"app\"; return 0; }\n",
        ),
        (
            application.join("feature.ska"),
            "public fn value() -> unit { \"feature\"; }\n",
        ),
    ];
    sources.extend(
        canonical_standard_library_sources(&[("std/str.ska", malformed_string)])
            .into_iter()
            .map(|(relative, source)| (standard_library.join(relative), source)),
    );
    if variant != 0 {
        sources.reverse();
    }
    for (path, source) in sources {
        write_source(&path, source);
    }

    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::standard_library(standard_library),
            ProviderRootConfiguration::module_root(application),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(application),
            ProviderRootConfiguration::standard_library(standard_library),
        ]
    };
    let providers = normalize_provider_roots(&fixture.path, &configurations).unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        &fixture.path,
        &providers,
    )
    .unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.has_errors());

    normalize_fixture_paths(
        &fixture.path,
        format!(
            "GRAPH\n{}DIAGNOSTICS\n{}",
            dump_module_graph(&graph),
            render_diagnostics(graph.sources(), &resolved.diagnostics),
        ),
    )
}

fn complete_phase_dump(text: &str) -> String {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("determinism.ska", text);
    let source = sources.get(source_id).unwrap();

    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty());
    let hir = checked.hir.unwrap();
    let mir = run_mir_pipeline(lower_hir(&hir)).unwrap();
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&mir),
    )
    .unwrap();

    format!(
        "TOKENS\n{}AST\n{}RESOLVED\n{}HIR\n{}MIR\n{}ASSEMBLY\n{}",
        dump_tokens(source, &lexed.tokens),
        dump_ast(&parsed.ast),
        dump_resolved(&resolved.program),
        dump_hir(&hir),
        dump_mir(&mir),
        assembly,
    )
}

fn mir_pipeline_checkpoint_dump() -> String {
    let text = "fn helper(value: i64) -> i64 { return value + 1; }\n\
                fn main() -> i64 { return helper(41); }\n";
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("checkpoint-determinism.ska", text);
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty());

    let mut checkpoints = Vec::new();
    let mut inspector = |checkpoint: MirPipelineCheckpoint<'_>| {
        checkpoints.push((
            checkpoint.label().to_string(),
            dump_mir(checkpoint.verified()),
        ));
    };
    run_mir_pipeline_inspected(lower_hir(&checked.hir.unwrap()), &mut inspector).unwrap();

    checkpoints
        .into_iter()
        .map(|(label, dump)| format!("CHECKPOINT {label}\n{dump}"))
        .collect()
}

fn function_value_composition_phase_dump() -> String {
    let text = include_str!("../../../tests/golden/function_values/composition.ska");
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("function-value-composition-determinism.ska", text);
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let preliminary = lower_preliminary_hir(&hir);
    let preliminary_dump = dump_preliminary_mir(&preliminary);
    let planned = plan_static_lifetimes(preliminary).unwrap();
    let planned_dump = dump_planned_mir(&planned);
    let mir = run_mir_pipeline(synthesize_static_lifecycle(
        verify_planned_mir(planned).unwrap(),
    ))
    .unwrap();
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::with_runtime_trace(&mir, &sources),
    )
    .unwrap();

    format!(
        "TOKENS\n{}AST\n{}RESOLVED\n{}HIR\n{}PRELIMINARY MIR\n{}PLANNED MIR\n{}MIR\n{}ASSEMBLY\n{}",
        dump_tokens(source, &lexed.tokens),
        dump_ast(&parsed.ast),
        dump_resolved(&resolved.program),
        dump_hir(&hir),
        preliminary_dump,
        planned_dump,
        dump_mir(&mir),
        assembly,
    )
}

fn lower_final_hir(hir: &HirProgram) -> VerifiedFinalMirProgram {
    let preliminary = lower_preliminary_hir(hir);
    let planned = plan_static_lifetimes(preliminary).unwrap();
    let mir = synthesize_static_lifecycle(verify_planned_mir(planned).unwrap());
    run_mir_pipeline(mir).unwrap()
}

fn planned_lifecycle_phase_dump(text: &str) -> String {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("typed-hir-determinism.ska", text);
    let source = sources.get(source_id).unwrap();

    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let preliminary = lower_preliminary_hir(&hir);
    let planned = plan_static_lifetimes(preliminary).unwrap();
    let planned_dump = dump_planned_mir(&planned);
    let final_mir = run_mir_pipeline(synthesize_static_lifecycle(
        verify_planned_mir(planned).unwrap(),
    ))
    .unwrap();
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&final_mir),
    )
    .unwrap();

    format!(
        "TOKENS\n{}AST\n{}RESOLVED\n{}HIR\n{}PLANNED MIR\n{}FINAL MIR\n{}ASSEMBLY\n{}",
        dump_tokens(source, &lexed.tokens),
        dump_ast(&parsed.ast),
        dump_resolved(&resolved.program),
        dump_hir(&hir),
        planned_dump,
        dump_mir(&final_mir),
        assembly,
    )
}

fn complete_golden_phase_dump(text: &str) -> String {
    let mut source = text
        .lines()
        .filter(|line| line != &"import std::io;")
        .collect::<Vec<_>>()
        .join("\n");
    let mut declarations = String::new();
    for (public_name, recorder_name, parameter_type) in [
        ("std::io::println_bool", "test_record_bool", "bool"),
        ("std::io::println_i64", "test_record_i64", "i64"),
        ("std::io::println_u64", "test_record_u64", "u64"),
        ("std::io::println_u8", "test_record_u8", "u8"),
        ("std::io::println_f64", "test_record_f64", "f64"),
    ] {
        if source.contains(public_name) {
            declarations.push_str(&format!(
                "extern fn {recorder_name}(value: {parameter_type}) -> unit;\n"
            ));
            source = source.replace(public_name, recorder_name);
        }
    }
    replace_standard_test_assertions(&mut source, &mut declarations);
    declarations.push_str(&source);
    complete_phase_dump(&declarations)
}

fn replace_standard_test_assertions(source: &mut String, declarations: &mut String) {
    let imports_assertions = source
        .lines()
        .any(|line| line == "import std::test;" || line.starts_with("from std::test import "));
    if !imports_assertions {
        return;
    }

    *source = source
        .lines()
        .filter(|line| line != &"import std::test;" && !line.starts_with("from std::test import "))
        .collect::<Vec<_>>()
        .join("\n");

    for (name, parameters) in [
        ("assert_eq_f64", "left: f64, right: f64"),
        ("assert_eq_i64", "left: i64, right: i64"),
        ("assert_eq_u64", "left: u64, right: u64"),
        ("assert_eq_u8", "left: u8, right: u8"),
        ("assert_false", "value: bool"),
        ("assert_true", "value: bool"),
    ] {
        *source = source.replace(&format!("std::test::{name}"), name);
        if source.contains(&format!("{name}(")) {
            declarations.push_str(&format!("extern fn {name}({parameters}) -> unit;\n"));
        }
    }
}

struct TemporaryArtifacts {
    first: PathBuf,
    second: PathBuf,
}

struct ModuleFixture {
    path: PathBuf,
}

impl ModuleFixture {
    fn new(label: &str, variant: usize) -> Self {
        let path = env::temp_dir().join(format!("skald-{label}-{}-{variant}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self { path }
    }
}

impl Drop for ModuleFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl TemporaryArtifacts {
    fn new(label: &str) -> Self {
        let stem = format!("skald-{label}-determinism-{}", std::process::id());
        let directory = env::temp_dir();
        Self {
            first: directory.join(format!("{stem}-first.txt")),
            second: directory.join(format!("{stem}-second.txt")),
        }
    }
}

impl Drop for TemporaryArtifacts {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.first);
        let _ = fs::remove_file(&self.second);
    }
}
