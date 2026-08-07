use crate::{MatchMode, ResolvedByteSource};
use std::{
    fmt, io,
    path::{Path, PathBuf},
};

/// One independently evaluated byte matcher for a captured process stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamMatcher {
    name: Option<String>,
    mode: MatchMode,
    expected: ResolvedByteSource,
}

impl StreamMatcher {
    pub fn new(mode: MatchMode, expected: ResolvedByteSource) -> Self {
        Self {
            name: None,
            mode,
            expected,
        }
    }

    pub fn named(name: impl Into<String>, mode: MatchMode, expected: ResolvedByteSource) -> Self {
        Self {
            name: Some(name.into()),
            mode,
            expected,
        }
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn mode(&self) -> MatchMode {
        self.mode
    }

    pub fn expected(&self) -> &ResolvedByteSource {
        &self.expected
    }
}

/// A validated nonempty collection of independent stream matchers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamMatcherSet {
    matchers: Vec<StreamMatcher>,
}

impl StreamMatcherSet {
    pub fn one(matcher: StreamMatcher) -> Self {
        Self {
            matchers: vec![matcher],
        }
    }

    pub fn matchers(&self) -> &[StreamMatcher] {
        &self.matchers
    }
}

impl TryFrom<Vec<StreamMatcher>> for StreamMatcherSet {
    type Error = EmptyStreamMatcherSet;

    fn try_from(matchers: Vec<StreamMatcher>) -> Result<Self, Self::Error> {
        if matchers.is_empty() {
            Err(EmptyStreamMatcherSet)
        } else {
            Ok(Self { matchers })
        }
    }
}

/// Construction error for an empty matcher collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyStreamMatcherSet;

impl fmt::Display for EmptyStreamMatcherSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a stream matcher collection must not be empty")
    }
}

impl std::error::Error for EmptyStreamMatcherSet {}

/// One successful matcher outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatcherMatch {
    index: usize,
    name: Option<String>,
    mode: MatchMode,
    offset: usize,
    expected_length: usize,
}

impl MatcherMatch {
    pub(super) fn new(
        index: usize,
        matcher: &StreamMatcher,
        offset: usize,
        expected_length: usize,
    ) -> Self {
        Self {
            index,
            name: matcher.name.clone(),
            mode: matcher.mode,
            offset,
            expected_length,
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn mode(&self) -> MatchMode {
        self.mode
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn expected_length(&self) -> usize {
        self.expected_length
    }
}

/// One matcher whose expected bytes did not match the captured stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatcherMismatch {
    index: usize,
    name: Option<String>,
    mode: MatchMode,
    expected: Vec<u8>,
}

impl MatcherMismatch {
    pub(super) fn new(index: usize, matcher: &StreamMatcher, expected: Vec<u8>) -> Self {
        Self {
            index,
            name: matcher.name.clone(),
            mode: matcher.mode,
            expected,
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn mode(&self) -> MatchMode {
        self.mode
    }

    pub fn expected(&self) -> &[u8] {
        &self.expected
    }

    pub(super) fn into_expected(self) -> Vec<u8> {
        self.expected
    }
}

/// One matcher whose external expected bytes could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatcherLoadFailure {
    index: usize,
    name: Option<String>,
    mode: MatchMode,
    path: PathBuf,
    message: String,
    source: Option<(io::ErrorKind, String)>,
}

impl MatcherLoadFailure {
    pub(super) fn new(
        index: usize,
        matcher: &StreamMatcher,
        error: super::ExpectationError,
    ) -> Self {
        let (path, message, source) = error.into_parts();
        Self {
            index,
            name: matcher.name.clone(),
            mode: matcher.mode,
            path,
            message,
            source,
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn mode(&self) -> MatchMode {
        self.mode
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn source_message(&self) -> Option<&str> {
        self.source.as_ref().map(|(_, message)| message.as_str())
    }

    pub(super) fn into_error(self) -> super::ExpectationError {
        super::ExpectationError::from_parts(self.path, self.message, self.source)
    }
}

impl fmt::Display for MatcherLoadFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path.display(), self.message)?;
        if let Some((_, source)) = &self.source {
            write!(formatter, ": {source}")?;
        }
        Ok(())
    }
}

/// The result of evaluating one matcher against a captured stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatcherOutcome {
    Matched(MatcherMatch),
    Mismatched(MatcherMismatch),
    LoadFailed(MatcherLoadFailure),
}

impl MatcherOutcome {
    pub fn index(&self) -> usize {
        match self {
            Self::Matched(result) => result.index(),
            Self::Mismatched(result) => result.index(),
            Self::LoadFailed(result) => result.index(),
        }
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Matched(result) => result.name(),
            Self::Mismatched(result) => result.name(),
            Self::LoadFailed(result) => result.name(),
        }
    }

    pub fn mode(&self) -> MatchMode {
        match self {
            Self::Matched(result) => result.mode(),
            Self::Mismatched(result) => result.mode(),
            Self::LoadFailed(result) => result.mode(),
        }
    }

    pub fn passed(&self) -> bool {
        matches!(self, Self::Matched(_))
    }
}

/// All ordered matcher outcomes for one captured stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamComparison {
    actual: Vec<u8>,
    outcomes: Vec<MatcherOutcome>,
}

impl StreamComparison {
    pub(super) fn new(actual: &[u8], outcomes: Vec<MatcherOutcome>) -> Self {
        Self {
            actual: actual.to_vec(),
            outcomes,
        }
    }

    pub fn actual(&self) -> &[u8] {
        &self.actual
    }

    pub fn outcomes(&self) -> &[MatcherOutcome] {
        &self.outcomes
    }

    pub fn passed(&self) -> bool {
        self.outcomes.iter().all(MatcherOutcome::passed)
    }

    pub(super) fn into_parts(self) -> (Vec<u8>, Vec<MatcherOutcome>) {
        (self.actual, self.outcomes)
    }
}

/// A successful byte-stream comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamMatch {
    Ignored,
    Matched { mode: MatchMode, offset: usize },
}

/// A byte-stream mismatch with complete data retained for later reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamMismatch {
    mode: MatchMode,
    expected: Vec<u8>,
    actual: Vec<u8>,
}

impl StreamMismatch {
    pub(super) fn from_owned(mode: MatchMode, expected: Vec<u8>, actual: Vec<u8>) -> Self {
        Self {
            mode,
            expected,
            actual,
        }
    }

    pub fn mode(&self) -> MatchMode {
        self.mode
    }

    pub fn expected(&self) -> &[u8] {
        &self.expected
    }

    pub fn actual(&self) -> &[u8] {
        &self.actual
    }
}
