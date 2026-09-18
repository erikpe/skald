//! Native move/preservation/ABI footprints shared by independent placement checking.
use super::{abi::STACK_SLOT_BYTES, selected::Instruction, Gpr, NativeResources};
use crate::backend::{
    placement::{
        self, CheckFailure, CheckLocation, CheckReason, CheckedPlacement, Location, PlacementDraft,
        PlacementTarget, SlotFootprint, Transfer, TransferKind,
    },
    plan::{SignatureId, TargetProfile},
    selected::{AbiArea, BankKind, Representation, RepresentationKind, UnitId, ViewId},
};

struct NativeTarget {
    resources: NativeResources,
    profile: TargetProfile,
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn check_native_placement<'s, 'p>(
    draft: PlacementDraft<'s, 'p, Instruction>,
) -> Result<CheckedPlacement<'s, 'p, Instruction>, CheckFailure> {
    let profile = draft
        .selected()
        .draft()
        .context()
        .catalog()
        .plan()
        .profile();
    let resources = NativeResources::for_profile(profile).map_err(|_| CheckFailure {
        location: CheckLocation::Entry,
        reason: CheckReason::WrongTarget,
        origin: None,
        structure: None,
    })?;
    placement::check_placement(draft, &NativeTarget { resources, profile })
}
impl PlacementTarget for NativeTarget {
    fn profile(&self) -> TargetProfile {
        self.profile
    }
    fn preserved_views(&self) -> Vec<ViewId> {
        Gpr::ALL
            .into_iter()
            .filter(|register| register.preserved() && !register.reserved())
            .map(|register| self.resources.gpr(register, 64).expect("canonical view"))
            .collect()
    }
    fn slot_footprint(
        &self,
        _: SignatureId,
        _: AbiArea,
        index: usize,
        representation: Representation,
    ) -> Result<SlotFootprint, CheckReason> {
        if representation.bits() > 64 {
            return Err(CheckReason::Abi);
        }
        Ok(SlotFootprint {
            offset: index
                .checked_mul(STACK_SLOT_BYTES)
                .ok_or(CheckReason::Capacity)?,
            bytes: STACK_SLOT_BYTES,
        })
    }
    fn check_transfer(&self, transfer: &Transfer) -> Result<Vec<UnitId>, CheckReason> {
        for representation in [
            transfer.source_representation,
            transfer.destination_representation,
        ] {
            if (representation.bank() == BankKind::Float && representation.bits() != 64)
                || (representation.bank() == BankKind::Integer
                    && !matches!(representation.bits(), 8 | 16 | 32 | 64))
            {
                return Err(CheckReason::Transfer);
            }
        }
        let memory_to_memory = !matches!(transfer.source, Location::Resource(_))
            && !matches!(transfer.destination, Location::Resource(_));
        if transfer.scratch.len() != usize::from(memory_to_memory) {
            return Err(CheckReason::Scratch);
        }
        let mut kills = vec![];
        if let Some(&scratch) = transfer.scratch.first() {
            let bank = if transfer.kind == TransferKind::Bitwise
                && transfer.source_representation != transfer.destination_representation
            {
                BankKind::Integer
            } else {
                transfer.source_representation.bank()
            };
            self.resources
                .catalog()
                .require_view(scratch, transfer.source_representation.bits(), bank, true)
                .map_err(|_| CheckReason::Scratch)?;
            kills.extend_from_slice(
                self.resources
                    .catalog()
                    .view_units(scratch)
                    .map_err(|_| CheckReason::Scratch)?,
            );
        }
        if transfer.kind == TransferKind::Bitwise
            && (!matches!(
                transfer.source_representation.kind,
                RepresentationKind::Bits | RepresentationKind::Float
            ) || !matches!(
                transfer.destination_representation.kind,
                RepresentationKind::Bits | RepresentationKind::Float
            ))
        {
            return Err(CheckReason::Transfer);
        }
        Ok(kills)
    }
}
