//! Contract tests for compiler integration subprocess and temporary resources.

mod support;

use std::{
    collections::HashSet,
    io::{self, Write},
    panic,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use support::{
    run_current_test_process, CurrentTestProcessRequest, TemporaryDirectory, TemporaryFile,
    TestProcessPolicy, TestProcessTermination,
};

const HELPER_MODE: &str = "SKALD_TEST_PLUMBING_HELPER_MODE";
const HELPER_VALUE: &str = "SKALD_TEST_PLUMBING_HELPER_VALUE";
const HELPER_TEST: &str = "test_process_helper";
const HELPER_FALLBACK: Duration = Duration::from_secs(2);
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn temporary_resources_are_unique_under_parallel_creation() {
    let paths: Vec<_> = (0..32)
        .map(|_| {
            thread::spawn(|| {
                let file = TemporaryFile::new("parallel/resource").unwrap();
                let directory = TemporaryDirectory::new("parallel/resource").unwrap();
                std::fs::write(file.path(), b"owned temporary output").unwrap();
                assert_eq!(file.read().unwrap(), b"owned temporary output");
                let paths = (file.path().to_owned(), directory.path().to_owned());
                assert!(paths.0.starts_with(std::env::temp_dir()));
                assert!(paths.1.starts_with(std::env::temp_dir()));
                paths
            })
        })
        .map(|worker| worker.join().unwrap())
        .flat_map(|paths| [paths.0, paths.1])
        .collect();

    assert_eq!(paths.iter().collect::<HashSet<_>>().len(), paths.len());
    assert!(paths.iter().all(|path| !path.exists()));
}

#[test]
fn temporary_resources_are_cleaned_during_unwinding() {
    let paths = Arc::new(Mutex::new(Vec::new()));
    let observed_paths = Arc::clone(&paths);
    let result = panic::catch_unwind(move || {
        let file = TemporaryFile::new("unwind-file").unwrap();
        let directory = TemporaryDirectory::new("unwind-directory").unwrap();
        observed_paths
            .lock()
            .unwrap()
            .extend([file.path().to_owned(), directory.path().to_owned()]);
        panic!("exercise cleanup during unwinding");
    });

    assert!(result.is_err());
    assert!(paths.lock().unwrap().iter().all(|path| !path.exists()));
}

#[test]
fn current_test_process_observes_success_and_failure() {
    let success = run_current_test_process(
        CurrentTestProcessRequest::new(
            HELPER_TEST,
            TestProcessPolicy::with_default_diagnostic_limit(TEST_TIMEOUT),
        )
        .with_environment(HELPER_MODE, "success"),
    )
    .unwrap();
    assert!(matches!(
        success.termination,
        TestProcessTermination::Completed(status) if status.success()
    ));
    assert!(String::from_utf8_lossy(success.stdout.retained()).contains("helper succeeded"));

    let failure = run_helper("failure", TEST_TIMEOUT, 4096);
    assert!(matches!(
        failure.termination,
        TestProcessTermination::Completed(status) if !status.success()
    ));
    assert!(String::from_utf8_lossy(failure.stderr.retained()).contains("helper failed"));
}

#[test]
fn current_test_process_kills_and_reaps_at_the_deadline() {
    let observation = run_helper("timeout", Duration::from_millis(50), 1024);
    assert!(matches!(
        observation.termination,
        TestProcessTermination::TimedOut
    ));
}

#[test]
fn current_test_process_bounds_both_diagnostic_streams() {
    let observation = run_current_test_process(
        CurrentTestProcessRequest::new(
            HELPER_TEST,
            TestProcessPolicy::with_default_diagnostic_limit(TEST_TIMEOUT),
        )
        .with_environment(HELPER_MODE, "overflow"),
    )
    .unwrap();
    assert!(matches!(
        observation.termination,
        TestProcessTermination::Completed(status) if status.success()
    ));
    for output in [&observation.stdout, &observation.stderr] {
        assert_eq!(output.retained().len(), 64 * 1024);
        assert!(output.observed_length() > 64 * 1024);
        assert!(output.overflowed());
    }
}

#[test]
fn concurrent_current_test_processes_isolate_environment_and_output() {
    let observations: Vec<_> = (0..8)
        .map(|index| {
            thread::spawn(move || {
                let value = format!("isolated-{index}");
                let request = CurrentTestProcessRequest::new(
                    HELPER_TEST,
                    TestProcessPolicy::new(TEST_TIMEOUT, 4096),
                )
                .with_environment(HELPER_MODE, "environment")
                .with_environment(HELPER_VALUE, &value);
                (value, run_current_test_process(request).unwrap())
            })
        })
        .map(|worker| worker.join().unwrap())
        .collect();

    for (value, observation) in &observations {
        assert!(matches!(
            observation.termination,
            TestProcessTermination::Completed(status) if status.success()
        ));
        let stdout = String::from_utf8_lossy(observation.stdout.retained());
        assert!(stdout.contains(value));
        assert!(observations
            .iter()
            .filter(|(candidate, _)| candidate != value)
            .all(|(candidate, _)| !stdout.contains(candidate)));
    }
}

#[test]
fn test_process_helper() {
    let Ok(mode) = std::env::var(HELPER_MODE) else {
        return;
    };
    match mode.as_str() {
        "success" => println!("helper succeeded"),
        "failure" => panic!("helper failed"),
        "timeout" => thread::sleep(HELPER_FALLBACK),
        "overflow" => {
            let bytes = vec![b'x'; 70 * 1024];
            io::stdout().write_all(&bytes).unwrap();
            io::stderr().write_all(&bytes).unwrap();
        }
        "environment" => println!("{}", std::env::var(HELPER_VALUE).unwrap()),
        mode => panic!("unknown test-process helper mode {mode:?}"),
    }
}

fn run_helper(
    mode: &str,
    timeout: Duration,
    diagnostic_limit: usize,
) -> support::CurrentTestProcessObservation {
    let request = CurrentTestProcessRequest::new(
        HELPER_TEST,
        TestProcessPolicy::new(timeout, diagnostic_limit),
    )
    .with_environment(HELPER_MODE, mode);
    run_current_test_process(request).unwrap()
}
