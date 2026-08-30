//! Final-MIR pipeline observation at its phase-owned boundary.

use std::time::Instant;

use crate::{
    mir::MirProgram,
    passes::{
        run_mir_pipeline_measured, run_mir_pipeline_with_occurrences, MeasuredMirPipeline,
        MirPassSchedule,
    },
    reporting::{ReportDetail, ReportEvent, ReportObserver, ReportOutcome, ReportPhase},
};

use super::super::statistics;

pub(in crate::driver) fn observe_mir_pipeline(
    observer: &mut dyn ReportObserver,
    program: MirProgram,
    schedule: &MirPassSchedule,
) -> MeasuredMirPipeline {
    if !observer.enabled(ReportDetail::Phases) {
        return run_mir_pipeline_measured(program, schedule);
    }

    let started = Instant::now();
    observer.observe(ReportEvent::PhaseStarted {
        phase: ReportPhase::MirPipeline,
    });
    let trace = observer.enabled(ReportDetail::Trace);
    let mut measured = if trace {
        run_mir_pipeline_with_occurrences(program, schedule)
    } else {
        run_mir_pipeline_measured(program, schedule)
    };

    if trace {
        for occurrence in measured.take_occurrences() {
            observer.observe(ReportEvent::MirPassFinished { occurrence });
        }
    }
    let outcome = if measured.result.is_ok() {
        ReportOutcome::Completed
    } else {
        ReportOutcome::Failed
    };
    let metrics = if observer.enabled(ReportDetail::Details) {
        statistics::mir_pipeline_metrics(&measured)
    } else {
        Vec::new()
    };
    observer.observe(ReportEvent::PhaseFinished {
        phase: ReportPhase::MirPipeline,
        elapsed: started.elapsed(),
        outcome,
        metrics,
    });
    measured
}
