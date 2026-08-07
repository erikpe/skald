use super::{
    load_bytes, MatcherLoadFailure, MatcherMatch, MatcherMismatch, MatcherOutcome,
    StreamComparison, StreamMatcher, StreamMatcherSet,
};
use crate::{ExitExpectation, MatchMode, ProcessTermination, ResolvedStreamExpectation};

/// Compares every matcher independently with the same captured stream.
pub fn compare_matchers(
    matchers: &StreamMatcherSet<crate::ResolvedByteSource>,
    actual: &[u8],
) -> StreamComparison {
    let outcomes = matchers
        .matchers()
        .iter()
        .enumerate()
        .map(|(index, matcher)| compare_matcher(index, matcher, actual))
        .collect();
    StreamComparison::new(actual, outcomes)
}

/// Compares one captured stream using its resolved matcher collection or
/// whole-stream ignore policy.
pub fn compare_stream(expectation: &ResolvedStreamExpectation, actual: &[u8]) -> StreamComparison {
    match expectation {
        ResolvedStreamExpectation::Ignore => StreamComparison::new_ignored(actual),
        ResolvedStreamExpectation::Match(matchers) => compare_matchers(matchers, actual),
    }
}

fn compare_matcher(
    index: usize,
    matcher: &StreamMatcher<crate::ResolvedByteSource>,
    actual: &[u8],
) -> MatcherOutcome {
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
