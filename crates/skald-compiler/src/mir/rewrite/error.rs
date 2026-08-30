use std::fmt;

use crate::identity::CallableId;

use super::{MirLocalIdentity, MirLocalIdentitySite};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MirReferenceFailure {
    Foreign,
    Unknown,
    Deleted,
}

/// A deterministic internal failure while editing or committing a callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum MirRewriteError {
    ForeignIdentity {
        expected: CallableId,
        identity: MirLocalIdentity,
    },
    UnknownIdentity {
        identity: MirLocalIdentity,
    },
    DeletedIdentity {
        identity: MirLocalIdentity,
    },
    DeclarationIdentityMismatch {
        expected: MirLocalIdentity,
        actual: MirLocalIdentity,
    },
    DuplicateOrderIdentity {
        identity: MirLocalIdentity,
    },
    MissingOrderIdentity {
        identity: MirLocalIdentity,
    },
    InvalidReference {
        expected: CallableId,
        identity: MirLocalIdentity,
        site: MirLocalIdentitySite,
        failure: MirReferenceFailure,
    },
    PathParentNotEarlier {
        condition: super::super::PathConditionId,
        parent: super::super::PathConditionId,
    },
    UnknownLogicalRecord {
        index: usize,
    },
    DeletedLogicalRecord {
        index: usize,
    },
    MissingLogicalOrder {
        index: usize,
    },
    DuplicateLogicalOrder {
        index: usize,
    },
}

impl fmt::Display for MirRewriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignIdentity { expected, identity } => write!(
                formatter,
                "{identity} belongs to {}, expected {expected}",
                identity.callable()
            ),
            Self::UnknownIdentity { identity } => {
                write!(formatter, "{identity} does not name an allocated edit slot")
            }
            Self::DeletedIdentity { identity } => {
                write!(formatter, "{identity} names a deleted edit slot")
            }
            Self::DeclarationIdentityMismatch { expected, actual } => write!(
                formatter,
                "declaration identity is {actual}, expected edit slot {expected}"
            ),
            Self::DuplicateOrderIdentity { identity } => {
                write!(formatter, "{identity} appears more than once in live order")
            }
            Self::MissingOrderIdentity { identity } => {
                write!(formatter, "live {identity} is missing from live order")
            }
            Self::InvalidReference {
                expected,
                identity,
                site,
                failure,
            } => match failure {
                MirReferenceFailure::Foreign => write!(
                    formatter,
                    "{identity} at {site} belongs to {}, expected {expected}",
                    identity.callable()
                ),
                MirReferenceFailure::Unknown => write!(
                    formatter,
                    "{identity} at {site} does not name an allocated edit slot"
                ),
                MirReferenceFailure::Deleted => {
                    write!(formatter, "{identity} at {site} names a deleted edit slot")
                }
            },
            Self::PathParentNotEarlier { condition, parent } => write!(
                formatter,
                "path condition {condition} requires earlier parent {parent}"
            ),
            Self::UnknownLogicalRecord { index } => {
                write!(formatter, "logical record {index} was never allocated")
            }
            Self::DeletedLogicalRecord { index } => {
                write!(formatter, "logical record {index} is deleted")
            }
            Self::MissingLogicalOrder { index } => {
                write!(
                    formatter,
                    "live logical record {index} is missing from live order"
                )
            }
            Self::DuplicateLogicalOrder { index } => {
                write!(
                    formatter,
                    "logical record {index} appears more than once in live order"
                )
            }
        }
    }
}

impl std::error::Error for MirRewriteError {}
