use crate::backend::{
    graph::SelectedObjectId,
    placement::{CheckedPlacement, Site},
    plan::SignatureId,
    selected::{AbiArea, ViewId},
};
use std::collections::BTreeMap;

/// Policy describes an established, aligned frame pointer and fixed body SP.
#[derive(Clone, Copy, Debug)]
pub(in crate::backend) struct FramePolicy {
    pub alignment: usize,
    pub entry_remainder: usize,
    pub header_bytes: usize,
    pub incoming_base: usize,
    pub return_address: ReturnAddress,
    pub max_frame: usize,
    pub direct_min: i64,
    pub direct_max: i64,
    /// Optional bounded address+access recipe; scratch must already be assigned.
    pub materialization: Option<(i64, i64, u16)>,
    /// Target-approved scratch group dedicated to address formation.
    pub address_scratch_group: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum ReturnAddress {
    /// Return address relative to the established frame pointer.
    Stack { offset: usize, bytes: usize },
    /// Link register saved in the explicitly reserved frame header.
    #[cfg_attr(not(test), allow(dead_code))]
    Link {
        view: ViewId,
        offset: usize,
        bytes: usize,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum Base {
    Frame,
    Stack,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) struct Region {
    pub base: Base,
    pub offset: i64,
    pub bytes: usize,
    pub alignment: usize,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Key {
    Object(SelectedObjectId),
    Storage(usize),
    Abi(SignatureId, AbiArea),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum AddressRecipe {
    Direct,
    Materialized { scratch: ViewId, steps: u16 },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::backend) enum FrameError {
    InvalidPolicy,
    Overflow,
    UnsupportedAlignment(usize),
    UnsupportedSize(usize),
    MissingAbiLayout(SignatureId, AbiArea),
    UnsupportedDisplacement(i64),
    UndeclaredAddressScratch(Site),
    WrongPlacement,
    UnknownLocation(crate::backend::placement::Location),
}
/// No unchecked constructor or mutable query. The borrow includes placement identity.
pub(in crate::backend) struct FramePlan<'f, 's, 'p, P> {
    pub(super) placement: &'f CheckedPlacement<'s, 'p, P>,
    pub(super) policy: FramePolicy,
    pub(super) bytes: usize,
    pub(super) outgoing_bytes: usize,
    pub(super) regions: BTreeMap<Key, Region>,
    pub(super) object_accesses: BTreeMap<(Site, SelectedObjectId), AddressRecipe>,
}
impl<'f, 's, 'p, P> FramePlan<'f, 's, 'p, P> {
    pub(in crate::backend) fn checked_placement(&self) -> &'f CheckedPlacement<'s, 'p, P> {
        self.placement
    }
    pub(in crate::backend) fn policy(&self) -> FramePolicy {
        self.policy
    }
    // Read-only realization queries cannot change the receipt.
    pub(in crate::backend) fn object(&self, object: SelectedObjectId) -> Option<Region> {
        self.regions.get(&Key::Object(object)).copied()
    }
    pub(in crate::backend) fn storage(
        &self,
        storage: crate::backend::placement::StorageId,
    ) -> Option<Region> {
        self.regions.get(&Key::Storage(storage.index())).copied()
    }
    pub(in crate::backend) fn object_access(
        &self,
        site: Site,
        object: SelectedObjectId,
    ) -> Option<AddressRecipe> {
        self.object_accesses.get(&(site, object)).copied()
    }
    pub(in crate::backend) fn bytes(&self) -> usize {
        self.bytes
    }
    pub(in crate::backend) fn outgoing_bytes(&self) -> usize {
        self.outgoing_bytes
    }
    pub(in crate::backend) fn require_placement(
        &self,
        placement: &CheckedPlacement<'_, '_, P>,
    ) -> Result<(), FrameError> {
        if std::ptr::eq(self.placement, placement) {
            Ok(())
        } else {
            Err(FrameError::WrongPlacement)
        }
    }
}
