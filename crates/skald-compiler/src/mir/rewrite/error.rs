use std::fmt;

use crate::identity::{BindingId, CallableId};

use super::{
    super::{MirStorageKind, MirType},
    MirLocalIdentity, MirLocalIdentitySite,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MirReferenceFailure {
    Foreign,
    Unknown,
    Deleted,
}

/// A deterministic internal failure while editing or committing a callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MirRewriteError {
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
    DuplicateValueDefinition {
        value: super::super::ValueId,
        first: MirLocalIdentitySite,
        duplicate: MirLocalIdentitySite,
    },
    MissingValueDefinition {
        value: super::super::ValueId,
    },
    MissingBlockTerminator {
        block: super::super::BlockId,
    },
    StaleCallableSnapshot {
        callable: CallableId,
        subject: &'static str,
    },
    InvalidValueDefinitionSite {
        value: super::super::ValueId,
        site: MirLocalIdentitySite,
    },
    PathParentNotEarlier {
        condition: super::super::PathConditionId,
        parent: super::super::PathConditionId,
    },
    ValueTypeMismatch {
        from: super::super::ValueId,
        from_type: MirType,
        to: super::super::ValueId,
        to_type: MirType,
    },
    StorageTypeMismatch {
        from: super::super::StorageId,
        from_type: MirType,
        to: super::super::StorageId,
        to_type: MirType,
    },
    StorageKindMismatch {
        storage: super::super::StorageId,
        expected: MirStorageKind,
        actual: MirStorageKind,
    },
    ImportSourceMatchesDestination {
        callable: CallableId,
    },
    DuplicateImportIdentity {
        identity: MirLocalIdentity,
    },
    DuplicateImportSubstitution {
        identity: MirLocalIdentity,
    },
    SelectedImportIdentityHasSubstitution {
        identity: MirLocalIdentity,
    },
    MissingImportSubstitution {
        identity: MirLocalIdentity,
        site: MirLocalIdentitySite,
    },
    InvalidImportStorageKind {
        storage: super::super::StorageId,
        kind: MirStorageKind,
    },
    ForeignImportBinding {
        expected: CallableId,
        storage: super::super::StorageId,
        binding: BindingId,
    },
    UnknownImportLogicalRecord {
        source: CallableId,
        index: usize,
    },
    DuplicateImportLogicalRecord {
        source: CallableId,
        index: usize,
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
            Self::DuplicateValueDefinition {
                value,
                first,
                duplicate,
            } => write!(
                formatter,
                "value {value} is defined at both {first} and {duplicate}"
            ),
            Self::MissingValueDefinition { value } => {
                write!(formatter, "value {value} has no executable definition")
            }
            Self::MissingBlockTerminator { block } => {
                write!(formatter, "block {block} has no control-flow terminator")
            }
            Self::StaleCallableSnapshot { callable, subject } => write!(
                formatter,
                "callable {callable} no longer matches the captured {subject} snapshot"
            ),
            Self::InvalidValueDefinitionSite { value, site } => {
                write!(formatter, "value {value} has a non-instruction definition at {site}")
            }
            Self::PathParentNotEarlier { condition, parent } => write!(
                formatter,
                "path condition {condition} requires earlier parent {parent}"
            ),
            Self::ValueTypeMismatch {
                from,
                from_type,
                to,
                to_type,
            } => write!(
                formatter,
                "cannot substitute value {from} ({from_type}) with {to} ({to_type})"
            ),
            Self::StorageTypeMismatch {
                from,
                from_type,
                to,
                to_type,
            } => write!(
                formatter,
                "cannot substitute storage {from} ({from_type}) with {to} ({to_type})"
            ),
            Self::StorageKindMismatch {
                storage,
                expected,
                actual,
            } => write!(
                formatter,
                "storage {storage} has kind {actual:?}, expected {expected:?}"
            ),
            Self::ImportSourceMatchesDestination { callable } => write!(
                formatter,
                "cross-callable import source and destination are both {callable}"
            ),
            Self::DuplicateImportIdentity { identity } => {
                write!(formatter, "{identity} is selected more than once for import")
            }
            Self::DuplicateImportSubstitution { identity } => write!(
                formatter,
                "source-local {identity} has more than one import substitution"
            ),
            Self::SelectedImportIdentityHasSubstitution { identity } => write!(
                formatter,
                "selected import identity {identity} also has a boundary substitution"
            ),
            Self::MissingImportSubstitution { identity, site } => write!(
                formatter,
                "source-local {identity} at {site} is outside the import selection and has no substitution"
            ),
            Self::InvalidImportStorageKind { storage, kind } => write!(
                formatter,
                "imported storage {storage} cannot use destination kind {kind:?}"
            ),
            Self::ForeignImportBinding {
                expected,
                storage,
                binding,
            } => write!(
                formatter,
                "source storage {storage} has binding {binding} owned by {}, expected {expected}",
                binding.callable()
            ),
            Self::UnknownImportLogicalRecord { source, index } => write!(
                formatter,
                "callable {source} has no logical record {index} to import"
            ),
            Self::DuplicateImportLogicalRecord { source, index } => write!(
                formatter,
                "logical record {index} from callable {source} is selected more than once for import"
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
