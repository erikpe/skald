//! Compiler-known standard-library module identities.

use super::{graph::CompilerDependencyKind, ModulePath};

/// A standard-library module whose identity has compiler-defined meaning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CanonicalModule {
    String,
    Iteration,
    Operators,
    Range,
    Error,
    Io,
    F64,
}

impl CanonicalModule {
    pub(crate) const fn path_str(self) -> &'static str {
        match self {
            Self::String => "std::str",
            Self::Iteration => "std::iter",
            Self::Operators => "std::ops",
            Self::Range => "std::range",
            Self::Error => "std::error",
            Self::Io => "std::io",
            Self::F64 => "std::f64",
        }
    }

    pub(crate) fn path(self) -> ModulePath {
        ModulePath::try_from(self.path_str()).expect("canonical module path must be valid")
    }
}

impl CompilerDependencyKind {
    pub(crate) const fn canonical_module(self) -> CanonicalModule {
        match self {
            Self::StringLiteral => CanonicalModule::String,
            Self::GeneralIteration => CanonicalModule::Iteration,
            Self::RangeForSource => CanonicalModule::Range,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_paths_and_compiler_dependencies_are_exact() {
        assert_eq!(CanonicalModule::String.path_str(), "std::str");
        assert_eq!(CanonicalModule::Iteration.path_str(), "std::iter");
        assert_eq!(CanonicalModule::Operators.path_str(), "std::ops");
        assert_eq!(CanonicalModule::Range.path_str(), "std::range");
        assert_eq!(CanonicalModule::Error.path_str(), "std::error");
        assert_eq!(CanonicalModule::Io.path_str(), "std::io");
        assert_eq!(CanonicalModule::F64.path_str(), "std::f64");

        assert_eq!(
            CompilerDependencyKind::StringLiteral.canonical_module(),
            CanonicalModule::String
        );
        assert_eq!(
            CompilerDependencyKind::GeneralIteration.canonical_module(),
            CanonicalModule::Iteration
        );
        assert_eq!(
            CompilerDependencyKind::RangeForSource.canonical_module(),
            CanonicalModule::Range
        );
    }
}
