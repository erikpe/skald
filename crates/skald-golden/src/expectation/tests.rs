use super::{
    compare::{compare_exit, find_bytes},
    compare_matchers, compare_stream, MatcherOutcome, StreamMatch, StreamMatcher, StreamMatcherSet,
};
use crate::{
    ExitExpectation, MatchMode, ProcessTermination, ResolvedByteSource, ResolvedStreamExpectation,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

static NEXT_EXPECTATION_FILE: AtomicUsize = AtomicUsize::new(0);

struct ExpectationFile(PathBuf);

impl ExpectationFile {
    fn new(bytes: &[u8]) -> Self {
        let sequence = NEXT_EXPECTATION_FILE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "skald-golden-expectation-{}-{sequence}",
            std::process::id()
        ));
        fs::write(&path, bytes).unwrap();
        Self(path)
    }

    fn missing() -> Self {
        let sequence = NEXT_EXPECTATION_FILE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "skald-golden-missing-expectation-{}-{sequence}",
            std::process::id()
        )))
    }

    fn source(&self) -> ResolvedByteSource {
        ResolvedByteSource::File(self.0.clone())
    }
}

impl Drop for ExpectationFile {
    fn drop(&mut self) {
        if self.0.exists() {
            fs::remove_file(&self.0).unwrap();
        }
    }
}

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

#[test]
fn matcher_sets_must_not_be_empty() {
    let error = StreamMatcherSet::try_from(Vec::new()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "a stream matcher collection must not be empty"
    );
}

#[test]
fn compares_arbitrary_mixed_matchers_independently_in_declaration_order() {
    let actual = b"header\0target-footer";
    let exact = ExpectationFile::new(actual);
    let matchers = StreamMatcherSet::try_from(vec![
        StreamMatcher::named(
            "prefix",
            MatchMode::StartsWith,
            ResolvedByteSource::Inline("header".into()),
        ),
        StreamMatcher::named(
            "shared prefix bytes",
            MatchMode::Contains,
            ResolvedByteSource::Inline("header".into()),
        ),
        StreamMatcher::new(
            MatchMode::Contains,
            ResolvedByteSource::Inline("target".into()),
        ),
        StreamMatcher::named("binary exact", MatchMode::Exact, exact.source()),
    ])
    .unwrap();

    let comparison = compare_matchers(&matchers, actual);

    assert!(comparison.passed());
    assert_eq!(comparison.actual(), actual);
    assert_eq!(comparison.outcomes().len(), 4);
    let expected = [
        (0, Some("prefix"), MatchMode::StartsWith, 0, 6),
        (1, Some("shared prefix bytes"), MatchMode::Contains, 0, 6),
        (2, None, MatchMode::Contains, 7, 6),
        (3, Some("binary exact"), MatchMode::Exact, 0, actual.len()),
    ];
    for (outcome, expected) in comparison.outcomes().iter().zip(expected) {
        let MatcherOutcome::Matched(result) = outcome else {
            panic!("expected a successful matcher, got {outcome:?}");
        };
        assert_eq!(
            (
                result.index(),
                result.name(),
                result.mode(),
                result.offset(),
                result.expected_length(),
            ),
            expected
        );
    }
}

#[test]
fn retains_every_mismatch_and_load_failure_without_short_circuiting() {
    let first_missing = ExpectationFile::missing();
    let second_missing = ExpectationFile::missing();
    let matchers = StreamMatcherSet::try_from(vec![
        StreamMatcher::named(
            "wrong exact value",
            MatchMode::Exact,
            ResolvedByteSource::Inline("expected".into()),
        ),
        StreamMatcher::new(
            MatchMode::Contains,
            ResolvedByteSource::Inline("absent".into()),
        ),
        StreamMatcher::named("missing one", MatchMode::Contains, first_missing.source()),
        StreamMatcher::named(
            "missing two",
            MatchMode::StartsWith,
            second_missing.source(),
        ),
    ])
    .unwrap();

    let comparison = compare_matchers(&matchers, b"actual");

    assert!(!comparison.passed());
    assert_eq!(comparison.actual(), b"actual");
    assert_eq!(comparison.outcomes().len(), 4);
    let MatcherOutcome::Mismatched(first) = &comparison.outcomes()[0] else {
        panic!("expected the exact matcher to fail");
    };
    assert_eq!(first.index(), 0);
    assert_eq!(first.name(), Some("wrong exact value"));
    assert_eq!(first.mode(), MatchMode::Exact);
    assert_eq!(first.expected(), b"expected");
    let MatcherOutcome::Mismatched(second) = &comparison.outcomes()[1] else {
        panic!("expected the contains matcher to fail");
    };
    assert_eq!(second.index(), 1);
    assert_eq!(second.name(), None);
    assert_eq!(second.expected(), b"absent");
    for (outcome, expected) in comparison.outcomes()[2..]
        .iter()
        .zip([&first_missing.0, &second_missing.0])
    {
        let MatcherOutcome::LoadFailed(failure) = outcome else {
            panic!("expected an independent load failure");
        };
        assert_eq!(failure.path(), expected);
        assert!(failure.to_string().contains("could not read"));
        assert!(failure.source_message().is_some());
    }
}

#[test]
fn exact_empty_and_binary_partial_matchers_remain_byte_exact() {
    let empty = StreamMatcherSet::one(StreamMatcher::new(
        MatchMode::Exact,
        ResolvedByteSource::Inline(String::new()),
    ));
    assert!(compare_matchers(&empty, b"").passed());
    assert!(!compare_matchers(&empty, b"nonempty").passed());

    let binary = ExpectationFile::new(b"\0target\xff");
    let matcher = StreamMatcherSet::one(StreamMatcher::new(MatchMode::Contains, binary.source()));
    let comparison = compare_matchers(&matcher, b"before\0target\xffafter");
    let MatcherOutcome::Matched(result) = &comparison.outcomes()[0] else {
        panic!("expected binary bytes to match");
    };
    assert_eq!(result.offset(), 6);
    assert_eq!(result.expected_length(), 8);
}

#[test]
fn singular_comparison_preserves_success_mismatch_ignore_and_load_errors() {
    let matched = ResolvedStreamExpectation::Match {
        mode: MatchMode::Contains,
        expected: ResolvedByteSource::Inline("target".into()),
    };
    assert!(matches!(
        compare_stream(&matched, b"before target").unwrap(),
        Ok(StreamMatch::Matched { offset: 7, .. })
    ));

    let mismatch = compare_stream(&matched, b"actual").unwrap().unwrap_err();
    assert_eq!(mismatch.mode(), MatchMode::Contains);
    assert_eq!(mismatch.expected(), b"target");
    assert_eq!(mismatch.actual(), b"actual");

    assert_eq!(
        compare_stream(&ResolvedStreamExpectation::Ignore, b"anything").unwrap(),
        Ok(StreamMatch::Ignored)
    );

    let missing = ExpectationFile::missing();
    let unloaded = ResolvedStreamExpectation::Match {
        mode: MatchMode::Exact,
        expected: missing.source(),
    };
    let error = compare_stream(&unloaded, b"").unwrap_err();
    assert_eq!(error.path(), missing.0);
}
