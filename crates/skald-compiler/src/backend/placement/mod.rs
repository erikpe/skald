//! Exact-snapshot drafts and independent checked-placement authority.
//! Structural validation alone cannot authorize realization.
//! See `docs/compiler/PLACEMENT_CHECKING.md` for the independent checker contract.
mod baseline;
mod check;
mod legality;
mod model;
mod requirements;
mod state;
mod structure;
mod target;
mod transfers;

pub(in crate::backend) use baseline::place_baseline;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use check::{check_placement, CheckedPlacement};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use requirements::{CheckFailure, CheckLocation, CheckReason};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use target::{PlacementTarget, SlotFootprint};

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use model::{
    Assignment, Location, PlacementDraft, PlacementError, Site, Storage, StorageId,
    StorageLifetime, StoragePurpose, Transfer, TransferKind, TransferPoint, TransferValue,
};

#[cfg(test)]
pub(super) mod tests;
