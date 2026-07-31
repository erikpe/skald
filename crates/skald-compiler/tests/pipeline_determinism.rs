//! Cross-process determinism coverage for representative complete pipelines.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use skald_compiler::{
    backend::{emit_assembly, Target},
    diagnostics::render_diagnostics,
    driver::EntrySelector,
    hir::dump_hir,
    lexer::{dump_tokens, lex},
    mir::{dump_mir, lower_hir},
    module::{
        dump_module_graph, load_module_graph, normalize_provider_roots, ProviderRootConfiguration,
    },
    passes::run_mir_pipeline,
    resolve::{dump_resolved, resolve, resolve_module_graph},
    source::SourceDatabase,
    syntax::{dump_ast, parse},
    typeck::type_check,
};

const OBJECT_HELPER_OUTPUT: &str = "SKALD_OBJECT_DETERMINISM_OUTPUT";
const OBJECT_TEST_NAME: &str = "object_lifetime_phase_products_are_deterministic_across_processes";
const POLYMORPHISM_HELPER_OUTPUT: &str = "SKALD_POLYMORPHISM_DETERMINISM_OUTPUT";
const POLYMORPHISM_TEST_NAME: &str =
    "polymorphism_phase_products_are_deterministic_across_processes";
const SHARED_HELPER_OUTPUT: &str = "SKALD_SHARED_DETERMINISM_OUTPUT";
const SHARED_TEST_NAME: &str = "shared_ownership_phase_products_are_deterministic_across_processes";
const OPTIONAL_HELPER_OUTPUT: &str = "SKALD_OPTIONAL_DETERMINISM_OUTPUT";
const OPTIONAL_TEST_NAME: &str = "optional_value_phase_products_are_deterministic_across_processes";
const ARRAY_HELPER_OUTPUT: &str = "SKALD_ARRAY_DETERMINISM_OUTPUT";
const ARRAY_TEST_NAME: &str = "array_phase_products_are_deterministic_across_processes";
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
const PRIVATE_INITIALIZER_HELPER_OUTPUT: &str = "SKALD_PRIVATE_INITIALIZER_DETERMINISM_OUTPUT";
const PRIVATE_INITIALIZER_TEST_NAME: &str =
    "private_initializer_phase_products_are_deterministic_across_processes";
const PRIVATE_INITIALIZER_DIAGNOSTIC_HELPER_OUTPUT: &str =
    "SKALD_PRIVATE_INITIALIZER_DIAGNOSTIC_DETERMINISM_OUTPUT";
const PRIVATE_INITIALIZER_DIAGNOSTIC_TEST_NAME: &str =
    "private_initializer_diagnostics_are_deterministic_across_processes";
const MODULE_HELPER_OUTPUT: &str = "SKALD_MODULE_DETERMINISM_OUTPUT";
const PERMUTATION_HELPER_VARIANT: &str = "SKALD_DETERMINISM_VARIANT";
const MODULE_TEST_NAME: &str = "module_phase_products_are_deterministic_across_processes";
const MODULE_DIAGNOSTIC_HELPER_OUTPUT: &str = "SKALD_MODULE_DIAGNOSTIC_DETERMINISM_OUTPUT";
const MODULE_DIAGNOSTIC_TEST_NAME: &str = "module_diagnostics_are_deterministic_across_processes";

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
fn polymorphism_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "polymorphism",
        POLYMORPHISM_HELPER_OUTPUT,
        POLYMORPHISM_TEST_NAME,
        polymorphism_phase_dump,
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
                    "../../../tests/golden/run/modules_cycle/modules/app.ska"
                ))
            ),
        ),
        (
            dependencies.join("first.ska"),
            include_str!("../../../tests/golden/run/modules_cycle/modules/first.ska").to_owned(),
        ),
        (
            dependencies.join("second.ska"),
            include_str!("../../../tests/golden/run/modules_cycle/modules/second.ska").to_owned(),
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
    let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();

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
            include_str!(
                "../../../tests/golden/compile_fail/modules_cycle_diagnostics/modules/app.ska"
            ),
        ),
        (
            modules.join("left.ska"),
            include_str!(
                "../../../tests/golden/compile_fail/modules_cycle_diagnostics/modules/left.ska"
            ),
        ),
        (
            modules.join("right.ska"),
            include_str!(
                "../../../tests/golden/compile_fail/modules_cycle_diagnostics/modules/right.ska"
            ),
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
    complete_phase_dump(include_str!("../../../tests/golden/run/polymorphism.ska"))
}

fn shared_ownership_phase_dump() -> String {
    complete_phase_dump(include_str!(
        "../../../tests/golden/run/shared_copy_allocation.ska"
    ))
}

fn optional_phase_dump() -> String {
    complete_phase_dump(include_str!(
        "../../../tests/golden/run/optional_shared_profile.ska"
    ))
}

fn array_phase_dump() -> String {
    complete_phase_dump(include_str!("../../../tests/golden/run/array_aliases.ska"))
}

fn integer_operation_phase_dump() -> String {
    complete_phase_dump(include_str!(
        "../../../tests/golden/run/integer_string_range_guards.ska"
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
        "return (((~bits.value + 1u8 << bits.count) >> 1u) & values[0] ",
        "^ optional! | 1u8) == 7u8 && true; }\n",
        "fn cleanup() -> u64 { return make(16u)->read() >> make(2u)->read(); }\n",
        "fn main() -> i64 { var bits: Bits = Bits(3u8, 2u); ",
        "var optional: u8? = 4u8; var values: u8[] = u8[](1u); values[0] = 7u8; ",
        "if (mix(bits, optional, values) || cleanup() == 4u) { return 0; } return 1; }\n",
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
    complete_phase_dump(include_str!(
        "../../../tests/golden/run/integer_division_operators.ska"
    ))
}

fn integer_division_diagnostic_dump() -> String {
    type_error_phase_dump(
        "integer-division-diagnostics.ska",
        include_str!("../../../tests/golden/compile_fail/integer_division_operator_types.ska"),
    )
}

fn floating_division_phase_dump() -> String {
    complete_phase_dump(include_str!(
        "../../../tests/golden/run/floating_division.ska"
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
    complete_phase_dump(include_str!(
        "../../../tests/golden/run/floating_comparisons.ska"
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
    complete_phase_dump(include_str!(
        "../../../tests/golden/run/primitive_operator_profile.ska"
    ))
}

fn eager_boolean_phase_dump() -> String {
    complete_phase_dump(include_str!(
        "../../../tests/golden/run/eager_boolean_operators.ska"
    ))
}

fn eager_boolean_diagnostic_dump() -> String {
    type_error_phase_dump(
        "eager-boolean-diagnostics.ska",
        include_str!("../../../tests/golden/compile_fail/eager_boolean_operator_types.ska"),
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
    let sources = [
        (
            application.join("app.ska"),
            include_str!("../../../tests/golden/run/strings.ska"),
        ),
        (
            standard_library.join("std/str.ska"),
            include_str!("../../../std/std/str.ska"),
        ),
        (
            standard_library.join("std/error.ska"),
            include_str!("../../../std/std/error.ska"),
        ),
    ];
    for index in if variant == 0 { [0, 1, 2] } else { [2, 1, 0] } {
        write_source(&sources[index].0, sources[index].1);
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
    let mir = run_mir_pipeline(lower_hir(&hir)).unwrap();
    let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();

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

fn string_diagnostic_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("string-diagnostics", variant);
    let application = fixture.path.join("application");
    let standard_library = fixture.path.join("standard-library");
    let sources = [
        (
            application.join("app.ska"),
            "import feature;\nfn main() -> i64 { \"app\"; return 0; }\n",
        ),
        (
            application.join("feature.ska"),
            "public fn value() -> unit { \"feature\"; }\n",
        ),
        (
            standard_library.join("std/str.ska"),
            concat!(
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
            ),
        ),
    ];
    for index in if variant == 0 { [0, 1, 2] } else { [2, 1, 0] } {
        write_source(&sources[index].0, sources[index].1);
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
    let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();

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
