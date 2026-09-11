//! Request-local accounting for semantic work performed during resolution.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ResolutionMeasurements {
    semantic_range_rounds: u64,
    semantic_range_bodies_revisited: u64,
    semantic_range_interner_copies: u64,
}

impl ResolutionMeasurements {
    pub(super) fn record_semantic_range_round(&mut self, bodies_revisited: usize) {
        self.semantic_range_rounds = self.semantic_range_rounds.saturating_add(1);
        self.semantic_range_bodies_revisited = self
            .semantic_range_bodies_revisited
            .saturating_add(count(bodies_revisited));
        self.semantic_range_interner_copies = self.semantic_range_interner_copies.saturating_add(1);
    }

    pub(crate) const fn semantic_range_rounds(self) -> u64 {
        self.semantic_range_rounds
    }

    pub(crate) const fn semantic_range_bodies_revisited(self) -> u64 {
        self.semantic_range_bodies_revisited
    }

    pub(crate) const fn semantic_range_interner_copies(self) -> u64 {
        self.semantic_range_interner_copies
    }
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_measurements_saturate() {
        let mut measurements = ResolutionMeasurements {
            semantic_range_rounds: u64::MAX,
            semantic_range_bodies_revisited: u64::MAX,
            semantic_range_interner_copies: u64::MAX,
        };
        measurements.record_semantic_range_round(1);
        assert_eq!(measurements.semantic_range_rounds(), u64::MAX);
        assert_eq!(measurements.semantic_range_bodies_revisited(), u64::MAX);
        assert_eq!(measurements.semantic_range_interner_copies(), u64::MAX);
    }
}
