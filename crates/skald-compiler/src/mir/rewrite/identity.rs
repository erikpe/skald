use std::{convert::Infallible, fmt};

use crate::identity::CallableId;

use super::super::{BlockId, OptionalGuardId, PathConditionId, StorageId, ValueId};

/// A deterministic structural location in one callable-owned MIR definition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirLocalIdentitySite {
    ReturnStorage,
    Receiver,
    Parameter(usize),
    StaticPublicationInitializationExit,
    StaticPublicationCleanupEntry,
    StorageDeclaration(usize),
    ValueDeclaration(usize),
    BodyEntry,
    BlockDeclaration(usize),
    Instruction { block: usize, instruction: usize },
    Terminator(usize),
    PathCondition(usize),
    LogicalExpression(usize),
}

impl fmt::Display for MirLocalIdentitySite {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReturnStorage => formatter.write_str("return storage"),
            Self::Receiver => formatter.write_str("receiver"),
            Self::Parameter(index) => write!(formatter, "parameter {index}"),
            Self::StaticPublicationInitializationExit => {
                formatter.write_str("static publication initialization exit")
            }
            Self::StaticPublicationCleanupEntry => {
                formatter.write_str("static publication cleanup entry")
            }
            Self::StorageDeclaration(index) => write!(formatter, "storage declaration {index}"),
            Self::ValueDeclaration(index) => write!(formatter, "value declaration {index}"),
            Self::BodyEntry => formatter.write_str("body entry"),
            Self::BlockDeclaration(index) => write!(formatter, "block declaration {index}"),
            Self::Instruction { block, instruction } => {
                write!(formatter, "instruction {instruction} in block {block}")
            }
            Self::Terminator(block) => write!(formatter, "terminator in block {block}"),
            Self::PathCondition(index) => write!(formatter, "path condition {index}"),
            Self::LogicalExpression(index) => write!(formatter, "logical expression {index}"),
        }
    }
}

/// One of the five callable-owned identities covered by MIR rewriting.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirLocalIdentity {
    Storage(StorageId),
    Value(ValueId),
    Block(BlockId),
    PathCondition(PathConditionId),
    OptionalGuard(OptionalGuardId),
}

impl MirLocalIdentity {
    pub(crate) const fn callable(self) -> CallableId {
        match self {
            Self::Storage(identity) => identity.callable(),
            Self::Value(identity) => identity.callable(),
            Self::Block(identity) => identity.callable(),
            Self::PathCondition(identity) => identity.callable(),
            Self::OptionalGuard(identity) => identity.callable(),
        }
    }
}

impl fmt::Display for MirLocalIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(identity) => write!(formatter, "storage {identity}"),
            Self::Value(identity) => write!(formatter, "value {identity}"),
            Self::Block(identity) => write!(formatter, "block {identity}"),
            Self::PathCondition(identity) => write!(formatter, "path condition {identity}"),
            Self::OptionalGuard(identity) => write!(formatter, "optional guard {identity}"),
        }
    }
}

/// Mutation hook used by the exhaustive MIR traversal.
///
/// Defaults preserve identities, so a pass can override only the identity
/// families it intends to inspect or rewrite.
pub(crate) trait MirLocalIdentityMapper {
    type Error;

    fn map_storage(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: StorageId,
    ) -> Result<StorageId, Self::Error> {
        Ok(identity)
    }

    fn map_value(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: ValueId,
    ) -> Result<ValueId, Self::Error> {
        Ok(identity)
    }

    fn map_block(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: BlockId,
    ) -> Result<BlockId, Self::Error> {
        Ok(identity)
    }

    fn map_path_condition(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: PathConditionId,
    ) -> Result<PathConditionId, Self::Error> {
        Ok(identity)
    }

    fn map_optional_guard(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: OptionalGuardId,
    ) -> Result<OptionalGuardId, Self::Error> {
        Ok(identity)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PreserveLocalIdentities;

impl MirLocalIdentityMapper for PreserveLocalIdentities {
    type Error = Infallible;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MirLocalIdentityOwnershipError {
    pub expected: CallableId,
    pub identity: MirLocalIdentity,
    pub site: MirLocalIdentitySite,
}

impl fmt::Display for MirLocalIdentityOwnershipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {} belongs to {}, expected {}",
            self.identity,
            self.site,
            self.identity.callable(),
            self.expected
        )
    }
}

impl std::error::Error for MirLocalIdentityOwnershipError {}

pub(super) struct LocalIdentityOwnerValidator {
    expected: CallableId,
}

impl LocalIdentityOwnerValidator {
    pub(super) const fn new(expected: CallableId) -> Self {
        Self { expected }
    }

    fn validate(
        &self,
        site: MirLocalIdentitySite,
        identity: MirLocalIdentity,
    ) -> Result<(), MirLocalIdentityOwnershipError> {
        if identity.callable() == self.expected {
            Ok(())
        } else {
            Err(MirLocalIdentityOwnershipError {
                expected: self.expected,
                identity,
                site,
            })
        }
    }
}

impl MirLocalIdentityMapper for LocalIdentityOwnerValidator {
    type Error = MirLocalIdentityOwnershipError;

    fn map_storage(
        &mut self,
        site: MirLocalIdentitySite,
        identity: StorageId,
    ) -> Result<StorageId, Self::Error> {
        self.validate(site, MirLocalIdentity::Storage(identity))?;
        Ok(identity)
    }

    fn map_value(
        &mut self,
        site: MirLocalIdentitySite,
        identity: ValueId,
    ) -> Result<ValueId, Self::Error> {
        self.validate(site, MirLocalIdentity::Value(identity))?;
        Ok(identity)
    }

    fn map_block(
        &mut self,
        site: MirLocalIdentitySite,
        identity: BlockId,
    ) -> Result<BlockId, Self::Error> {
        self.validate(site, MirLocalIdentity::Block(identity))?;
        Ok(identity)
    }

    fn map_path_condition(
        &mut self,
        site: MirLocalIdentitySite,
        identity: PathConditionId,
    ) -> Result<PathConditionId, Self::Error> {
        self.validate(site, MirLocalIdentity::PathCondition(identity))?;
        Ok(identity)
    }

    fn map_optional_guard(
        &mut self,
        site: MirLocalIdentitySite,
        identity: OptionalGuardId,
    ) -> Result<OptionalGuardId, Self::Error> {
        self.validate(site, MirLocalIdentity::OptionalGuard(identity))?;
        Ok(identity)
    }
}
