//! Must-contents lattice and bit-transfer semantics. No producer maps enter here.
use super::{model::*, requirements::*};
use crate::backend::selected::{AbiArea, BankKind, Payload, Representation, RepresentationKind};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const WORD_BITS: usize = u64::BITS as usize;

/// The deterministic mapping from semantic identities to checker-private bits.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct TokenLayout {
    tokens: Vec<TransferValue>,
    bits: BTreeMap<TransferValue, usize>,
}

impl TokenLayout {
    pub fn new(mut tokens: Vec<TransferValue>) -> Arc<Self> {
        tokens.sort_unstable();
        tokens.dedup();
        let bits = tokens
            .iter()
            .copied()
            .enumerate()
            .map(|(bit, token)| (token, bit))
            .collect();
        Arc::new(Self { tokens, bits })
    }

    pub fn tokens(&self) -> &[TransferValue] {
        &self.tokens
    }

    fn bit(&self, token: TransferValue) -> usize {
        self.bits[&token]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BitMatrix {
    locations: usize,
    words_per_location: usize,
    words: Vec<u64>,
}

impl BitMatrix {
    fn empty(locations: usize, bits: usize) -> Option<Self> {
        Self::filled(locations, bits, 0)
    }

    fn top(locations: usize, bits: usize) -> Option<Self> {
        let mut matrix = Self::filled(locations, bits, u64::MAX)?;
        let remainder = bits % WORD_BITS;
        if remainder != 0 {
            let final_mask = (1_u64 << remainder) - 1;
            for location in 0..locations {
                let final_word = matrix.range(location).end - 1;
                matrix.words[final_word] &= final_mask;
            }
        }
        Some(matrix)
    }

    fn filled(locations: usize, bits: usize, value: u64) -> Option<Self> {
        let words_per_location =
            (bits / WORD_BITS).checked_add(usize::from(bits % WORD_BITS != 0))?;
        let word_count = locations.checked_mul(words_per_location)?;
        word_count.checked_mul(std::mem::size_of::<u64>())?;

        let mut words = Vec::new();
        words.try_reserve_exact(word_count).ok()?;
        words.resize(word_count, value);
        Some(Self {
            locations,
            words_per_location,
            words,
        })
    }

    fn range(&self, location: usize) -> std::ops::Range<usize> {
        assert!(location < self.locations, "checked location index");
        let start = location * self.words_per_location;
        start..start + self.words_per_location
    }

    fn contains(&self, location: usize, bit: usize) -> bool {
        let range = self.range(location);
        self.words[range.start + bit / WORD_BITS] & (1_u64 << (bit % WORD_BITS)) != 0
    }

    fn clear(&mut self, location: usize) {
        let range = self.range(location);
        self.words[range].fill(0);
    }

    fn insert(&mut self, location: usize, bit: usize) {
        let range = self.range(location);
        self.words[range.start + bit / WORD_BITS] |= 1_u64 << (bit % WORD_BITS);
    }

    fn remove(&mut self, location: usize, bit: usize) {
        let range = self.range(location);
        self.words[range.start + bit / WORD_BITS] &= !(1_u64 << (bit % WORD_BITS));
    }

    fn intersect(&mut self, other: &Self) -> usize {
        debug_assert_eq!(self.locations, other.locations);
        debug_assert_eq!(self.words_per_location, other.words_per_location);
        self.words
            .iter_mut()
            .zip(&other.words)
            .map(|(left, &right)| {
                let removed = (*left & !right).count_ones() as usize;
                *left &= right;
                removed
            })
            .sum()
    }

    fn set_bits(&self, location: usize) -> impl Iterator<Item = usize> + '_ {
        let range = self.range(location);
        self.words[range]
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(word_index, mut word)| {
                std::iter::from_fn(move || {
                    if word == 0 {
                        return None;
                    }
                    let offset = word.trailing_zeros() as usize;
                    word &= word - 1;
                    Some(word_index * WORD_BITS + offset)
                })
            })
    }
}

#[derive(Clone, Debug)]
pub(super) struct State {
    layout: Arc<TokenLayout>,
    contents: BitMatrix,
}

impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        (Arc::ptr_eq(&self.layout, &other.layout) || self.layout == other.layout)
            && self.contents == other.contents
    }
}

impl Eq for State {}

impl State {
    pub fn empty(locations: usize, layout: &Arc<TokenLayout>) -> Option<Self> {
        Some(Self {
            layout: Arc::clone(layout),
            contents: BitMatrix::empty(locations, layout.tokens.len())?,
        })
    }

    pub fn top(locations: usize, layout: &Arc<TokenLayout>) -> Option<Self> {
        Some(Self {
            layout: Arc::clone(layout),
            contents: BitMatrix::top(locations, layout.tokens.len())?,
        })
    }

    pub fn contains(&self, location: usize, token: TransferValue) -> bool {
        self.contents.contains(location, self.layout.bit(token))
    }

    pub fn capture(
        &self,
        location: usize,
        mut include: impl FnMut(TransferValue) -> bool,
    ) -> BTreeSet<TransferValue> {
        self.contents
            .set_bits(location)
            .map(|bit| self.layout.tokens[bit])
            .filter(|&token| include(token))
            .collect()
    }

    pub fn clear(&mut self, location: usize) {
        self.contents.clear(location);
    }

    pub fn replace(&mut self, location: usize, tokens: BTreeSet<TransferValue>) {
        self.clear(location);
        for token in tokens {
            self.insert(location, token);
        }
    }

    pub fn insert(&mut self, location: usize, token: TransferValue) {
        self.contents.insert(location, self.layout.bit(token));
    }

    pub fn intersect(&mut self, other: &Self) -> usize {
        debug_assert!(Arc::ptr_eq(&self.layout, &other.layout) || self.layout == other.layout);
        self.contents.intersect(&other.contents)
    }

    pub fn forget(&mut self, token: TransferValue) {
        let bit = self.layout.bit(token);
        for location in 0..self.contents.locations {
            self.contents.remove(location, bit);
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
    pub fn has(&self, state: &State, location: Location, token: TransferValue) -> bool {
        state.contains(self.index(location), token)
    }
    pub fn capture(
        &self,
        state: &State,
        location: Location,
        include: impl FnMut(TransferValue) -> bool,
    ) -> BTreeSet<TransferValue> {
        state.capture(self.index(location), include)
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
                state.clear(index);
            }
        }
        state.replace(self.index(location), tokens);
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
                state.clear(index);
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
                state.clear(self.index(Location::Storage(StorageId(index))));
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
                    self.capture(state, transfer.source, |token| {
                        transfer_compatible(
                            self.token_representation(draft, token),
                            transfer.source_representation,
                        )
                    })
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
            self.seed.insert(index, TransferValue::Selected(value));
        }
        for &view in &self.preserved {
            let index = self.index(Location::Resource(view));
            self.seed.insert(index, TransferValue::Preserved(view));
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
                state.clear(index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BitMatrix, WORD_BITS};

    #[test]
    fn zero_bit_matrices_retain_location_shape() {
        let empty = BitMatrix::empty(3, 0).unwrap();
        let top = BitMatrix::top(3, 0).unwrap();
        assert_eq!(empty, top);
        assert_eq!(empty.locations, 3);
        assert_eq!(empty.set_bits(2).collect::<Vec<_>>(), vec![]);
    }

    #[test]
    fn top_masks_the_partially_used_final_word() {
        let matrix = BitMatrix::top(2, WORD_BITS + 3).unwrap();
        assert_eq!(matrix.set_bits(0).count(), WORD_BITS + 3);
        assert_eq!(matrix.set_bits(1).count(), WORD_BITS + 3);
        assert_eq!(matrix.words[1], 0b111);
        assert_eq!(matrix.words[3], 0b111);
    }

    #[test]
    fn operations_cross_word_and_location_boundaries() {
        let mut left = BitMatrix::empty(2, WORD_BITS + 2).unwrap();
        for bit in [0, WORD_BITS - 1, WORD_BITS, WORD_BITS + 1] {
            left.insert(0, bit);
        }
        left.insert(1, WORD_BITS);
        assert!(left.contains(0, WORD_BITS));
        assert!(left.contains(1, WORD_BITS));

        left.remove(0, WORD_BITS - 1);
        let mut right = BitMatrix::empty(2, WORD_BITS + 2).unwrap();
        right.insert(0, 0);
        right.insert(0, WORD_BITS + 1);
        assert_eq!(left.intersect(&right), 2);
        assert_eq!(left.set_bits(0).collect::<Vec<_>>(), vec![0, WORD_BITS + 1]);
        assert!(left.set_bits(1).next().is_none());

        left.clear(0);
        assert!(left.set_bits(0).next().is_none());
    }

    #[test]
    fn dimensions_and_reservations_are_checked() {
        assert!(BitMatrix::empty(usize::MAX, WORD_BITS + 1).is_none());
        assert!(BitMatrix::empty(usize::MAX / 4, WORD_BITS * 8).is_none());
    }
}
