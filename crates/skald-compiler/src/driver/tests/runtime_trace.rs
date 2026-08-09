use super::*;

use std::{env, fs, process::Command};

use crate::test_support::{run_native_assembly_with_runtime_trace_probe, TemporaryDirectory};

const DETERMINISM_OUTPUT: &str = "SKALD_RUNTIME_TRACE_DETERMINISM_OUTPUT";
const DETERMINISM_TEST: &str =
    "driver::tests::runtime_trace::enabled_trace_products_are_deterministic_across_processes";

fn compile_and_run(path: &str, source: &str) -> std::process::Output {
    let artifact = compile_source_to_assembly(path, source, Target::X86_64SysV)
        .expect("runtime-trace fixture must compile");
    run_native_assembly_with_runtime_trace_probe(&artifact.assembly)
}

#[test]
fn singleton_pipeline_renders_escaped_paths() {
    let result = compile_and_run(
        "odd\\part\nnext\trow/main.ska",
        "fn main() -> i64 { var zero: i64 = 0; return 1 / zero; }",
    );

    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(result.stderr).unwrap(),
        concat!(
            "panic: integer division by zero\n",
            "stacktrace:\n",
            "  at main::main (odd\\\\part\\nnext\\trow/main.ska:1:46)\n",
        )
    );
}

#[test]
fn singleton_pipeline_renders_semantic_initializer_signatures() {
    let result = compile_and_run(
        "initializer.ska",
        concat!(
            "class Item {\n",
            "  value: i64;\n",
            "  init(value: i64) { self.value = value / 0; }\n",
            "}\n",
            "fn main() -> i64 { var item: Item = Item(1); return 0; }\n",
        ),
    );

    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(result.stderr).unwrap(),
        concat!(
            "panic: integer division by zero\n",
            "stacktrace:\n",
            "  at main::Item.init(i64) (initializer.ska:3:35)\n",
            "  at main::main (initializer.ska:5:37)\n",
        )
    );
}

#[test]
fn enabled_trace_products_are_deterministic_across_processes() {
    if let Some(output) = env::var_os(DETERMINISM_OUTPUT) {
        fs::write(output, enabled_trace_products()).unwrap();
        return;
    }

    let directory = TemporaryDirectory::new("runtime-trace-determinism-output").unwrap();
    let first = directory.join("first.txt");
    let second = directory.join("second.txt");
    run_determinism_process(&first);
    run_determinism_process(&second);

    let first = fs::read_to_string(first).unwrap();
    let second = fs::read_to_string(second).unwrap();
    assert_eq!(
        first, second,
        "enabled trace products depend on provider roots"
    );
    assert!(first.contains("STATUS\n1\n"));
    assert!(first.contains("  at app::main::fail (app/main.ska:2:10)\n"));
    assert!(first.contains("  at app::main::main (app/main.ska:5:10)\n"));
    assert!(first.contains("ASSEMBLY\n"));
    assert!(first.contains("ska_rt_trace_top@tpoff"));
    assert!(first.contains(".Lska.trace.context."));
    assert!(first.contains(".Lska.trace.location."));
}

fn run_determinism_process(output: &std::path::Path) {
    let result = Command::new(env::current_exe().unwrap())
        .args(["--exact", DETERMINISM_TEST, "--nocapture"])
        .env(DETERMINISM_OUTPUT, output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "runtime-trace determinism helper failed:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn enabled_trace_products() -> String {
    let directory = TemporaryDirectory::new("runtime-trace-provider").unwrap();
    let provider = directory.join("provider");
    fs::create_dir_all(provider.join("app")).unwrap();
    fs::write(
        provider.join("app/main.ska"),
        concat!(
            "fn fail(value: i64) -> i64 {\n",
            "  return value / 0;\n",
            "}\n",
            "fn main() -> i64 {\n",
            "  return fail(1);\n",
            "}\n",
        ),
    )
    .unwrap();
    let request = CompilationRequest::new(
        EntrySelector::Module("app::main".parse().unwrap()),
        vec![provider],
        StandardLibrarySelection::Disabled,
        Target::X86_64SysV,
        ArtifactOptions::new(ArtifactKind::Assembly, None),
        CompilationEnvironment::new(directory.path().to_owned(), directory.join("unused-std")),
    );

    let artifact = compile_request_to_assembly(&request).unwrap();
    let result = run_native_assembly_with_runtime_trace_probe(&artifact.assembly);
    assert!(result.stdout.is_empty());

    format!(
        "STATUS\n{}\nSTDERR\n{}ASSEMBLY\n{}",
        result.status.code().unwrap(),
        String::from_utf8(result.stderr).unwrap(),
        artifact.assembly,
    )
}
