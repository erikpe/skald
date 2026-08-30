use crate::mir::rewrite::{MirProgramRewriteResult, MirRewriteChangeSummary};

use super::{
    measurement::{MirPassMeasurement, MirPassOccurrenceRecord},
    model::MirPassData,
    MirPipelineError,
};
use crate::passes::pipeline::{MirPassIdentity, MirPassOccurrence, VerifiedFinalMirProgram};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirPipelineStatistics {
    verification_executions: u64,
    pass_executions: u64,
    processed_callables: u64,
    changed_callables: u64,
    rewrite_changes: MirRewriteChangeSummary,
    pass_measurements: Vec<MirPassAggregateMeasurement>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MirPassAggregateMeasurement {
    identity: MirPassIdentity,
    pass_name: &'static str,
    measurement: MirPassMeasurement,
}

impl MirPipelineStatistics {
    pub(crate) const fn verification_executions(&self) -> u64 {
        self.verification_executions
    }

    pub(crate) const fn pass_executions(&self) -> u64 {
        self.pass_executions
    }

    pub(crate) const fn processed_callables(&self) -> u64 {
        self.processed_callables
    }

    pub(crate) const fn changed_callables(&self) -> u64 {
        self.changed_callables
    }

    pub(crate) const fn rewrite_changes(&self) -> MirRewriteChangeSummary {
        self.rewrite_changes
    }

    pub(crate) fn pass_measurements(
        &self,
    ) -> impl Iterator<Item = (MirPassIdentity, &'static str, MirPassMeasurement)> + '_ {
        self.pass_measurements.iter().map(|aggregate| {
            (
                aggregate.identity,
                aggregate.pass_name,
                aggregate.measurement,
            )
        })
    }

    pub(super) fn record_verification(&mut self) {
        self.verification_executions = self.verification_executions.saturating_add(1);
    }

    pub(super) fn record_pass_execution(&mut self) {
        self.pass_executions = self.pass_executions.saturating_add(1);
    }

    pub(super) fn record_pass_data(&mut self, occurrence: MirPassOccurrence, data: &MirPassData) {
        self.processed_callables = self
            .processed_callables
            .saturating_add(count(data.processed_callables()));
        self.changed_callables = self
            .changed_callables
            .saturating_add(count(data.changed_callables()));

        for measurement in data.measurements() {
            match self.pass_measurements.iter_mut().find(|aggregate| {
                aggregate.identity == occurrence.identity()
                    && aggregate.measurement.name() == measurement.name()
            }) {
                Some(aggregate) => {
                    aggregate.measurement = MirPassMeasurement::count(
                        aggregate.measurement.name(),
                        aggregate
                            .measurement
                            .value()
                            .saturating_add(measurement.value()),
                    );
                }
                None => self.pass_measurements.push(MirPassAggregateMeasurement {
                    identity: occurrence.identity(),
                    pass_name: occurrence.name(),
                    measurement: *measurement,
                }),
            }
        }
    }

    pub(super) fn record_rewrite(
        &mut self,
        rewrite: &MirProgramRewriteResult,
    ) -> MirRewriteChangeSummary {
        let mut changes = MirRewriteChangeSummary::default();
        for callable in &rewrite.callables {
            changes.accumulate(callable.changes);
            self.rewrite_changes.accumulate(callable.changes);
        }
        changes
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        verification_executions: u64,
        pass_executions: u64,
        processed_callables: u64,
        changed_callables: u64,
        pass_measurements: Vec<(u16, &'static str, &'static str, u64)>,
    ) -> Self {
        Self {
            verification_executions,
            pass_executions,
            processed_callables,
            changed_callables,
            rewrite_changes: MirRewriteChangeSummary::default(),
            pass_measurements: pass_measurements
                .into_iter()
                .map(
                    |(identity, pass_name, measurement_name, value)| MirPassAggregateMeasurement {
                        identity: MirPassIdentity::new(identity),
                        pass_name,
                        measurement: MirPassMeasurement::count(measurement_name, value),
                    },
                )
                .collect(),
        }
    }
}

pub(crate) struct MeasuredMirPipeline {
    pub(crate) result: Result<VerifiedFinalMirProgram, MirPipelineError>,
    pub(crate) statistics: MirPipelineStatistics,
    occurrences: Vec<MirPassOccurrenceRecord>,
}

impl MeasuredMirPipeline {
    #[cfg(test)]
    pub(crate) fn occurrences(&self) -> &[MirPassOccurrenceRecord] {
        &self.occurrences
    }

    pub(crate) fn take_occurrences(&mut self) -> Vec<MirPassOccurrenceRecord> {
        std::mem::take(&mut self.occurrences)
    }

    pub(super) fn new(
        result: Result<VerifiedFinalMirProgram, MirPipelineError>,
        statistics: MirPipelineStatistics,
        occurrences: Vec<MirPassOccurrenceRecord>,
    ) -> Self {
        Self {
            result,
            statistics,
            occurrences,
        }
    }
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
