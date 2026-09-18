//! Immutable target semantics, never producer availability or allocation decisions.
use super::{CheckReason, Transfer};
use crate::backend::{
    plan::{SignatureId, TargetProfile},
    selected::{AbiArea, Representation, UnitId, ViewId},
};

#[derive(Clone, Copy, Debug)]
pub(in crate::backend) struct SlotFootprint {
    pub offset: usize,
    pub bytes: usize,
}

/// Private target-owner contract. Implementations are reviewed immutable facts;
/// drafts cannot supply callbacks or producer-authored clobber/preservation lists.
pub(in crate::backend) trait PlacementTarget {
    fn profile(&self) -> TargetProfile;
    fn preserved_views(&self) -> Vec<ViewId>;
    fn slot_footprint(
        &self,
        signature: SignatureId,
        area: AbiArea,
        index: usize,
        representation: Representation,
    ) -> Result<SlotFootprint, CheckReason>;
    /// Independently validate a supported bit move and its exact declared scratch.
    /// Return every killed working unit, excluding the established destination.
    fn check_transfer(&self, transfer: &Transfer) -> Result<Vec<UnitId>, CheckReason>;
}
