use super::{
    load_bytes, ExpectationError, MatcherLoadFailure, MatcherMatch, MatcherMismatch,
    MatcherOutcome, StreamComparison, StreamMatch, StreamMatcher, StreamMatcherSet, StreamMismatch,
};
use crate::{ExitExpectation, MatchMode, ProcessTermination, ResolvedStreamExpectation};

/// Compares every matcher independently with the same captured stream.
pub fn compare_matchers(matchers: &StreamMatcherSet, actual: &[u8]) -> StreamComparison {
    let outcomes = matchers
        .matchers()
        .iter()
        .enumerate()
        .map(|(index, matcher)| compare_matcher(index, matcher, actual))
        .collect();
    StreamComparison::new(actual, outcomes)
}

/// Compares one captured stream using its resolved byte policy.
pub fn compare_stream(
    expectation: &ResolvedStreamExpectation,
    actual: &[u8],
) -> Result<Result<StreamMatch, StreamMismatch>, ExpectationError> {
    let ResolvedStreamExpectation::Match { mode, expected } = expectation else {
        return Ok(Ok(StreamMatch::Ignored));
    };
    let matcher = StreamMatcher::new(*mode, expected.clone());
    let comparison = compare_matchers(&StreamMatcherSet::one(matcher), actual);
    let (actual, mut outcomes) = comparison.into_parts();
    let outcome = outcomes
        .pop()
        .expect("a singular matcher comparison must have one outcome");
    match outcome {
        MatcherOutcome::Matched(result) => Ok(Ok(StreamMatch::Matched {
            mode: result.mode(),
            offset: result.offset(),
        })),
        MatcherOutcome::Mismatched(result) => Ok(Err(StreamMismatch::from_owned(
            result.mode(),
            result.into_expected(),
            actual,
        ))),
        MatcherOutcome::LoadFailed(failure) => Err(failure.into_error()),
    }
}

fn compare_matcher(index: usize, matcher: &StreamMatcher, actual: &[u8]) -> MatcherOutcome {
    let expected = match load_bytes(matcher.expected()) {
        Ok(expected) => expected,
        Err(error) => {
            return MatcherOutcome::LoadFailed(MatcherLoadFailure::new(index, matcher, error));
        }
    };
    match match_offset(matcher.mode(), &expected, actual) {
        Some(offset) => {
            MatcherOutcome::Matched(MatcherMatch::new(index, matcher, offset, expected.len()))
        }
        None => MatcherOutcome::Mismatched(MatcherMismatch::new(index, matcher, expected)),
    }
}

fn match_offset(mode: MatchMode, expected: &[u8], actual: &[u8]) -> Option<usize> {
    match mode {
        MatchMode::Exact if actual == expected => Some(0),
        MatchMode::StartsWith if actual.starts_with(expected) => Some(0),
        MatchMode::Contains => find_bytes(actual, expected),
        MatchMode::Exact | MatchMode::StartsWith => None,
    }
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

pub(super) fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
