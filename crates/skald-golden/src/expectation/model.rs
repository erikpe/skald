use crate::MatchMode;
use std::{
    fmt, io,
    path::{Path, PathBuf},
};

/// One independently evaluated byte matcher for a captured process stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamMatcher<S> {
    name: Option<String>,
    mode: MatchMode,
    expected: S,
}

impl<S> StreamMatcher<S> {
    pub fn new(mode: MatchMode, expected: S) -> Self {
        Self {
            name: None,
            mode,
            expected,
        }
    }

    pub fn named(name: impl Into<String>, mode: MatchMode, expected: S) -> Self {
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

    pub fn expected(&self) -> &S {
        &self.expected
    }
}

/// A validated nonempty collection of independent stream matchers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamMatcherSet<S> {
    matchers: Vec<StreamMatcher<S>>,
}

impl<S> StreamMatcherSet<S> {
    pub fn one(matcher: StreamMatcher<S>) -> Self {
        Self {
            matchers: vec![matcher],
        }
    }

    pub fn matchers(&self) -> &[StreamMatcher<S>] {
        &self.matchers
    }
}

impl<S> TryFrom<Vec<StreamMatcher<S>>> for StreamMatcherSet<S> {
    type Error = EmptyStreamMatcherSet;

    fn try_from(matchers: Vec<StreamMatcher<S>>) -> Result<Self, Self::Error> {
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
    pub(super) fn new<S>(
        index: usize,
        matcher: &StreamMatcher<S>,
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
    pub(super) fn new<S>(index: usize, matcher: &StreamMatcher<S>, expected: Vec<u8>) -> Self {
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
    pub(super) fn new<S>(
        index: usize,
        matcher: &StreamMatcher<S>,
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
    ignored: bool,
    outcomes: Vec<MatcherOutcome>,
}

impl StreamComparison {
    pub(super) fn new(actual: &[u8], outcomes: Vec<MatcherOutcome>) -> Self {
        Self {
            actual: actual.to_vec(),
            ignored: false,
            outcomes,
        }
    }

    pub(super) fn new_ignored(actual: &[u8]) -> Self {
        Self {
            actual: actual.to_vec(),
            ignored: true,
            outcomes: Vec::new(),
        }
    }

    pub fn actual(&self) -> &[u8] {
        &self.actual
    }

    pub fn outcomes(&self) -> &[MatcherOutcome] {
        &self.outcomes
    }

    pub fn is_ignored(&self) -> bool {
        self.ignored
    }

    pub fn passed(&self) -> bool {
        self.outcomes.iter().all(MatcherOutcome::passed)
    }
}
