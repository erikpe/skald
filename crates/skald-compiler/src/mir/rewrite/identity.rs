use std::{convert::Infallible, fmt};

use crate::identity::CallableId;

use super::super::{BlockId, OptionalGuardId, PathConditionId, StorageId, ValueId};
use super::{storage_use::MirStorageUseRole, value_use::MirValueUseRole};

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

/// Common behavior of identities that occupy callable-local rewrite slots.
///
/// Keeping this vocabulary beside [`MirLocalIdentity`] lets sparse editing and
/// dense commit share one typed implementation without duplicating identity
/// construction or classification.
pub(crate) trait MirLocalId: Copy + Eq + Ord {
    fn new(owner: CallableId, index: usize) -> Self;
    fn callable(self) -> CallableId;
    fn index(self) -> usize;
    fn local_identity(self) -> MirLocalIdentity;
}

macro_rules! local_id {
    ($identity:ty, $variant:ident) => {
        impl MirLocalId for $identity {
            fn new(owner: CallableId, index: usize) -> Self {
                <$identity>::new(owner, index)
            }

            fn callable(self) -> CallableId {
                self.callable()
            }

            fn index(self) -> usize {
                self.index()
            }

            fn local_identity(self) -> MirLocalIdentity {
                MirLocalIdentity::$variant(self)
            }
        }
    };
}

local_id!(StorageId, Storage);
local_id!(ValueId, Value);
local_id!(BlockId, Block);
local_id!(PathConditionId, PathCondition);
local_id!(OptionalGuardId, OptionalGuard);

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

    /// Maps a value identity at the instruction that defines it.
    ///
    /// Dense compaction treats definitions like every other reference, while
    /// use substitution deliberately preserves them. Mappers which do not
    /// distinguish those operations inherit the ordinary value mapping.
    fn map_value_definition(
        &mut self,
        site: MirLocalIdentitySite,
        identity: ValueId,
    ) -> Result<ValueId, Self::Error> {
        self.map_value(site, identity)
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

/// Read-only hook used by the exhaustive MIR identity traversal.
///
/// Defaults ignore identities, so an analysis can observe only the identity
/// families it needs. The structural traversal is shared with
/// [`MirLocalIdentityMapper`]; adding an identity-bearing MIR field therefore
/// remains a single compile-time-checked maintenance task.
pub(crate) trait MirLocalIdentityObserver {
    type Error;

    fn observe_storage(
        &mut self,
        _site: MirLocalIdentitySite,
        _identity: StorageId,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Observes one semantically classified use of callable-local storage.
    ///
    /// Identity-only analyses inherit ordinary storage observation. Analyses
    /// which reason about storage contents override this hook so every
    /// storage-bearing MIR position remains an explicit, closed decision.
    fn observe_storage_use(
        &mut self,
        site: MirLocalIdentitySite,
        _role: MirStorageUseRole,
        identity: StorageId,
    ) -> Result<(), Self::Error> {
        self.observe_storage(site, identity)
    }

    fn observe_value(
        &mut self,
        _site: MirLocalIdentitySite,
        _identity: ValueId,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Observes a semantic use of a transient value.
    ///
    /// Analyses interested only in identity coverage inherit ordinary value
    /// observation. Use-site analyses override this hook to retain the closed
    /// semantic role selected by the exhaustive traversal.
    fn observe_value_use(
        &mut self,
        site: MirLocalIdentitySite,
        _role: MirValueUseRole,
        identity: ValueId,
    ) -> Result<(), Self::Error> {
        self.observe_value(site, identity)
    }

    /// Observes a value identity at the instruction that defines it.
    ///
    /// Analyses which do not distinguish definitions from uses inherit the
    /// ordinary value observation.
    fn observe_value_definition(
        &mut self,
        site: MirLocalIdentitySite,
        identity: ValueId,
    ) -> Result<(), Self::Error> {
        self.observe_value(site, identity)
    }

    fn observe_block(
        &mut self,
        _site: MirLocalIdentitySite,
        _identity: BlockId,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn observe_path_condition(
        &mut self,
        _site: MirLocalIdentitySite,
        _identity: PathConditionId,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn observe_optional_guard(
        &mut self,
        _site: MirLocalIdentitySite,
        _identity: OptionalGuardId,
    ) -> Result<(), Self::Error> {
        Ok(())
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

impl MirLocalIdentityObserver for LocalIdentityOwnerValidator {
    type Error = MirLocalIdentityOwnershipError;

    fn observe_storage(
        &mut self,
        site: MirLocalIdentitySite,
        identity: StorageId,
    ) -> Result<(), Self::Error> {
        self.validate(site, MirLocalIdentity::Storage(identity))
    }

    fn observe_value(
        &mut self,
        site: MirLocalIdentitySite,
        identity: ValueId,
    ) -> Result<(), Self::Error> {
        self.validate(site, MirLocalIdentity::Value(identity))
    }

    fn observe_block(
        &mut self,
        site: MirLocalIdentitySite,
        identity: BlockId,
    ) -> Result<(), Self::Error> {
        self.validate(site, MirLocalIdentity::Block(identity))
    }

    fn observe_path_condition(
        &mut self,
        site: MirLocalIdentitySite,
        identity: PathConditionId,
    ) -> Result<(), Self::Error> {
        self.validate(site, MirLocalIdentity::PathCondition(identity))
    }

    fn observe_optional_guard(
        &mut self,
        site: MirLocalIdentitySite,
        identity: OptionalGuardId,
    ) -> Result<(), Self::Error> {
        self.validate(site, MirLocalIdentity::OptionalGuard(identity))
    }
}
