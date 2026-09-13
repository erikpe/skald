use std::{env, fs, path::Path, path::PathBuf, time::Duration};

use crate::support::{
    run_current_test_process, CurrentTestProcessRequest, TemporaryFile, TestProcessPolicy,
    TestProcessTermination,
};

const HELPER_OUTPUT: &str = "SKALD_PIPELINE_DETERMINISM_HELPER_OUTPUT";
const HELPER_VARIANT: &str = "SKALD_PIPELINE_DETERMINISM_HELPER_VARIANT";
const HELPER_TIMEOUT: Duration = Duration::from_secs(60);

pub(crate) fn assert_cross_process_determinism(
    label: &str,
    test_name: &str,
    generate: fn() -> String,
) {
    if let Some(output) = same_input_helper_output() {
        fs::write(output, generate()).unwrap();
        return;
    }

    let first = TemporaryFile::new(&format!("{label}-determinism-first"))
        .expect("first determinism output must be creatable");
    let second = TemporaryFile::new(&format!("{label}-determinism-second"))
        .expect("second determinism output must be creatable");
    run_helper_process(first.path(), test_name, None);
    run_helper_process(second.path(), test_name, None);

    assert_eq!(
        first.read().unwrap(),
        second.read().unwrap(),
        "{label} phase products changed across independent compiler processes"
    );
}

pub(crate) fn assert_cross_process_variants(
    label: &str,
    test_name: &str,
    generate: fn(usize) -> String,
) {
    if let Some((output, variant)) = variant_helper_state() {
        fs::write(output, generate(variant)).unwrap();
        return;
    }

    let first = TemporaryFile::new(&format!("{label}-determinism-first"))
        .expect("first determinism output must be creatable");
    let second = TemporaryFile::new(&format!("{label}-determinism-second"))
        .expect("second determinism output must be creatable");
    run_helper_process(first.path(), test_name, Some(0));
    run_helper_process(second.path(), test_name, Some(1));

    assert_eq!(
        first.read().unwrap(),
        second.read().unwrap(),
        "{label} products changed across independent compiler processes and input permutations"
    );
}

fn run_helper_process(output: &Path, test_name: &str, variant: Option<usize>) {
    let mut request = CurrentTestProcessRequest::new(
        test_name,
        TestProcessPolicy::with_default_diagnostic_limit(HELPER_TIMEOUT),
    )
    .with_environment(HELPER_OUTPUT, output);
    if let Some(variant) = variant {
        request = request.with_environment(HELPER_VARIANT, variant.to_string());
    }

    let observation = run_current_test_process(request)
        .unwrap_or_else(|error| panic!("failed to run determinism helper {test_name:?}: {error}"));
    let succeeded = matches!(
        observation.termination,
        TestProcessTermination::Completed(status) if status.success()
    );
    assert!(
        succeeded && !observation.stdout.overflowed() && !observation.stderr.overflowed(),
        "determinism helper {test_name:?} ended with {:?}\n\
         stdout ({} bytes observed, {} retained):\n{}\n\
         stderr ({} bytes observed, {} retained):\n{}",
        observation.termination,
        observation.stdout.observed_length(),
        observation.stdout.retained().len(),
        String::from_utf8_lossy(observation.stdout.retained()),
        observation.stderr.observed_length(),
        observation.stderr.retained().len(),
        String::from_utf8_lossy(observation.stderr.retained()),
    );
}

fn same_input_helper_output() -> Option<PathBuf> {
    match (env::var_os(HELPER_OUTPUT), env::var_os(HELPER_VARIANT)) {
        (None, None) => None,
        (Some(output), None) => Some(output.into()),
        (output, variant) => panic!(
            "malformed determinism helper state for same-input case: \
             {HELPER_OUTPUT}={output:?}, {HELPER_VARIANT}={variant:?}"
        ),
    }
}

fn variant_helper_state() -> Option<(PathBuf, usize)> {
    match (env::var_os(HELPER_OUTPUT), env::var_os(HELPER_VARIANT)) {
        (None, None) => None,
        (Some(output), Some(variant)) => {
            let variant = variant
                .to_str()
                .and_then(|variant| variant.parse().ok())
                .filter(|variant| matches!(variant, 0 | 1))
                .unwrap_or_else(|| {
                    panic!("invalid determinism helper permutation {variant:?}; expected 0 or 1")
                });
            Some((output.into(), variant))
        }
        (output, variant) => panic!(
            "malformed determinism helper state for permutation case: \
             {HELPER_OUTPUT}={output:?}, {HELPER_VARIANT}={variant:?}"
        ),
    }
}
