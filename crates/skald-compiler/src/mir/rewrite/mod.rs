//! Exhaustive observation and rewriting of callable-local MIR identities.
//!
//! This module is the single maintenance point for every occurrence of
//! [`StorageId`], [`ValueId`], [`BlockId`], [`PathConditionId`], and
//! [`OptionalGuardId`] in executable MIR and its callable-owned metadata.
//! Adding a new identity-bearing model field requires extending this traversal
//! in the same change. Its shared structural kernel produces both immutable
//! observation and mutable mapping, so full destructuring and exhaustive enum
//! matches give both behaviors the same compile-time coverage.
//!
//! Program-semantic identities, including source
//! [`BindingId`](crate::identity::BindingId) values, are deliberately outside
//! this traversal.
//!
//! Dense callable tables move into private sparse edit state while a
//! transformation is in progress. The supported crate-private facade is
//! [`rewrite_program`], [`MirCallableEdit`], and their typed result and error
//! vocabulary. Passes use explicit lookup, allocation, removal, substitution,
//! instruction, terminator, edge, and cross-callable import operations; sparse
//! slots and compaction remain implementation details. Helpers never infer
//! semantic cascading deletion of liveness or proof metadata.

mod callable;
mod census;
mod cfg;
mod commit;
mod edit;
mod error;
mod identity;
mod import;
mod map;
mod program;
mod value_use;

pub(crate) use census::{value_use_census_for_definition, MirValueCensusEntry, MirValueUseCensus};
pub(crate) use cfg::{
    final_cfg_facts_for_definition, local_cfg_facts_for_definition, MirLocalCfgBlockFacts,
    MirLocalCfgEdge, MirLocalCfgFacts, MirLocalCfgTerminatorKind, MirProtectedBlockRoot,
};
pub(crate) use commit::{
    MirCommitMap, MirCommitMaps, MirEntityChangeCount, MirRewriteChangeSummary,
};
pub(crate) use edit::{BlockPlacement, LogicalRecordIndex, MirCallableEdit};
pub(crate) use error::{MirReferenceFailure, MirRewriteError};

pub(crate) use identity::{
    MirLocalIdentity, MirLocalIdentityMapper, MirLocalIdentityObserver,
    MirLocalIdentityOwnershipError, MirLocalIdentitySite,
};
pub(crate) use import::{
    MirImportMap, MirImportMaps, MirImportRequest, MirImportResult, MirImportSource,
};
pub(crate) use map::{
    map_function_local_identities, map_member_local_identities,
    map_static_initializer_local_identities, validate_function_local_identity_owners,
    validate_member_local_identity_owners, validate_static_initializer_local_identity_owners,
};
pub(crate) use program::{rewrite_program, MirCallableRewriteResult, MirProgramRewriteResult};
pub(crate) use value_use::{
    value_use_sites_for_definition, MirCallValueUse, MirScalarValueUse, MirValueUseRole,
    MirValueUseSite, MirValueUseSites,
};

#[cfg(test)]
mod tests;
