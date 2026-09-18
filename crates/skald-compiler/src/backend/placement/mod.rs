//! Unchecked, exact-snapshot placement data. Structural validation is not authority.
//! See `docs/compiler/PLACEMENT_CHECKING.md` for the independent checker contract.
#[cfg_attr(not(test), allow(dead_code))]
mod model;
#[cfg_attr(not(test), allow(dead_code))]
mod structure;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use model::{
    Assignment, Location, PlacementDraft, PlacementError, Site, Storage, StorageId,
    StorageLifetime, StoragePurpose, Transfer, TransferKind, TransferPoint, TransferValue,
};

#[cfg(test)]
pub(super) mod tests;
