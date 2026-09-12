//! Stable observations for one analysis kind in one schedule occurrence.

/// Closed compiler-owned identity for snapshot analyses reported by the MIR pipeline.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MirSnapshotAnalysisKind {
    LocalConstants,
}

impl MirSnapshotAnalysisKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::LocalConstants => "local-constants",
        }
    }
}

/// Deterministic usage of one snapshot analysis in one schedule occurrence.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MirSnapshotAnalysisUsage {
    requests: u64,
    computations: u64,
    repeated_snapshot_requests: u64,
    results_before: u64,
    results_inserted: u64,
    results_discarded: u64,
}

impl MirSnapshotAnalysisUsage {
    pub const fn requests(self) -> u64 {
        self.requests
    }
    pub const fn computations(self) -> u64 {
        self.computations
    }
    pub const fn repeated_snapshot_requests(self) -> u64 {
        self.repeated_snapshot_requests
    }
    pub const fn distinct_snapshot_keys(self) -> u64 {
        self.requests
            .saturating_sub(self.repeated_snapshot_requests)
    }
    pub const fn results_before(self) -> u64 {
        self.results_before
    }
    pub const fn results_inserted(self) -> u64 {
        self.results_inserted
    }
    pub const fn results_discarded(self) -> u64 {
        self.results_discarded
    }

    pub(super) fn between(before: Self, after: Self) -> Self {
        Self {
            requests: after.requests.saturating_sub(before.requests),
            computations: after.computations.saturating_sub(before.computations),
            repeated_snapshot_requests: after
                .repeated_snapshot_requests
                .saturating_sub(before.repeated_snapshot_requests),
            results_before: before
                .results_inserted
                .saturating_sub(before.results_discarded),
            results_inserted: after
                .results_inserted
                .saturating_sub(before.results_inserted),
            results_discarded: after
                .results_discarded
                .saturating_sub(before.results_discarded),
        }
    }

    pub(super) fn record_uncached_request(&mut self, repeated: bool) {
        self.requests = self.requests.saturating_add(1);
        self.computations = self.computations.saturating_add(1);
        self.repeated_snapshot_requests = self
            .repeated_snapshot_requests
            .saturating_add(u64::from(repeated));
    }

    pub(crate) fn accumulate(&mut self, other: Self) {
        self.requests = self.requests.saturating_add(other.requests);
        self.computations = self.computations.saturating_add(other.computations);
        self.repeated_snapshot_requests = self
            .repeated_snapshot_requests
            .saturating_add(other.repeated_snapshot_requests);
        self.results_before = self.results_before.saturating_add(other.results_before);
        self.results_inserted = self.results_inserted.saturating_add(other.results_inserted);
        self.results_discarded = self
            .results_discarded
            .saturating_add(other.results_discarded);
    }

    pub(in crate::passes::pipeline) const fn is_empty(self) -> bool {
        self.requests == 0
            && self.computations == 0
            && self.repeated_snapshot_requests == 0
            && self.results_before == 0
            && self.results_inserted == 0
            && self.results_discarded == 0
    }

    #[cfg(test)]
    pub(crate) const fn for_test(
        requests: u64,
        computations: u64,
        repeated_snapshot_requests: u64,
        results_before: u64,
        results_inserted: u64,
        results_discarded: u64,
    ) -> Self {
        Self {
            requests,
            computations,
            repeated_snapshot_requests,
            results_before,
            results_inserted,
            results_discarded,
        }
    }
}
