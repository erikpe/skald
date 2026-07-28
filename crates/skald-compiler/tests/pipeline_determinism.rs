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
const STRING_HELPER_OUTPUT: &str = "SKALD_STRING_DETERMINISM_OUTPUT";
const STRING_TEST_NAME: &str = "string_typed_phase_products_are_deterministic_across_processes";
const MODULE_HELPER_OUTPUT: &str = "SKALD_MODULE_DETERMINISM_OUTPUT";
const MODULE_HELPER_VARIANT: &str = "SKALD_MODULE_DETERMINISM_VARIANT";
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
fn string_typed_phase_products_are_deterministic_across_processes() {
    assert_cross_process_determinism(
        "strings",
        STRING_HELPER_OUTPUT,
        STRING_TEST_NAME,
        string_typed_phase_dump,
    );
}

#[test]
fn module_phase_products_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(MODULE_HELPER_OUTPUT) {
        let variant = env::var(MODULE_HELPER_VARIANT).unwrap().parse().unwrap();
        fs::write(output, module_phase_dump(variant)).unwrap();
        return;
    }

    assert_cross_process_variants(
        "modules",
        MODULE_HELPER_OUTPUT,
        MODULE_TEST_NAME,
        MODULE_HELPER_VARIANT,
    );
}

#[test]
fn module_diagnostics_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(MODULE_DIAGNOSTIC_HELPER_OUTPUT) {
        let variant = env::var(MODULE_HELPER_VARIANT).unwrap().parse().unwrap();
        fs::write(output, module_diagnostic_dump(variant)).unwrap();
        return;
    }

    assert_cross_process_variants(
        "module-diagnostics",
        MODULE_DIAGNOSTIC_HELPER_OUTPUT,
        MODULE_DIAGNOSTIC_TEST_NAME,
        MODULE_HELPER_VARIANT,
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
        "import lib::answer;\nfrom support import zero;\n"
    } else {
        "from support import zero;\nimport lib::answer;\n"
    };
    let sources = [
        (
            application.join("app/main.ska"),
            format!("{imports}fn main() -> i64 {{ return lib::answer::value() + zero(); }}\n"),
        ),
        (
            dependencies.join("lib/answer.ska"),
            "public fn value() -> i64 { return 42; }\n".to_owned(),
        ),
        (
            dependencies.join("support.ska"),
            "public fn zero() -> i64 { return 0; }\n".to_owned(),
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
        EntrySelector::Module("app::main".parse().unwrap())
    } else {
        EntrySelector::File(application_alias.join("app/main.ska"))
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
    let application = fixture.path.join("application");
    let first = fixture.path.join("first");
    let second = fixture.path.join("second");
    write_source(
        &application.join("app.ska"),
        "import collision;\nfn main() -> i64 { return 0; }\n",
    );
    write_source(&first.join("collision.ska"), "fn first() -> unit {}\n");
    write_source(&second.join("collision.ska"), "fn second() -> unit {}\n");
    let roots = if variant == 0 {
        [&second, &application, &first]
    } else {
        [&first, &application, &second]
    };
    let configurations = roots
        .into_iter()
        .map(|root| ProviderRootConfiguration::module_root(root.to_owned()))
        .collect::<Vec<_>>();
    let providers = normalize_provider_roots(&fixture.path, &configurations).unwrap();
    let failure = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        &fixture.path,
        &providers,
    )
    .unwrap_err();

    normalize_fixture_paths(
        &fixture.path,
        render_diagnostics(failure.sources(), failure.diagnostics()),
    )
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

fn string_typed_phase_dump() -> String {
    let fixture = ModuleFixture::new("string-products", 0);
    let root = fixture.path.join("modules");
    write_source(
        &root.join("app.ska"),
        concat!(
            "from std::str import Str;\n",
            "fn produce() -> Str { return \"first\\0\"; }\n",
            "fn main() -> i64 { var value: Str = \"\\x73econd\"; return 0; }\n",
        ),
    );
    write_source(
        &root.join("std/str.ska"),
        concat!(
            "public class Str {\n",
            "  private storage: shared u8[];\n",
            "  private start: u64;\n",
            "  private length: u64;\n",
            "  init() { self.storage = new u8[](); self.start = 0u; self.length = 0u; }\n",
            "}\n",
        ),
    );
    let providers = normalize_provider_roots(
        &fixture.path,
        &[ProviderRootConfiguration::module_root(root)],
    )
    .unwrap();
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

    normalize_fixture_paths(
        &fixture.path,
        format!(
            "GRAPH\n{}RESOLVED\n{}HIR\n{}",
            dump_module_graph(&graph),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
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
