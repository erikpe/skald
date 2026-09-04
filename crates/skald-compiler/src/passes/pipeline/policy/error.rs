use std::fmt;

use super::identity::MirPassIdentity;
use super::stage::MirPassStage;

/// One invalid static pass-registry fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum MirPassRegistryError {
    DuplicateIdentity {
        identity: MirPassIdentity,
    },
    DuplicateName {
        name: &'static str,
    },
    InvalidName {
        name: &'static str,
    },
    EmptyDescription {
        identity: MirPassIdentity,
    },
    ImplementationIdentityMismatch {
        descriptor: MirPassIdentity,
        implementation: MirPassIdentity,
    },
    ImplementationStageMismatch {
        identity: MirPassIdentity,
        descriptor: MirPassStage,
        implementation: MirPassStage,
    },
    ReservedNormalizationName,
}

impl fmt::Display for MirPassRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateIdentity { identity } => {
                write!(formatter, "duplicate MIR {identity}")
            }
            Self::DuplicateName { name } => {
                write!(formatter, "duplicate MIR pass name `{name}`")
            }
            Self::InvalidName { name } => write!(
                formatter,
                "invalid MIR pass name `{name}`; expected lowercase kebab-case"
            ),
            Self::EmptyDescription { identity } => {
                write!(formatter, "MIR {identity} has an empty description")
            }
            Self::ImplementationIdentityMismatch {
                descriptor,
                implementation,
            } => write!(
                formatter,
                "MIR pass descriptor {descriptor} is wired to {implementation}"
            ),
            Self::ImplementationStageMismatch {
                identity,
                descriptor,
                implementation,
            } => write!(
                formatter,
                "MIR {identity} has descriptor stage `{descriptor}` but implementation stage `{implementation}`"
            ),
            Self::ReservedNormalizationName => formatter.write_str(
                "mandatory proof-provenance normalization cannot be registered as a selectable MIR pass",
            ),
        }
    }
}

/// Deterministically ordered failures found in one immutable registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirPassRegistryErrors {
    errors: Vec<MirPassRegistryError>,
}

impl MirPassRegistryErrors {
    pub(super) fn new(errors: Vec<MirPassRegistryError>) -> Self {
        Self { errors }
    }

    #[cfg(test)]
    pub(super) fn as_slice(&self) -> &[MirPassRegistryError] {
        &self.errors
    }
}

impl fmt::Display for MirPassRegistryErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid final-MIR pass registry")?;
        for error in &self.errors {
            write!(formatter, ": {error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for MirPassRegistryErrors {}

/// Failure to resolve a requested final-MIR pass schedule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MirPassScheduleError {
    InvalidRegistry(MirPassRegistryErrors),
    UnknownIdentity {
        identity: MirPassIdentity,
    },
    UnknownNames {
        names: Vec<String>,
        known_names: Vec<&'static str>,
    },
    MandatoryNormalizationSelection,
    WrongStageOrder {
        proof_rich: MirPassIdentity,
        position: usize,
    },
}

impl fmt::Display for MirPassScheduleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRegistry(errors) => errors.fmt(formatter),
            Self::UnknownIdentity { identity } => {
                write!(formatter, "unknown MIR {identity}")
            }
            Self::UnknownNames { names, known_names } => {
                write!(
                    formatter,
                    "unknown MIR pass name{}: {}",
                    if names.len() == 1 { "" } else { "s" },
                    names.join(", ")
                )?;
                if known_names.is_empty() {
                    write!(formatter, "; no MIR passes are registered")
                } else {
                    write!(formatter, "; known MIR passes: {}", known_names.join(", "))
                }
            }
            Self::MandatoryNormalizationSelection => formatter.write_str(
                "mandatory proof-provenance normalization cannot be selected, disabled, or repeated",
            ),
            Self::WrongStageOrder {
                proof_rich,
                position,
            } => write!(
                formatter,
                "proof-rich MIR {proof_rich} occurs after the final-stage boundary at schedule position {position}"
            ),
        }
    }
}

impl std::error::Error for MirPassScheduleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidRegistry(errors) => Some(errors),
            Self::UnknownIdentity { .. }
            | Self::UnknownNames { .. }
            | Self::MandatoryNormalizationSelection
            | Self::WrongStageOrder { .. } => None,
        }
    }
}

impl From<MirPassRegistryErrors> for MirPassScheduleError {
    fn from(errors: MirPassRegistryErrors) -> Self {
        Self::InvalidRegistry(errors)
    }
}
