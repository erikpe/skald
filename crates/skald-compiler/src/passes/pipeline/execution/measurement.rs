//! Typed data for one attempted final-MIR pass occurrence.

use std::time::Duration;

use crate::mir::rewrite::MirRewriteChangeSummary;

use super::super::{MirPassIdentity, MirPassOccurrence};
use super::model::MirPassData;

/// One stable pass-owned integer measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirPassMeasurement {
    name: &'static str,
    value: u64,
}

impl MirPassMeasurement {
    pub(in crate::passes::pipeline) const fn count(name: &'static str, value: u64) -> Self {
        Self { name, value }
    }

    pub const fn name(self) -> &'static str {
        self.name
    }

    pub const fn value(self) -> u64 {
        self.value
    }
}

/// Observable result of one attempted pass occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirPassOccurrenceOutcome {
    Unchanged,
    Changed,
    Failed,
}

/// Ordered data produced for one attempted pass occurrence.
///
/// Durations are operational observations. Identity, outcome, and integer
/// measurements are deterministic for an identical compiler input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirPassOccurrenceRecord {
    position: usize,
    identity: MirPassIdentity,
    name: &'static str,
    occurrence: usize,
    elapsed: Duration,
    outcome: MirPassOccurrenceOutcome,
    data_available: bool,
    processed_callables: u64,
    changed_callables: u64,
    retained_mir_entities: u64,
    inserted_mir_entities: u64,
    removed_mir_entities: u64,
    verification_executions: u64,
    measurements: Vec<MirPassMeasurement>,
}

impl MirPassOccurrenceRecord {
    pub const fn position(&self) -> usize {
        self.position
    }

    pub const fn identity(&self) -> MirPassIdentity {
        self.identity
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub const fn occurrence(&self) -> usize {
        self.occurrence
    }

    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub const fn outcome(&self) -> MirPassOccurrenceOutcome {
        self.outcome
    }

    pub const fn processed_callables(&self) -> Option<u64> {
        if self.data_available {
            Some(self.processed_callables)
        } else {
            None
        }
    }

    pub const fn changed_callables(&self) -> Option<u64> {
        if self.data_available {
            Some(self.changed_callables)
        } else {
            None
        }
    }

    pub const fn retained_mir_entities(&self) -> Option<u64> {
        if self.data_available {
            Some(self.retained_mir_entities)
        } else {
            None
        }
    }

    pub const fn inserted_mir_entities(&self) -> Option<u64> {
        if self.data_available {
            Some(self.inserted_mir_entities)
        } else {
            None
        }
    }

    pub const fn removed_mir_entities(&self) -> Option<u64> {
        if self.data_available {
            Some(self.removed_mir_entities)
        } else {
            None
        }
    }

    pub const fn verification_executions(&self) -> u64 {
        self.verification_executions
    }

    pub fn measurements(&self) -> &[MirPassMeasurement] {
        &self.measurements
    }

    pub(super) fn completed(
        occurrence: MirPassOccurrence,
        elapsed: Duration,
        outcome: MirPassOccurrenceOutcome,
        data: MirPassData,
        rewrite_changes: MirRewriteChangeSummary,
        verification_executions: u64,
    ) -> Self {
        Self::new(
            occurrence,
            elapsed,
            outcome,
            true,
            data.processed_callables(),
            data.changed_callables(),
            count(rewrite_changes.retained()),
            count(rewrite_changes.inserted()),
            count(rewrite_changes.removed()),
            verification_executions,
            data.into_measurements(),
        )
    }

    pub(super) fn failed(occurrence: MirPassOccurrence, elapsed: Duration) -> Self {
        Self::new(
            occurrence,
            elapsed,
            MirPassOccurrenceOutcome::Failed,
            false,
            0,
            0,
            0,
            0,
            0,
            0,
            Vec::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        occurrence: MirPassOccurrence,
        elapsed: Duration,
        outcome: MirPassOccurrenceOutcome,
        data_available: bool,
        processed_callables: usize,
        changed_callables: usize,
        retained_mir_entities: u64,
        inserted_mir_entities: u64,
        removed_mir_entities: u64,
        verification_executions: u64,
        measurements: Vec<MirPassMeasurement>,
    ) -> Self {
        Self {
            position: occurrence.position(),
            identity: occurrence.identity(),
            name: occurrence.name(),
            occurrence: occurrence.occurrence(),
            elapsed,
            outcome,
            data_available,
            processed_callables: count(processed_callables),
            changed_callables: count(changed_callables),
            retained_mir_entities,
            inserted_mir_entities,
            removed_mir_entities,
            verification_executions,
            measurements,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        identity: (usize, u16, &'static str, usize),
        elapsed: Duration,
        outcome: MirPassOccurrenceOutcome,
        callables: Option<(u64, u64)>,
        measurements: Vec<(&'static str, u64)>,
    ) -> Self {
        let (position, identity, name, occurrence) = identity;
        let data_available = callables.is_some();
        let (processed_callables, changed_callables) = callables.unwrap_or_default();
        Self {
            position,
            identity: MirPassIdentity::new(identity),
            name,
            occurrence,
            elapsed,
            outcome,
            data_available,
            processed_callables,
            changed_callables,
            retained_mir_entities: 0,
            inserted_mir_entities: 0,
            removed_mir_entities: 0,
            verification_executions: 0,
            measurements: measurements
                .into_iter()
                .map(|(name, value)| MirPassMeasurement::count(name, value))
                .collect(),
        }
    }
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
