//! Deterministic mapping from phase-owned products to reporting metrics.

use crate::{
    diagnostics::{Diagnostics, Severity},
    hir::HirProgram,
    lexer::LexOutput,
    mir::{MirProgram, MirProgramStatistics, MirVerificationErrors, PreliminaryMirProgram},
    module::{MeasuredModuleGraphLoad, ModuleLoadMeasurements, ModuleParseStage},
    passes::{
        static_lifecycle::{PlannedMirProgram, StaticLifecyclePlanningFailure},
        MeasuredMirPipeline, MirPipelineStatistics,
    },
    reporting::{
        ReportDetail, ReportEvent, ReportMetric, ReportModuleStage, ReportObserver, ReportOutcome,
    },
    resolve::ResolvedProgram,
    syntax::ParseOutput,
    typeck::TypeCheckOutput,
};

pub(super) fn lexing_metrics(output: &LexOutput, source_bytes: usize) -> Vec<ReportMetric> {
    let mut metrics = vec![
        ReportMetric::count("lex executions", 1),
        ReportMetric::bytes("source bytes", count(source_bytes)),
        ReportMetric::count("tokens", count(output.tokens.len())),
    ];
    metrics.extend(diagnostic_metrics(&output.diagnostics));
    metrics
}

pub(super) fn parsing_metrics(output: &ParseOutput, tokens: usize) -> Vec<ReportMetric> {
    let mut metrics = vec![
        ReportMetric::count("parse executions", 1),
        ReportMetric::count("tokens", count(tokens)),
    ];
    metrics.extend(diagnostic_metrics(&output.diagnostics));
    metrics
}

pub(super) fn module_loading_metrics(
    measured: &MeasuredModuleGraphLoad,
    observer: &mut dyn ReportObserver,
) -> Vec<ReportMetric> {
    if observer.enabled(ReportDetail::Trace) {
        for parsed in measured.measurements.parses() {
            observer.observe(ReportEvent::ModuleParsed {
                module: parsed.module().to_string(),
                stage: match parsed.stage() {
                    ModuleParseStage::Discovery => ReportModuleStage::Discovery,
                    ModuleParseStage::Final => ReportModuleStage::Final,
                },
                tokens: parsed.tokens(),
                outcome: if parsed.completed() {
                    ReportOutcome::Completed
                } else {
                    ReportOutcome::Failed
                },
            });
        }
    }

    let measurements = &measured.measurements;
    let mut metrics = module_measurement_metrics(measurements);
    if let Err(failure) = &measured.result {
        metrics.extend(diagnostic_metrics(failure.diagnostics()));
    }
    metrics
}

fn module_measurement_metrics(measurements: &ModuleLoadMeasurements) -> Vec<ReportMetric> {
    vec![
        ReportMetric::count("reached modules", measurements.reached_modules()),
        ReportMetric::count("source reads", measurements.source_reads()),
        ReportMetric::bytes("source bytes", measurements.source_bytes()),
        ReportMetric::count(
            "discovery lex executions",
            measurements.discovery_lex_executions(),
        ),
        ReportMetric::count(
            "discovery parse executions",
            measurements.discovery_parse_executions(),
        ),
        ReportMetric::count("discovery tokens", measurements.discovery_tokens()),
        ReportMetric::count("final lex executions", measurements.final_lex_executions()),
        ReportMetric::count(
            "final parse executions",
            measurements.final_parse_executions(),
        ),
        ReportMetric::count("final tokens", measurements.final_tokens()),
    ]
}

pub(super) fn resolution_metrics(
    program: &ResolvedProgram,
    diagnostics: &Diagnostics,
) -> Vec<ReportMetric> {
    let mut metrics = vec![
        ReportMetric::count("modules", count(program.modules.len())),
        ReportMetric::count("function declarations", count(program.declarations.len())),
        ReportMetric::count("function definitions", count(program.definitions.len())),
        ReportMetric::count("class declarations", count(program.classes.len())),
        ReportMetric::count("class definitions", count(program.class_definitions.len())),
        ReportMetric::count("interface declarations", count(program.interfaces.len())),
    ];
    metrics.extend(diagnostic_metrics(diagnostics));
    metrics
}

pub(super) fn type_checking_metrics(output: &TypeCheckOutput) -> Vec<ReportMetric> {
    let mut metrics = output.hir.as_ref().map(hir_metrics).unwrap_or_default();
    metrics.extend(diagnostic_metrics(&output.diagnostics));
    metrics
}

fn hir_metrics(program: &HirProgram) -> Vec<ReportMetric> {
    vec![
        ReportMetric::count("modules", count(program.modules.len())),
        ReportMetric::count("function definitions", count(program.definitions.len())),
        ReportMetric::count("class definitions", count(program.class_definitions.len())),
    ]
}

pub(super) fn preliminary_mir_metrics(program: &PreliminaryMirProgram) -> Vec<ReportMetric> {
    mir_metrics(program.reporting_statistics())
}

pub(super) fn verification_metrics<T>(
    result: &Result<T, MirVerificationErrors>,
) -> Vec<ReportMetric> {
    let mut metrics = vec![ReportMetric::count("verification executions", 1)];
    if let Err(errors) = result {
        metrics.push(ReportMetric::count(
            "verification errors",
            count(errors.len()),
        ));
    }
    metrics
}

pub(super) fn lifecycle_planning_metrics(
    result: &Result<PlannedMirProgram, StaticLifecyclePlanningFailure>,
) -> Vec<ReportMetric> {
    match result {
        Ok(planned) => vec![
            ReportMetric::count(
                "effect summaries",
                count(planned.planning_report().analysis().summaries().len()),
            ),
            ReportMetric::count("dependencies", count(planned.dependencies().len())),
            ReportMetric::count(
                "activation fields",
                count(planned.lifecycle().activation().len()),
            ),
            ReportMetric::count(
                "shutdown fields",
                count(planned.lifecycle().shutdown().len()),
            ),
            ReportMetric::count(
                "static initializers",
                count(planned.static_initializers().len()),
            ),
        ],
        Err(failure) => {
            let mut diagnostics = 0usize;
            let mut warnings = 0usize;
            let mut errors = 0usize;
            for diagnostic in failure.diagnostics() {
                diagnostics += 1;
                match diagnostic.severity {
                    Severity::Warning => warnings += 1,
                    Severity::Error => errors += 1,
                }
            }
            vec![
                ReportMetric::count("dependencies", count(failure.dependencies().len())),
                ReportMetric::count("diagnostics", count(diagnostics)),
                ReportMetric::count("warnings", count(warnings)),
                ReportMetric::count("errors", count(errors)),
            ]
        }
    }
}

pub(super) fn lifecycle_synthesis_metrics(program: &MirProgram) -> Vec<ReportMetric> {
    let mut metrics = mir_metrics(program.reporting_statistics());
    if let Some(coordinator) = &program.static_lifecycle {
        metrics.extend([
            ReportMetric::count(
                "lifecycle definitions",
                count(coordinator.lifecycle().definitions().len()),
            ),
            ReportMetric::count("activation regions", count(coordinator.activation().len())),
            ReportMetric::count("shutdown regions", count(coordinator.shutdown().len())),
        ]);
    }
    metrics
}

pub(super) fn mir_pipeline_metrics(measured: &MeasuredMirPipeline) -> Vec<ReportMetric> {
    mir_pipeline_metrics_from(
        measured.statistics,
        measured
            .result
            .as_ref()
            .ok()
            .map(|program| program.program()),
    )
}

fn mir_pipeline_metrics_from(
    statistics: MirPipelineStatistics,
    program: Option<&MirProgram>,
) -> Vec<ReportMetric> {
    let mut metrics = pipeline_execution_metrics(statistics);
    if let Some(program) = program {
        metrics.extend(mir_metrics(program.reporting_statistics()));
    }
    metrics
}

fn pipeline_execution_metrics(statistics: MirPipelineStatistics) -> Vec<ReportMetric> {
    let mut metrics = vec![
        ReportMetric::count(
            "verification executions",
            statistics.verification_executions(),
        ),
        ReportMetric::count("pass executions", statistics.pass_executions()),
    ];
    if statistics.pass_executions() != 0 {
        let changes = statistics.rewrite_changes();
        metrics.extend([
            ReportMetric::count("rewritten callables", statistics.rewritten_callables()),
            ReportMetric::count(
                "retained MIR entities",
                u64::try_from(changes.retained()).unwrap_or(u64::MAX),
            ),
            ReportMetric::count(
                "inserted MIR entities",
                u64::try_from(changes.inserted()).unwrap_or(u64::MAX),
            ),
            ReportMetric::count(
                "removed MIR entities",
                u64::try_from(changes.removed()).unwrap_or(u64::MAX),
            ),
        ]);
    }
    metrics
}

fn mir_metrics(statistics: MirProgramStatistics) -> Vec<ReportMetric> {
    vec![
        ReportMetric::count("definitions", statistics.definitions()),
        ReportMetric::count("blocks", statistics.blocks()),
        ReportMetric::count("instructions", statistics.instructions()),
    ]
}

pub(super) fn backend_metrics<E>(result: &Result<String, E>) -> Vec<ReportMetric> {
    let Ok(assembly) = result else {
        return Vec::new();
    };
    vec![
        ReportMetric::bytes("assembly bytes", count(assembly.len())),
        ReportMetric::count("assembly lines", count(assembly.lines().count())),
    ]
}

fn diagnostic_metrics(diagnostics: &Diagnostics) -> Vec<ReportMetric> {
    let warnings = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Warning)
        .count();
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .count();
    vec![
        ReportMetric::count("diagnostics", count(diagnostics.len())),
        ReportMetric::count("warnings", count(warnings)),
        ReportMetric::count("errors", count(errors)),
    ]
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use crate::{
        passes::{resolve_mir_pass_schedule, run_mir_pipeline_measured, MirOptimizationProfile},
        reporting::MetricValue,
        test_support::lower_source_to_final_mir,
    };

    use super::*;

    #[test]
    fn empty_pipeline_metrics_report_verification_without_phantom_pass_work() {
        let schedule =
            resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty()).unwrap();
        let measured = run_mir_pipeline_measured(
            lower_source_to_final_mir("fn main() -> i64 { return 0; }"),
            &schedule,
        );
        let program = measured
            .result
            .as_ref()
            .ok()
            .map(|verified| verified.program());
        let metrics = mir_pipeline_metrics_from(measured.statistics, program);

        assert_eq!(metric(&metrics, "verification executions"), Some(1));
        assert_eq!(metric(&metrics, "pass executions"), Some(0));
        assert_eq!(metric(&metrics, "rewritten callables"), None);
        assert_eq!(metric(&metrics, "retained MIR entities"), None);
    }

    fn metric(metrics: &[ReportMetric], name: &str) -> Option<u64> {
        metrics.iter().find_map(|metric| {
            (metric.name() == name).then(|| match metric.value() {
                MetricValue::Count(value) => value,
                MetricValue::Bytes(_) => panic!("pipeline execution metrics are counts"),
            })
        })
    }
}
