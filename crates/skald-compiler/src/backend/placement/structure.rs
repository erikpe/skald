//! Shape/type validation only. Does not check availability, constraints or target moves.
use super::model::*;
use crate::backend::selected::{Bundle, Payload, Representation, RepresentationKind, SelectedFact};
use std::collections::{BTreeMap, BTreeSet};

impl<P: Payload> PlacementDraft<'_, '_, P> {
    pub(in crate::backend) fn validate_structure(&self) -> Result<(), PlacementError> {
        let mut values = BTreeMap::new();
        let mut required = BTreeMap::new();
        let mut points = BTreeSet::from([TransferPoint::Entry]);
        let mut inputs = vec![];
        self.selected
            .visit::<()>(|fact| {
                match fact {
                    SelectedFact::Entry { inputs: ids, .. } => inputs.extend_from_slice(ids),
                    SelectedFact::Value {
                        id, representation, ..
                    } => {
                        values.insert(id, representation);
                    }
                    _ => {}
                }
                Ok(())
            })
            .expect("infallible visitor");
        for (slot, id) in inputs.iter().enumerate() {
            required.insert(Assignment::Input(slot), values[id]);
        }
        self.selected
            .visit::<()>(|fact| {
                let site_payload = match fact {
                    SelectedFact::Block { id, parameters, .. } => {
                        for (slot, value) in parameters.iter().enumerate() {
                            required
                                .insert(Assignment::Parameter { block: id, slot }, values[value]);
                        }
                        None
                    }
                    SelectedFact::Instruction {
                        block,
                        ordinal,
                        payload,
                    } => Some((Site::Instruction { block, ordinal }, payload, true)),
                    SelectedFact::Terminal {
                        block,
                        payload,
                        edges,
                    } => {
                        for (edge, (_, arguments)) in edges.iter().enumerate() {
                            points.insert(TransferPoint::Edge { block, slot: edge });
                            for (slot, value) in arguments.iter().enumerate() {
                                required.insert(
                                    Assignment::EdgeArgument { block, edge, slot },
                                    values[value],
                                );
                            }
                        }
                        payload.map(|payload| (Site::Terminal(block), payload, false))
                    }
                    _ => None,
                };
                if let Some((site, payload, after)) = site_payload {
                    points.insert(TransferPoint::Before(site));
                    if after {
                        points.insert(TransferPoint::After(site));
                    }
                    let description = payload.describe();
                    for (slot, operand) in description.operands.iter().enumerate() {
                        required.insert(Assignment::Operand { site, slot }, operand.representation);
                    }
                    if let Some(Bundle::Bounded { scratch, .. }) = description.bundle {
                        for (group, scratch) in scratch.iter().enumerate() {
                            for slot in 0..usize::from(scratch.count.get()) {
                                required.insert(
                                    Assignment::Scratch { site, group, slot },
                                    scratch.representation,
                                );
                            }
                        }
                    }
                }
                Ok(())
            })
            .expect("infallible visitor");
        for (index, storage) in self.storage.iter().enumerate() {
            let invalid = || PlacementError::InvalidStorage(index);
            if storage.bytes < usize::from(storage.representation.bits()).div_ceil(8)
                || !storage.alignment.is_power_of_two()
                || matches!(storage.lifetime, StorageLifetime::Transfer(point) if !points.contains(&point))
            {
                return Err(invalid());
            }
            match storage.purpose {
                StoragePurpose::Home(value) | StoragePurpose::Spill(value) => {
                    if values.get(&value) != Some(&storage.representation) {
                        return Err(invalid());
                    }
                }
                StoragePurpose::CalleeSave(view) => {
                    if !self.valid_location(Location::Resource(view), storage.representation) {
                        return Err(invalid());
                    }
                }
                StoragePurpose::TransferScratch => {}
            }
        }
        for (assignment, representation) in &required {
            let location = self
                .assignments
                .get(assignment)
                .ok_or(PlacementError::MissingAssignment(*assignment))?;
            if !self.valid_location(*location, *representation) {
                return Err(PlacementError::InvalidLocation(*assignment));
            }
        }
        for assignment in self.assignments.keys() {
            if !required.contains_key(assignment) {
                return Err(PlacementError::UnknownAssignment(*assignment));
            }
        }
        for (point, transfers) in &self.transfers {
            if !points.contains(point) {
                return Err(PlacementError::UnknownTransferPoint(*point));
            }
            for (index, transfer) in transfers.iter().enumerate() {
                let valid_identity = match transfer.value {
                    TransferValue::Selected(value) => {
                        values.get(&value).is_some_and(|representation| {
                            transfer_compatible(*representation, transfer.source_representation)
                                && transfer_compatible(
                                    *representation,
                                    transfer.destination_representation,
                                )
                        })
                    }
                    TransferValue::Preserved(view) => {
                        matches!(
                            transfer.source_representation.kind,
                            RepresentationKind::Bits | RepresentationKind::Float
                        ) && self.selected.draft().context().resources.views().any(
                            |(id, resource)| {
                                id == view && resource.bits == transfer.source_representation.bits()
                            },
                        ) && transfer_compatible(
                            transfer.source_representation,
                            transfer.destination_representation,
                        )
                    }
                };
                if !valid_identity
                    || (transfer.kind == TransferKind::Copy
                        && transfer.source_representation != transfer.destination_representation)
                    || !self.valid_location(transfer.source, transfer.source_representation)
                    || !self
                        .valid_location(transfer.destination, transfer.destination_representation)
                    || transfer.scratch.iter().any(|view| {
                        self.selected
                            .draft()
                            .context()
                            .resources
                            .view_units(*view)
                            .is_err()
                    })
                {
                    return Err(PlacementError::InvalidTransfer(*point, index));
                }
            }
        }
        Ok(())
    }
    fn valid_location(&self, location: Location, representation: Representation) -> bool {
        let context = self.selected.draft().context();
        match location {
            Location::Resource(view) => context
                .resources
                .require_view(view, representation.bits(), representation.bank(), false)
                .is_ok(),
            Location::Storage(id) => self
                .storage
                .get(id.0)
                .is_some_and(|storage| storage.representation == representation),
            Location::Abi {
                signature,
                area,
                index,
            } => context
                .areas(signature)
                .is_some_and(|areas| areas.require_slot(area, index, representation).is_ok()),
        }
    }
}
