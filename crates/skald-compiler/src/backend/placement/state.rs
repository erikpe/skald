//! Must-contents lattice and bit-transfer semantics. No producer maps enter here.
use super::{model::*, requirements::*};
use crate::backend::selected::{AbiArea, BankKind, Payload, Representation, RepresentationKind};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct State(pub Vec<BTreeSet<TransferValue>>);
impl State {
    pub fn empty(locations: usize) -> Self {
        Self(vec![BTreeSet::new(); locations])
    }
    pub fn top(locations: usize, tokens: &[TransferValue]) -> Self {
        Self(vec![tokens.iter().copied().collect(); locations])
    }
    pub fn intersect(&mut self, other: &Self) -> usize {
        let mut removed = 0;
        for (left, right) in self.0.iter_mut().zip(&other.0) {
            let before = left.len();
            left.retain(|token| right.contains(token));
            removed += before - left.len();
        }
        removed
    }
    pub fn forget(&mut self, token: TransferValue) {
        for contents in &mut self.0 {
            contents.remove(&token);
        }
    }
}
impl<P: Payload> Requirements<'_, P> {
    pub fn index(&self, location: Location) -> usize {
        self.locations
            .binary_search(&location)
            .expect("structurally checked location")
    }
    pub fn aliases(&self, draft: &PlacementDraft<'_, '_, P>, a: Location, b: Location) -> bool {
        match (a, b) {
            (Location::Resource(a), Location::Resource(b)) => draft
                .selected
                .draft()
                .context()
                .resources
                .overlaps(a, b)
                .expect("checked views"),
            (Location::Abi { area: a_area, .. }, Location::Abi { area: b_area, .. })
                if a_area == b_area =>
            {
                let a = self.footprints[&a];
                let b = self.footprints[&b];
                a.offset < b.offset + b.bytes && b.offset < a.offset + a.bytes
            }
            _ => a == b,
        }
    }
    pub fn contents<'a>(
        &self,
        state: &'a State,
        location: Location,
    ) -> &'a BTreeSet<TransferValue> {
        &state.0[self.index(location)]
    }
    pub fn has(&self, state: &State, location: Location, token: TransferValue) -> bool {
        self.contents(state, location).contains(&token)
    }
    pub fn write(
        &self,
        draft: &PlacementDraft<'_, '_, P>,
        state: &mut State,
        location: Location,
        tokens: BTreeSet<TransferValue>,
    ) {
        for (index, &other) in self.locations.iter().enumerate() {
            if self.aliases(draft, location, other) {
                state.0[index].clear();
            }
        }
        state.0[self.index(location)] = tokens;
    }
    pub fn kill_unit(
        &self,
        draft: &PlacementDraft<'_, '_, P>,
        state: &mut State,
        unit: crate::backend::selected::UnitId,
    ) {
        for (index, location) in self.locations.iter().enumerate() {
            if matches!(location, Location::Resource(view) if draft.selected.draft().context().resources.view_units(*view).expect("checked view").contains(&unit))
            {
                state.0[index].clear();
            }
        }
    }
    pub fn expire(
        &self,
        draft: &PlacementDraft<'_, '_, P>,
        state: &mut State,
        point: TransferPoint,
    ) {
        for (index, storage) in draft.storage.iter().enumerate() {
            if storage.lifetime == StorageLifetime::Transfer(point) {
                state.0[self.index(Location::Storage(StorageId(index)))].clear();
            }
        }
    }
    pub fn token_representation(
        &self,
        draft: &PlacementDraft<'_, '_, P>,
        token: TransferValue,
    ) -> Representation {
        match token {
            TransferValue::Selected(value) => self.values[&value],
            TransferValue::Preserved(view) => {
                let resources = &draft.selected.draft().context().resources;
                let resource = resources
                    .views()
                    .find(|(id, _)| *id == view)
                    .expect("checked preserved view")
                    .1;
                let bank = resources
                    .banks()
                    .find(|(id, _)| *id == resource.bank)
                    .expect("checked bank")
                    .1;
                Representation::new(
                    if bank == BankKind::Float {
                        RepresentationKind::Float
                    } else {
                        RepresentationKind::Bits
                    },
                    resource.bits,
                )
                .expect("checked width")
            }
        }
    }
    pub fn transfers(
        &self,
        draft: &PlacementDraft<'_, '_, P>,
        state: &mut State,
        point: TransferPoint,
        strict: bool,
    ) -> Result<(), CheckFailure> {
        self.expire(draft, state, point);
        if let Some(transfers) = draft.transfers.get(&point) {
            for (index, transfer) in transfers.iter().enumerate() {
                let available = self.has(state, transfer.source, transfer.value);
                if strict && !available {
                    return Err(self.failure(
                        CheckLocation::Transfer { point, index },
                        CheckReason::MissingValue,
                    ));
                }
                let captured = if available {
                    self.contents(state, transfer.source)
                        .iter()
                        .copied()
                        .filter(|&token| {
                            transfer_compatible(
                                self.token_representation(draft, token),
                                transfer.source_representation,
                            )
                        })
                        .collect()
                } else {
                    BTreeSet::new()
                };
                for &unit in &self.move_kills[&point][index] {
                    self.kill_unit(draft, state, unit);
                }
                self.write(draft, state, transfer.destination, captured);
            }
        }
        self.expire(draft, state, point);
        Ok(())
    }
    pub fn seed(&mut self, draft: &PlacementDraft<'_, '_, P>) -> Result<(), CheckFailure> {
        for (slot, &value) in self.inputs.iter().enumerate() {
            let location = draft.assignments[&Assignment::Input(slot)];
            for earlier in 0..slot {
                if self.aliases(
                    draft,
                    location,
                    draft.assignments[&Assignment::Input(earlier)],
                ) {
                    return Err(self.failure(
                        CheckLocation::Assignment(Assignment::Input(slot)),
                        CheckReason::Abi,
                    ));
                }
            }
            let index = self.index(location);
            self.seed.0[index].insert(TransferValue::Selected(value));
        }
        for &view in &self.preserved {
            let index = self.index(Location::Resource(view));
            self.seed.0[index].insert(TransferValue::Preserved(view));
        }
        let mut seed = self.seed.clone();
        self.transfers(draft, &mut seed, TransferPoint::Entry, true)?;
        self.seed = seed;
        Ok(())
    }
    pub fn kill_call_slots(&self, state: &mut State) {
        for (index, location) in self.locations.iter().enumerate() {
            if matches!(
                location,
                Location::Abi {
                    area: AbiArea::Outgoing | AbiArea::Results,
                    ..
                }
            ) {
                state.0[index].clear();
            }
        }
    }
}
