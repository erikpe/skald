//! Process-isolated regressions for expression shapes that formerly aborted
//! the compiler with a stack overflow.

use std::time::Duration;

use skald_compiler::{
    backend::Target,
    diagnostics::render_diagnostics,
    driver::{compile_source_to_assembly, CompilationError},
};

mod support;
use support::{
    run_current_test_process, CurrentTestProcessObservation, CurrentTestProcessRequest,
    TestProcessPolicy, TestProcessTermination,
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
    let request = CurrentTestProcessRequest::new(
        "expression_depth_subprocess_helper",
        TestProcessPolicy::with_default_diagnostic_limit(TIMEOUT),
    )
    .with_environment(HELPER_CASE, case);
    let observation = run_current_test_process(request)
        .unwrap_or_else(|error| panic!("failed to run expression-depth helper {case:?}: {error}"));
    let context = helper_failure_context(&observation);

    match observation.termination {
        TestProcessTermination::TimedOut => {
            panic!("expression-depth helper {case:?} exceeded {TIMEOUT:?}:{context}")
        }
        TestProcessTermination::Completed(status) => assert!(
            status.success()
                && !observation.stdout.overflowed()
                && !observation.stderr.overflowed(),
            "expression-depth helper {case:?} failed with {status}:{context}"
        ),
    }
}

fn helper_failure_context(observation: &CurrentTestProcessObservation) -> String {
    format!(
        "\nstdout ({} bytes observed, {} retained):\n{}\n\
         stderr ({} bytes observed, {} retained):\n{}",
        observation.stdout.observed_length(),
        observation.stdout.retained().len(),
        String::from_utf8_lossy(observation.stdout.retained()),
        observation.stderr.observed_length(),
        observation.stderr.retained().len(),
        String::from_utf8_lossy(observation.stderr.retained()),
    )
}
