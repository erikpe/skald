use super::{load_bytes, ExpectationError, StreamMatch, StreamMismatch};
use crate::{ExitExpectation, MatchMode, ProcessTermination, ResolvedStreamExpectation};

/// Compares one captured stream using its resolved byte policy.
pub fn compare_stream(
    expectation: &ResolvedStreamExpectation,
    actual: &[u8],
) -> Result<Result<StreamMatch, StreamMismatch>, ExpectationError> {
    let ResolvedStreamExpectation::Match { mode, expected } = expectation else {
        return Ok(Ok(StreamMatch::Ignored));
    };
    let expected = load_bytes(expected)?;
    let offset = match mode {
        MatchMode::Exact if actual == expected => Some(0),
        MatchMode::StartsWith if actual.starts_with(&expected) => Some(0),
        MatchMode::Contains => find_bytes(actual, &expected),
        MatchMode::Exact | MatchMode::StartsWith => None,
    };
    Ok(match offset {
        Some(offset) => Ok(StreamMatch::Matched {
            mode: *mode,
            offset,
        }),
        None => Err(StreamMismatch::new(*mode, expected, actual)),
    })
}

/// Checks an exit expectation. Timeouts never satisfy a general failure.
pub fn compare_exit(expected: ExitExpectation, actual: ProcessTermination) -> bool {
    match (expected, actual) {
        (ExitExpectation::Code(expected), ProcessTermination::Code(actual)) => expected == actual,
        (ExitExpectation::Failure, ProcessTermination::Code(code)) => code != 0,
        (ExitExpectation::Failure, ProcessTermination::Signal(_)) => true,
        (ExitExpectation::Code(_), ProcessTermination::Signal(_))
        | (_, ProcessTermination::TimedOut { .. }) => false,
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::{compare_exit, find_bytes};
    use crate::{ExitExpectation, ProcessTermination};
    use std::time::Duration;

    #[test]
    fn byte_search_reports_the_first_contiguous_offset() {
        assert_eq!(
            find_bytes(b"before\0target\xffafter", b"target\xff"),
            Some(7)
        );
        assert_eq!(find_bytes(b"target", b"absent"), None);
    }

    #[test]
    fn failure_expectations_do_not_treat_timeouts_as_program_failures() {
        assert!(compare_exit(
            ExitExpectation::Failure,
            ProcessTermination::Code(9)
        ));
        assert!(compare_exit(
            ExitExpectation::Failure,
            ProcessTermination::Signal(15)
        ));
        assert!(!compare_exit(
            ExitExpectation::Failure,
            ProcessTermination::Code(0)
        ));
        assert!(!compare_exit(
            ExitExpectation::Failure,
            ProcessTermination::TimedOut {
                limit: Duration::from_secs(1)
            }
        ));
    }
}
