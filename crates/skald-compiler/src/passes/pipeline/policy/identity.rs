use std::fmt;

/// Compiler-owned identity of one target-independent final-MIR pass.
///
/// The numeric representation is private. Stable external selection uses the
/// descriptor name rather than this value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MirPassIdentity(u16);

impl MirPassIdentity {
    pub(super) const fn new(value: u16) -> Self {
        Self(value)
    }
}

impl fmt::Display for MirPassIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "pass identity {}", self.0)
    }
}
