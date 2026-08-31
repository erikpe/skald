//! Structured failures at the verified preliminary-MIR activation boundary.

use std::fmt;

use crate::{identity::StaticInitializerId, passes::reachability::MirDependencyExtractionError};

use super::StaticActivationNode;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StaticActivationAnalysisError {
    Dependency(MirDependencyExtractionError),
    UnknownStaticInitializer(StaticInitializerId),
    MissingWitness(StaticActivationNode),
}

impl fmt::Display for StaticActivationAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dependency(error) => error.fmt(formatter),
            Self::UnknownStaticInitializer(initializer) => {
                write!(formatter, "unknown static initializer {initializer}")
            }
            Self::MissingWitness(node) => {
                write!(formatter, "activation node {node:?} has no witness")
            }
        }
    }
}

impl std::error::Error for StaticActivationAnalysisError {}

impl From<MirDependencyExtractionError> for StaticActivationAnalysisError {
    fn from(error: MirDependencyExtractionError) -> Self {
        Self::Dependency(error)
    }
}
