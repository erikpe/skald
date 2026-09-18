//! Static legality covers unreachable blocks too; content proofs run separately.
use super::{model::*, requirements::*, target::PlacementTarget};
use crate::backend::selected::{AbiArea, AbiLocation, Bundle, Constraint, OperandRole, Payload};
use std::collections::BTreeSet;

impl<P: Payload> Requirements<'_, P> {
    fn persistent(&self, draft: &PlacementDraft<'_, '_, P>, location: Location) -> bool {
        !matches!(location, Location::Storage(id) if draft.storage[id.0].lifetime != StorageLifetime::WholeCallable
            || !matches!(draft.storage[id.0].purpose, StoragePurpose::Home(_) | StoragePurpose::Spill(_)))
    }
    pub(super) fn validate(
        &mut self,
        draft: &PlacementDraft<'_, '_, P>,
        target: &impl PlacementTarget,
    ) -> Result<(), CheckFailure> {
        let resources = &draft.selected.draft().context().resources;
        let mut seen = BTreeSet::new();
        for &view in &self.preserved {
            let location = CheckLocation::Entry;
            let resource = resources
                .views()
                .find(|(id, _)| *id == view)
                .ok_or_else(|| self.failure(location, CheckReason::Preservation))?
                .1;
            if !seen.insert(view) || resource.reserved {
                return Err(self.failure(location, CheckReason::Preservation));
            }
            for &other in &seen {
                if view != other && resources.overlaps(view, other).expect("checked views") {
                    return Err(self.failure(location, CheckReason::Preservation));
                }
            }
        }
        for (&assignment, &location) in &draft.assignments {
            let fail = |reason| self.failure(CheckLocation::Assignment(assignment), reason);
            if let Location::Resource(view) = location {
                let resource = resources
                    .views()
                    .find(|(id, _)| *id == view)
                    .expect("shape-checked resource")
                    .1;
                let bank = resources
                    .banks()
                    .find(|(id, _)| *id == resource.bank)
                    .expect("checked bank")
                    .1;
                if resources
                    .require_view(view, resource.bits, bank, true)
                    .is_err()
                {
                    return Err(fail(CheckReason::Reserved));
                }
            }
            if !self.persistent(draft, location) {
                return Err(fail(CheckReason::Lifetime));
            }
            match assignment {
                Assignment::Input(slot) => {
                    let owner = draft.selected.receipt().key();
                    let signature = draft
                        .selected
                        .draft()
                        .context()
                        .binding(owner)
                        .expect("verified owner")
                        .signature_id();
                    if location != abi_location(signature, self.abi[slot].location) {
                        return Err(fail(CheckReason::Abi));
                    }
                }
                Assignment::Parameter { .. } => {
                    if matches!(location, Location::Abi { .. }) {
                        return Err(fail(CheckReason::Abi));
                    }
                }
                Assignment::EdgeArgument { .. } => {
                    if matches!(
                        location,
                        Location::Abi {
                            area: AbiArea::Outgoing | AbiArea::Results,
                            ..
                        }
                    ) {
                        return Err(fail(CheckReason::Abi));
                    }
                }
                _ => {}
            }
        }
        for (index, storage) in draft.storage.iter().enumerate() {
            let valid = match storage.purpose {
                StoragePurpose::Home(_) | StoragePurpose::Spill(_) => {
                    storage.lifetime == StorageLifetime::WholeCallable
                }
                StoragePurpose::CalleeSave(view) => {
                    self.preserved.contains(&view)
                        && storage.representation
                            == self.token_representation(draft, TransferValue::Preserved(view))
                        && storage.lifetime == StorageLifetime::WholeCallable
                }
                StoragePurpose::TransferScratch => {
                    matches!(storage.lifetime, StorageLifetime::Transfer(_))
                }
            };
            if !valid {
                return Err(self.failure(CheckLocation::Storage(index), CheckReason::Lifetime));
            }
        }
        for (&block, facts) in &self.blocks {
            for (ordinal, &payload) in facts.instructions.iter().enumerate() {
                self.check_node(draft, Site::Instruction { block, ordinal }, payload)?;
            }
            self.check_node(
                draft,
                Site::Terminal(block),
                facts.terminal.expect("verified terminal"),
            )?;
            for (slot, (successor, _)) in facts.edges.iter().enumerate() {
                let parameters = &self.blocks[successor].parameters;
                for a in 0..parameters.len() {
                    let left = draft.assignments[&Assignment::Parameter {
                        block: *successor,
                        slot: a,
                    }];
                    for b in a + 1..parameters.len() {
                        let right = draft.assignments[&Assignment::Parameter {
                            block: *successor,
                            slot: b,
                        }];
                        if left != right && self.aliases(draft, left, right) {
                            return Err(self.failure(
                                CheckLocation::Edge { block, slot },
                                CheckReason::Overlap,
                            ));
                        }
                    }
                }
            }
        }
        for (&point, transfers) in &draft.transfers {
            let mut kills = vec![];
            for (index, transfer) in transfers.iter().enumerate() {
                let location = CheckLocation::Transfer { point, index };
                let fail = |reason| self.failure(location, reason);
                if matches!(
                    transfer.destination,
                    Location::Abi {
                        area: AbiArea::Incoming,
                        ..
                    }
                ) {
                    return Err(fail(CheckReason::Abi));
                }
                if let TransferValue::Preserved(view) = transfer.value {
                    if !self.preserved.contains(&view) {
                        return Err(fail(CheckReason::Preservation));
                    }
                }
                for operand in [transfer.source, transfer.destination] {
                    match operand {
                        Location::Resource(view) => {
                            let resource = resources
                                .views()
                                .find(|(id, _)| *id == view)
                                .expect("checked resource")
                                .1;
                            let bank = resources
                                .banks()
                                .find(|(id, _)| *id == resource.bank)
                                .expect("checked bank")
                                .1;
                            if resources
                                .require_view(view, resource.bits, bank, true)
                                .is_err()
                            {
                                return Err(fail(CheckReason::Reserved));
                            }
                        }
                        Location::Storage(id) => {
                            if matches!(draft.storage[id.0].lifetime, StorageLifetime::Transfer(required) if required != point)
                            {
                                return Err(fail(CheckReason::Lifetime));
                            }
                            if let StoragePurpose::CalleeSave(view) = draft.storage[id.0].purpose {
                                if transfer.value != TransferValue::Preserved(view) {
                                    return Err(fail(CheckReason::Preservation));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                for (slot, &view) in transfer.scratch.iter().enumerate() {
                    let resource = resources
                        .views()
                        .find(|(id, _)| *id == view)
                        .expect("checked scratch")
                        .1;
                    let bank = resources
                        .banks()
                        .find(|(id, _)| *id == resource.bank)
                        .expect("checked bank")
                        .1;
                    if resources
                        .require_view(view, resource.bits, bank, true)
                        .is_err()
                        || self.aliases(draft, Location::Resource(view), transfer.source)
                        || self.aliases(draft, Location::Resource(view), transfer.destination)
                        || transfer.scratch[..slot]
                            .iter()
                            .any(|&other| resources.overlaps(view, other).expect("checked views"))
                    {
                        return Err(fail(CheckReason::Scratch));
                    }
                }
                let killed = target.check_transfer(transfer).map_err(fail)?;
                if killed
                    .iter()
                    .any(|&unit| resources.require_unit(unit).is_err())
                {
                    return Err(fail(CheckReason::Transfer));
                }
                kills.push(killed);
            }
            self.move_kills.insert(point, kills);
        }
        Ok(())
    }
    fn check_node(
        &self,
        draft: &PlacementDraft<'_, '_, P>,
        site: Site,
        payload: &P,
    ) -> Result<(), CheckFailure> {
        let description = payload.describe();
        let owner = draft.selected.receipt().key();
        let signature = description.call_signature.unwrap_or_else(|| {
            draft
                .selected
                .draft()
                .context()
                .binding(owner)
                .expect("verified owner")
                .signature_id()
        });
        for (slot, operand) in description.operands.iter().enumerate() {
            let assignment = Assignment::Operand { site, slot };
            let location = draft.assignments[&assignment];
            let legal = match operand.constraint {
                Constraint::Fixed(view) => location == Location::Resource(view),
                Constraint::Resources { views, memory } => match location {
                    Location::Resource(view) => views.contains(&view),
                    Location::Storage(_) => memory,
                    _ => false,
                },
                Constraint::AbiSlot { area, index } => {
                    location
                        == Location::Abi {
                            signature,
                            area,
                            index,
                        }
                }
            };
            if !legal {
                return Err(self.failure(
                    CheckLocation::Assignment(assignment),
                    CheckReason::Constraint,
                ));
            }
            if operand.role == OperandRole::Definition {
                for (earlier, other) in description.operands[..slot].iter().enumerate() {
                    if other.role == OperandRole::Definition
                        && other.timing == operand.timing
                        && self.aliases(
                            draft,
                            location,
                            draft.assignments[&Assignment::Operand {
                                site,
                                slot: earlier,
                            }],
                        )
                    {
                        return Err(self
                            .failure(CheckLocation::Assignment(assignment), CheckReason::Overlap));
                    }
                }
            }
        }
        for tie in description.ties {
            if draft.assignments[&Assignment::Operand {
                site,
                slot: tie.input,
            }] != draft.assignments[&Assignment::Operand {
                site,
                slot: tie.output,
            }] {
                return Err(self.failure(CheckLocation::Site(site), CheckReason::Tie));
            }
        }
        if let Some(Bundle::Bounded { scratch, .. }) = description.bundle {
            let mut occupied = vec![];
            for (group, scratch) in scratch.iter().enumerate() {
                for slot in 0..usize::from(scratch.count.get()) {
                    let assignment = Assignment::Scratch { site, group, slot };
                    let location = draft.assignments[&assignment];
                    if !matches!(location, Location::Resource(view) if scratch.views.contains(&view))
                        || occupied
                            .iter()
                            .any(|&other| self.aliases(draft, location, other))
                        || description.operands.iter().enumerate().any(|(slot, _)| {
                            self.aliases(
                                draft,
                                location,
                                draft.assignments[&Assignment::Operand { site, slot }],
                            )
                        })
                    {
                        return Err(self
                            .failure(CheckLocation::Assignment(assignment), CheckReason::Scratch));
                    }
                    occupied.push(location);
                }
            }
        }
        Ok(())
    }
}
fn abi_location(signature: crate::backend::plan::SignatureId, location: AbiLocation) -> Location {
    match location {
        AbiLocation::Fixed(view) => Location::Resource(view),
        AbiLocation::Slot { area, index } => Location::Abi {
            signature,
            area,
            index,
        },
    }
}
