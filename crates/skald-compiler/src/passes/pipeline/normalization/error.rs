use std::fmt;

use crate::{
    identity::CallableId,
    mir::{
        rewrite::MirRewriteError, BlockId, MirStorageKind, MirType, MirVerificationErrors,
        PathConditionId, StorageId,
    },
};

/// Defensive failure while consuming proof-only final-MIR provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline) struct MirProofNormalizationError {
    pub(super) kind: Box<MirProofNormalizationErrorKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum MirProofNormalizationErrorKind {
    InvalidPathConditionIdentity {
        callable: CallableId,
        index: usize,
        actual: PathConditionId,
    },
    ForeignActivationStorage {
        callable: CallableId,
        condition: PathConditionId,
        storage: StorageId,
    },
    UnknownActivationStorage {
        callable: CallableId,
        condition: PathConditionId,
        storage: StorageId,
    },
    UnexpectedNormalizedActivationStorage {
        callable: CallableId,
        storage: StorageId,
    },
    InvalidActivationStorage {
        callable: CallableId,
        condition: PathConditionId,
        storage: StorageId,
        kind: MirStorageKind,
        ty: MirType,
    },
    DuplicateActivationStorage {
        callable: CallableId,
        storage: StorageId,
    },
    OrphanPathConditionStorage {
        callable: CallableId,
        storage: StorageId,
    },
    ForeignPathReadCondition {
        callable: CallableId,
        block: BlockId,
        instruction: usize,
        condition: PathConditionId,
    },
    UnknownPathReadCondition {
        callable: CallableId,
        block: BlockId,
        instruction: usize,
        condition: PathConditionId,
    },
    PathReadActivationMismatch {
        callable: CallableId,
        block: BlockId,
        instruction: usize,
        condition: PathConditionId,
        expected: StorageId,
        actual: StorageId,
    },
    Rewrite(MirRewriteError),
    NormalizedInvariant(MirVerificationErrors),
}

impl fmt::Display for MirProofNormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind.as_ref() {
            MirProofNormalizationErrorKind::InvalidPathConditionIdentity {
                callable,
                index,
                actual,
            } => write!(
                formatter,
                "callable {callable} path-condition slot {index} contains {actual}"
            ),
            MirProofNormalizationErrorKind::ForeignActivationStorage {
                callable,
                condition,
                storage,
            } => write!(
                formatter,
                "callable {callable} path condition {condition} uses foreign activation storage {storage}"
            ),
            MirProofNormalizationErrorKind::UnknownActivationStorage {
                callable,
                condition,
                storage,
            } => write!(
                formatter,
                "callable {callable} path condition {condition} uses undeclared activation storage {storage}"
            ),
            MirProofNormalizationErrorKind::UnexpectedNormalizedActivationStorage {
                callable,
                storage,
            } => write!(
                formatter,
                "callable {callable} contains already-normalized path activation storage {storage} before proof normalization"
            ),
            MirProofNormalizationErrorKind::InvalidActivationStorage {
                callable,
                condition,
                storage,
                kind,
                ty,
            } => write!(
                formatter,
                "callable {callable} path condition {condition} requires boolean PathCondition storage, but {storage} is {kind:?} {ty}"
            ),
            MirProofNormalizationErrorKind::DuplicateActivationStorage {
                callable,
                storage,
            } => write!(
                formatter,
                "callable {callable} assigns activation storage {storage} to more than one path condition"
            ),
            MirProofNormalizationErrorKind::OrphanPathConditionStorage { callable, storage } => write!(
                formatter,
                "callable {callable} has path-condition storage {storage} without an owning path condition"
            ),
            MirProofNormalizationErrorKind::ForeignPathReadCondition {
                callable,
                block,
                instruction,
                condition,
            } => write!(
                formatter,
                "callable {callable} block {block} instruction {instruction} reads foreign path condition {condition}"
            ),
            MirProofNormalizationErrorKind::UnknownPathReadCondition {
                callable,
                block,
                instruction,
                condition,
            } => write!(
                formatter,
                "callable {callable} block {block} instruction {instruction} reads undeclared path condition {condition}"
            ),
            MirProofNormalizationErrorKind::PathReadActivationMismatch {
                callable,
                block,
                instruction,
                condition,
                expected,
                actual,
            } => write!(
                formatter,
                "callable {callable} block {block} instruction {instruction} reads path condition {condition} through {actual}, expected {expected}"
            ),
            MirProofNormalizationErrorKind::Rewrite(error) => {
                write!(formatter, "proof normalization rewrite failed: {error}")
            }
            MirProofNormalizationErrorKind::NormalizedInvariant(errors) => {
                write!(formatter, "normalized MIR verification failed: {errors}")
            }
        }
    }
}

impl std::error::Error for MirProofNormalizationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind.as_ref() {
            MirProofNormalizationErrorKind::Rewrite(error) => Some(error),
            MirProofNormalizationErrorKind::NormalizedInvariant(errors) => Some(errors),
            MirProofNormalizationErrorKind::InvalidPathConditionIdentity { .. }
            | MirProofNormalizationErrorKind::ForeignActivationStorage { .. }
            | MirProofNormalizationErrorKind::UnknownActivationStorage { .. }
            | MirProofNormalizationErrorKind::UnexpectedNormalizedActivationStorage { .. }
            | MirProofNormalizationErrorKind::InvalidActivationStorage { .. }
            | MirProofNormalizationErrorKind::DuplicateActivationStorage { .. }
            | MirProofNormalizationErrorKind::OrphanPathConditionStorage { .. }
            | MirProofNormalizationErrorKind::ForeignPathReadCondition { .. }
            | MirProofNormalizationErrorKind::UnknownPathReadCondition { .. }
            | MirProofNormalizationErrorKind::PathReadActivationMismatch { .. } => None,
        }
    }
}

impl From<MirProofNormalizationErrorKind> for MirProofNormalizationError {
    fn from(kind: MirProofNormalizationErrorKind) -> Self {
        Self {
            kind: Box::new(kind),
        }
    }
}

impl From<MirRewriteError> for MirProofNormalizationError {
    fn from(error: MirRewriteError) -> Self {
        MirProofNormalizationErrorKind::Rewrite(error).into()
    }
}
