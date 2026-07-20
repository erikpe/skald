//! Stable identities shared by name-independent compiler phases.
//!
//! Resolution assigns these identities when source declarations and bindings
//! are selected. Later phases preserve and compare them without depending on
//! resolver implementation details or returning to source names.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionId(usize);

impl FunctionId {
    pub const fn index(self) -> usize {
        self.0
    }

    pub(crate) const fn new(index: usize) -> Self {
        Self(index)
    }
}

impl fmt::Display for FunctionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "f{}", self.index())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ParameterId {
    function: FunctionId,
    index: usize,
}

impl ParameterId {
    pub const fn function(self) -> FunctionId {
        self.function
    }

    pub const fn index(self) -> usize {
        self.index
    }

    pub(crate) const fn new(function: FunctionId, index: usize) -> Self {
        Self { function, index }
    }
}

impl fmt::Display for ParameterId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:p{}", self.function(), self.index())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalId {
    function: FunctionId,
    index: usize,
}

impl LocalId {
    pub const fn function(self) -> FunctionId {
        self.function
    }

    pub const fn index(self) -> usize {
        self.index
    }

    pub(crate) const fn new(function: FunctionId, index: usize) -> Self {
        Self { function, index }
    }
}

impl fmt::Display for LocalId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:l{}", self.function(), self.index())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BindingId {
    Parameter(ParameterId),
    Local(LocalId),
}

impl BindingId {
    pub const fn function(self) -> FunctionId {
        match self {
            Self::Parameter(id) => id.function(),
            Self::Local(id) => id.function(),
        }
    }
}

impl fmt::Display for BindingId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parameter(id) => id.fmt(formatter),
            Self::Local(id) => id.fmt(formatter),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_preserve_owner_index_ordering_and_display() {
        let first = FunctionId::new(2);
        let second = FunctionId::new(3);
        let parameter = ParameterId::new(first, 4);
        let local = LocalId::new(first, 5);

        assert_eq!(first.index(), 2);
        assert!(first < second);
        assert_eq!(parameter.function(), first);
        assert_eq!(parameter.index(), 4);
        assert_eq!(local.function(), first);
        assert_eq!(local.index(), 5);
        assert_eq!(first.to_string(), "f2");
        assert_eq!(parameter.to_string(), "f2:p4");
        assert_eq!(local.to_string(), "f2:l5");
        assert_eq!(BindingId::Parameter(parameter).function(), first);
        assert_eq!(BindingId::Local(local).function(), first);
        assert_eq!(BindingId::Parameter(parameter).to_string(), "f2:p4");
        assert_eq!(BindingId::Local(local).to_string(), "f2:l5");
    }
}
