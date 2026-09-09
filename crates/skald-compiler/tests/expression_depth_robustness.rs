//! Process-isolated regressions for expression shapes that formerly aborted
//! the compiler with a stack overflow.

use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use skald_compiler::{
    backend::Target,
    diagnostics::render_diagnostics,
    driver::{compile_source_to_assembly, CompilationError},
};

const HELPER_CASE: &str = "SKALD_EXPRESSION_DEPTH_HELPER_CASE";
const HOSTILE_TERMS: usize = 10_000;
const TIMEOUT: Duration = Duration::from_secs(15);

#[test]
fn hostile_expression_trees_terminate_in_an_external_process() {
    for case in [
        "additive",
        "postfix",
        "nested-postfix",
        "malformed-additive",
    ] {
        run_helper_with_watchdog(case);
    }
}

#[test]
fn expression_depth_subprocess_helper() {
    let Ok(case) = std::env::var(HELPER_CASE) else {
        return;
    };
    let expression = match case.as_str() {
        "additive" => std::iter::repeat_n("1", HOSTILE_TERMS)
            .collect::<Vec<_>>()
            .join(" + "),
        "postfix" => format!("value{}", ".member".repeat(HOSTILE_TERMS)),
        "nested-postfix" => (1..=100).fold("value".to_owned(), |expression, members| {
            format!("({expression}){}", ".member".repeat(members))
        }),
        "malformed-additive" => "1 + ".repeat(HOSTILE_TERMS),
        _ => panic!("unknown expression-depth helper case {case:?}"),
    };
    let source = format!("fn main() -> i64 {{ return {expression}; }}");

    let CompilationError::Diagnostics(report) =
        compile_source_to_assembly("hostile-expression.ska", source, Target::X86_64SysV)
            .expect_err("hostile expression must stop at the parser resource limit")
    else {
        panic!("hostile expression failed outside source diagnostics");
    };
    let rendered = render_diagnostics(&report.sources, &report.diagnostics);
    assert!(
        rendered.contains("error[PAR005]") && rendered.contains("implementation limit"),
        "unexpected diagnostics for {case}:\n{rendered}"
    );
}

fn run_helper_with_watchdog(case: &str) {
    let mut child =
        Command::new(std::env::current_exe().expect("test executable must have a path"))
            .args([
                "--exact",
                "expression_depth_subprocess_helper",
                "--nocapture",
            ])
            .env(HELPER_CASE, case)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("failed to spawn expression-depth helper: {error}"));

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .unwrap_or_else(|error| panic!("failed to poll expression-depth helper: {error}"))
        {
            break status;
        }
        if started.elapsed() >= TIMEOUT {
            child.kill().expect("timed-out helper must be killable");
            let _ = child.wait();
            panic!("expression-depth helper {case:?} exceeded {TIMEOUT:?}");
        }
        thread::sleep(Duration::from_millis(10));
    };

    let mut output = String::new();
    child
        .stdout
        .take()
        .expect("helper stdout must be piped")
        .read_to_string(&mut output)
        .expect("helper stdout must be readable");
    child
        .stderr
        .take()
        .expect("helper stderr must be piped")
        .read_to_string(&mut output)
        .expect("helper stderr must be readable");
    assert!(
        status.success(),
        "expression-depth helper {case:?} failed with {status}:\n{output}"
    );
}
