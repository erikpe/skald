//! Structured failures from executable-definition retention preparation.

use std::fmt;

use crate::identity::StaticInitializerId;

/// A defensive failure found before any definition container is consumed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MirDefinitionRetentionError {
    /// Static activation is always observable, so its body may never be
    /// removed by target-independent whole-world retention.
    UnreachableStaticInitializer(StaticInitializerId),
}

impl fmt::Display for MirDefinitionRetentionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnreachableStaticInitializer(initializer) => write!(
                formatter,
                "static initializer {initializer} is not reachable from final-MIR roots"
            ),
        }
    }
}

impl std::error::Error for MirDefinitionRetentionError {}
