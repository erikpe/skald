use crate::MatchMode;

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
    pub(super) fn new(mode: MatchMode, expected: Vec<u8>, actual: &[u8]) -> Self {
        Self {
            mode,
            expected,
            actual: actual.to_vec(),
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
