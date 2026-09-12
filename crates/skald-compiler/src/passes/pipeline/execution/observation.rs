//! Optional timing and ordered records for attempted MIR pass occurrences.

use std::time::Instant;

use crate::mir::rewrite::MirRewriteChangeSummary;

use super::{
    measurement::{MirPassOccurrenceOutcome, MirPassOccurrenceRecord},
    model::MirPassData,
};
use crate::passes::pipeline::{
    snapshot_analysis::{MirSnapshotAnalysisKind, MirSnapshotAnalysisUsage},
    MirPassOccurrence,
};

pub(super) struct MirPassOccurrenceRecorder {
    records: Option<Vec<MirPassOccurrenceRecord>>,
}

pub(super) struct MirPassAttempt {
    occurrence: MirPassOccurrence,
    started: Option<Instant>,
}

impl MirPassOccurrenceRecorder {
    pub(super) fn new(enabled: bool, schedule_len: usize) -> Self {
        Self {
            records: enabled.then(|| Vec::with_capacity(schedule_len)),
        }
    }

    pub(super) fn start(&self, occurrence: MirPassOccurrence) -> MirPassAttempt {
        MirPassAttempt {
            occurrence,
            started: self.records.as_ref().map(|_| Instant::now()),
        }
    }

    pub(super) fn record_completed(
        &mut self,
        attempt: MirPassAttempt,
        outcome: MirPassOccurrenceOutcome,
        data: MirPassData,
        rewrite_changes: MirRewriteChangeSummary,
        verification_executions: u64,
        analysis_usage: Option<(MirSnapshotAnalysisKind, MirSnapshotAnalysisUsage)>,
    ) {
        let Some(elapsed) = attempt.elapsed() else {
            debug_assert!(self.records.is_none());
            return;
        };
        self.records
            .as_mut()
            .expect("a timed pass attempt must have an occurrence sink")
            .push(MirPassOccurrenceRecord::completed(
                attempt.occurrence,
                elapsed,
                outcome,
                data,
                rewrite_changes,
                verification_executions,
                analysis_records(analysis_usage),
            ));
    }

    pub(super) fn record_failed(
        &mut self,
        attempt: MirPassAttempt,
        analysis_usage: Option<(MirSnapshotAnalysisKind, MirSnapshotAnalysisUsage)>,
    ) {
        let Some(elapsed) = attempt.elapsed() else {
            debug_assert!(self.records.is_none());
            return;
        };
        self.records
            .as_mut()
            .expect("a timed pass attempt must have an occurrence sink")
            .push(MirPassOccurrenceRecord::failed(
                attempt.occurrence,
                elapsed,
                analysis_records(analysis_usage),
            ));
    }

    pub(super) fn into_records(self) -> Vec<MirPassOccurrenceRecord> {
        self.records.unwrap_or_default()
    }
}

impl MirPassAttempt {
    fn elapsed(&self) -> Option<std::time::Duration> {
        self.started.map(|started| started.elapsed())
    }
}

fn analysis_records(
    usage: Option<(MirSnapshotAnalysisKind, MirSnapshotAnalysisUsage)>,
) -> Vec<(MirSnapshotAnalysisKind, MirSnapshotAnalysisUsage)> {
    usage
        .filter(|(_, usage)| !usage.is_empty())
        .into_iter()
        .collect()
}
