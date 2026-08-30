//! Exhaustive rewriting of callable-local MIR identities.
//!
//! This module is the single maintenance point for every occurrence of
//! [`StorageId`], [`ValueId`], [`BlockId`], [`PathConditionId`], and
//! [`OptionalGuardId`] in executable MIR and its callable-owned metadata.
//! Adding a new identity-bearing model field requires extending this traversal
//! in the same change. Full destructuring and exhaustive enum matches make
//! omissions compiler errors whenever the Rust model permits that.
//!
//! Program-semantic identities, including source
//! [`BindingId`](crate::identity::BindingId) values, are deliberately outside
//! this traversal.
//!
//! Dense callable tables move into private sparse edit state while a
//! transformation is in progress. Tombstones and explicit order never
//! masquerade as committed MIR. The private commit boundary consumes that
//! state and either returns one canonically compacted common callable with
//! complete maps and change counts or one structured error.

mod commit;
mod edit;
mod error;
mod identity;
mod map;

pub(crate) use identity::{
    MirLocalIdentity, MirLocalIdentityMapper, MirLocalIdentityOwnershipError, MirLocalIdentitySite,
};
pub(crate) use map::{
    map_function_local_identities, map_member_local_identities,
    map_static_initializer_local_identities, validate_function_local_identity_owners,
    validate_member_local_identity_owners, validate_static_initializer_local_identity_owners,
};

#[cfg(test)]
mod tests;
