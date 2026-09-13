//! Cross-process determinism coverage for representative complete pipelines.

use skald_compiler::{
    backend::{emit_assembly, BackendInput, Target},
    diagnostics::render_diagnostics,
    driver::EntrySelector,
    hir::dump_hir,
    lexer::lex,
    mir::{dump_mir, lower_hir},
    module::{
        dump_module_graph, load_module_graph, normalize_provider_roots, ProviderRootConfiguration,
    },
    passes::{run_mir_pipeline_inspected, MirPipelineCheckpoint},
    resolve::{dump_resolved, resolve, resolve_module_graph},
    source::SourceDatabase,
    syntax::parse,
    typeck::type_check,
};

#[path = "pipeline_determinism/mod.rs"]
mod determinism;
#[path = "../test_support/standard_library.rs"]
mod standard_library;
mod support;

use determinism::{
    array_element_list_phase_dump, array_phase_dump, assert_cross_process_determinism,
    assert_cross_process_variants, final_field_diagnostic_dump, final_field_phase_dump,
    function_value_composition_phase_dump, generic_interface_diagnostic_dump,
    generic_interface_module_phase_dump, generic_module_phase_dump,
    generic_operator_module_phase_dump, imported_unused_static_phase_dump,
    indexed_array_frontend_phase_dump, iteration_diagnostic_dump, iteration_module_phase_dump,
    lower_final_hir, module_diagnostic_dump, module_phase_dump, normalize_fixture_paths,
    object_phase_dump, optional_phase_dump, polymorphism_phase_dump, private_cell_diagnostic_dump,
    private_cell_phase_dump, private_initializer_diagnostic_dump, private_initializer_phase_dump,
    produced_alias_phase_dump, produced_field_phase_dump, produced_receiver_phase_dump,
    range_module_phase_dump, range_syntax_diagnostic_dump, shared_ownership_phase_dump,
    single_source_full_phase_dump, single_source_type_error_dump, static_field_diagnostic_dump,
    static_field_module_phase_dump, static_field_phase_dump,
    static_initializer_lifecycle_phase_dump, static_lifetime_cycle_diagnostic_dump, write_source,
    ModuleFixture, StandardLibraryInput,
};
use standard_library::{canonical_standard_library_sources, CANONICAL_IO_SOURCE};

const INTEGER_OPERATION_TEST_NAME: &str =
    "integer_operation_phase_products_are_deterministic_across_processes";
const INTEGER_BITWISE_SHIFT_TEST_NAME: &str =
    "integer_bitwise_and_shift_phase_products_are_deterministic_across_processes";
const INTEGER_BITWISE_SHIFT_DIAGNOSTIC_TEST_NAME: &str =
    "integer_bitwise_and_shift_diagnostics_are_deterministic_across_processes";
const INTEGER_DIVISION_TEST_NAME: &str =
    "integer_division_phase_products_are_deterministic_across_processes";
const INTEGER_DIVISION_DIAGNOSTIC_TEST_NAME: &str =
    "integer_division_diagnostics_are_deterministic_across_processes";
const FLOATING_DIVISION_TEST_NAME: &str =
    "floating_division_phase_products_are_deterministic_across_processes";
const FLOATING_DIVISION_DIAGNOSTIC_TEST_NAME: &str =
    "floating_division_diagnostics_are_deterministic_across_processes";
const FLOATING_COMPARISON_TEST_NAME: &str =
    "floating_comparison_phase_products_are_deterministic_across_processes";
const FLOATING_COMPARISON_DIAGNOSTIC_TEST_NAME: &str =
    "floating_comparison_diagnostics_are_deterministic_across_processes";
const PRIMITIVE_OPERATOR_PROFILE_TEST_NAME: &str =
    "primitive_operator_profile_phase_products_are_deterministic_across_processes";
const PRIMITIVE_CAST_TEST_NAME: &str =
    "primitive_cast_phase_products_are_deterministic_across_processes";
const PRIMITIVE_CAST_DIAGNOSTIC_TEST_NAME: &str =
    "primitive_cast_diagnostics_are_deterministic_across_processes";
const EAGER_BOOLEAN_TEST_NAME: &str =
    "eager_boolean_phase_products_are_deterministic_across_processes";
const EAGER_BOOLEAN_DIAGNOSTIC_TEST_NAME: &str =
    "eager_boolean_diagnostics_are_deterministic_across_processes";
const SHORT_CIRCUIT_SOURCE_TEST_NAME: &str =
    "short_circuit_source_products_are_deterministic_across_processes";
const STRING_TEST_NAME: &str = "string_phase_products_are_deterministic_across_processes";
const STRING_DIAGNOSTIC_TEST_NAME: &str =
    "string_language_item_diagnostics_are_deterministic_across_processes";
const IO_TEST_NAME: &str = "io_phase_products_are_deterministic_across_processes";
const IO_DIAGNOSTIC_TEST_NAME: &str = "io_provider_diagnostics_are_deterministic_across_processes";
const MIR_CHECKPOINT_TEST_NAME: &str =
    "mir_pipeline_checkpoints_are_deterministic_across_processes";

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

same_input_case!(
    object_lifetime_phase_products_are_deterministic_across_processes,
    "object",
    object_phase_dump
);

#[test]
fn mir_pipeline_checkpoints_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "mir-pipeline-checkpoints",
        MIR_CHECKPOINT_TEST_NAME,
        mir_pipeline_checkpoint_dump,
    );
}

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

#[test]
fn integer_operation_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "integer-operations",
        INTEGER_OPERATION_TEST_NAME,
        integer_operation_phase_dump,
    );
}

#[test]
fn integer_bitwise_and_shift_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "integer-bitwise-shifts",
        INTEGER_BITWISE_SHIFT_TEST_NAME,
        integer_bitwise_and_shift_phase_dump,
    );
}

#[test]
fn integer_bitwise_and_shift_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "integer-bitwise-shift-diagnostics",
        INTEGER_BITWISE_SHIFT_DIAGNOSTIC_TEST_NAME,
        integer_bitwise_and_shift_diagnostic_dump,
    );
}

#[test]
fn integer_division_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "integer-division",
        INTEGER_DIVISION_TEST_NAME,
        integer_division_phase_dump,
    );
}

#[test]
fn integer_division_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "integer-division-diagnostics",
        INTEGER_DIVISION_DIAGNOSTIC_TEST_NAME,
        integer_division_diagnostic_dump,
    );
}

#[test]
fn floating_division_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "floating-division",
        FLOATING_DIVISION_TEST_NAME,
        floating_division_phase_dump,
    );
}

#[test]
fn floating_division_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "floating-division-diagnostics",
        FLOATING_DIVISION_DIAGNOSTIC_TEST_NAME,
        floating_division_diagnostic_dump,
    );
}

#[test]
fn floating_comparison_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "floating-comparisons",
        FLOATING_COMPARISON_TEST_NAME,
        floating_comparison_phase_dump,
    );
}

#[test]
fn floating_comparison_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "floating-comparison-diagnostics",
        FLOATING_COMPARISON_DIAGNOSTIC_TEST_NAME,
        floating_comparison_diagnostic_dump,
    );
}

#[test]
fn primitive_operator_profile_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "primitive-operator-profile",
        PRIMITIVE_OPERATOR_PROFILE_TEST_NAME,
        primitive_operator_profile_phase_dump,
    );
}

#[test]
fn primitive_cast_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "primitive-casts",
        PRIMITIVE_CAST_TEST_NAME,
        primitive_cast_phase_dump,
    );
}

#[test]
fn primitive_cast_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "primitive-cast-diagnostics",
        PRIMITIVE_CAST_DIAGNOSTIC_TEST_NAME,
        primitive_cast_diagnostic_dump,
    );
}

#[test]
fn eager_boolean_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "eager-booleans",
        EAGER_BOOLEAN_TEST_NAME,
        eager_boolean_phase_dump,
    );
}

#[test]
fn eager_boolean_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "eager-boolean-diagnostics",
        EAGER_BOOLEAN_DIAGNOSTIC_TEST_NAME,
        eager_boolean_diagnostic_dump,
    );
}

#[test]
fn short_circuit_source_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "short-circuit-source",
        SHORT_CIRCUIT_SOURCE_TEST_NAME,
        short_circuit_source_phase_dump,
    );
}

#[test]
fn string_phase_products_are_deterministic_across_processes() {
    assert_cross_process_variants("strings", STRING_TEST_NAME, string_phase_dump);
}

#[test]
fn string_language_item_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_variants(
        "string-diagnostics",
        STRING_DIAGNOSTIC_TEST_NAME,
        string_diagnostic_dump,
    );
}

#[test]
fn io_phase_products_are_deterministic_across_processes() {
    assert_cross_process_variants("io", IO_TEST_NAME, |variant| io_phase_dump(variant, false));
}

#[test]
fn io_provider_diagnostics_are_deterministic_across_processes() {
    assert_cross_process_variants("io-diagnostics", IO_DIAGNOSTIC_TEST_NAME, |variant| {
        io_phase_dump(variant, true)
    });
}

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

same_input_case!(
    function_value_composition_products_are_deterministic_across_processes,
    "function-value-composition",
    function_value_composition_phase_dump
);

fn integer_operation_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!("../../../tests/golden/primitives/integer_string_range_guards.ska"),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

fn integer_bitwise_and_shift_phase_dump() -> String {
    single_source_full_phase_dump(
        concat!(
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
        ),
        StandardLibraryInput::None,
    )
}

fn integer_bitwise_and_shift_diagnostic_dump() -> String {
    single_source_type_error_dump(
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
        StandardLibraryInput::None,
    )
}

fn integer_division_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!("../../../tests/golden/operators/integer_division_operators.ska"),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

fn integer_division_diagnostic_dump() -> String {
    single_source_type_error_dump(
        "integer-division-diagnostics.ska",
        include_str!("../../../tests/golden/operators/integer_division_operator_types.ska"),
        StandardLibraryInput::None,
    )
}

fn floating_division_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!("../../../tests/golden/operators/floating_division.ska"),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

fn floating_division_diagnostic_dump() -> String {
    single_source_type_error_dump(
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
        StandardLibraryInput::None,
    )
}

fn floating_comparison_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!("../../../tests/golden/operators/floating_comparisons.ska"),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

fn floating_comparison_diagnostic_dump() -> String {
    single_source_type_error_dump(
        "floating-comparison-diagnostics.ska",
        concat!(
            "fn invalid(left: f64, integer: i64, flag: bool, optional: f64?) -> bool {\n",
            "  var mixed: bool = left < integer;\n",
            "  var boolean: bool = left == flag;\n",
            "  return left >= optional;\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        StandardLibraryInput::None,
    )
}

fn primitive_operator_profile_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!("../../../tests/golden/operators/primitive_operator_profile.ska"),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

fn primitive_cast_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!("../../../tests/golden/primitives/primitive_cast_matrix.ska"),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

fn primitive_cast_diagnostic_dump() -> String {
    single_source_type_error_dump(
        "primitive-cast-diagnostics.ska",
        concat!(
            "fn invalid(values: i64[]) -> u64 { return (u64) values; }\n",
            "fn implicit(value: f64) -> i64 { return value; }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        StandardLibraryInput::None,
    )
}

fn eager_boolean_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!("../../../tests/golden/operators/eager_boolean_operators.ska"),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

fn eager_boolean_diagnostic_dump() -> String {
    single_source_type_error_dump(
        "eager-boolean-diagnostics.ska",
        include_str!("../../../tests/golden/operators/eager_boolean_operator_types.ska"),
        StandardLibraryInput::None,
    )
}

fn short_circuit_source_phase_dump() -> String {
    single_source_full_phase_dump(
        concat!(
            "fn selected(a: bool, b: bool, c: bool) -> bool { return (a || b) && !c; }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        StandardLibraryInput::None,
    )
}

fn string_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("string-products", variant);
    let application = fixture.path().join("application");
    let standard_library = fixture.path().join("standard-library");
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
    let providers = normalize_provider_roots(fixture.path(), &configurations).unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        fixture.path(),
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
        fixture.path(),
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
    let application = fixture.path().join("application");
    let standard_library = fixture.path().join("standard-library");
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
    let providers = normalize_provider_roots(fixture.path(), &configurations).unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        fixture.path(),
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
    normalize_fixture_paths(fixture.path(), phases)
}

fn string_diagnostic_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("string-diagnostics", variant);
    let application = fixture.path().join("application");
    let standard_library = fixture.path().join("standard-library");
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
    let providers = normalize_provider_roots(fixture.path(), &configurations).unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        fixture.path(),
        &providers,
    )
    .unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.has_errors());

    normalize_fixture_paths(
        fixture.path(),
        format!(
            "GRAPH\n{}DIAGNOSTICS\n{}",
            dump_module_graph(&graph),
            render_diagnostics(graph.sources(), &resolved.diagnostics),
        ),
    )
}

fn mir_pipeline_checkpoint_dump() -> String {
    let text = "fn removed_target() -> i64 { return 99; }\n\
                fn identity(value: i64) -> i64 { return value + 0; }\n\
                fn main() -> i64 {\n\
                    if (1 + 1 == 2) { return identity(6 * 7); }\n\
                    return removed_target();\n\
                }\n";
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
        let label = checkpoint.label().to_string();
        match checkpoint {
            MirPipelineCheckpoint::ProofRich(checkpoint) => {
                checkpoints.push((label, dump_mir(checkpoint.verified()), None));
            }
            MirPipelineCheckpoint::Final(checkpoint) => checkpoints.push((
                label,
                dump_mir(checkpoint.verified()),
                Some(checkpoint.reachability_dump()),
            )),
        }
    };
    run_mir_pipeline_inspected(lower_hir(&checked.hir.unwrap()), &mut inspector).unwrap();

    checkpoints
        .into_iter()
        .map(|(label, mir, reachability)| {
            format!(
                "CHECKPOINT {label}\n{mir}{}",
                reachability.unwrap_or_default()
            )
        })
        .collect()
}
