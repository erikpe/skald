//! Deterministic mapping from phase-owned products to reporting metrics.

use crate::{
    diagnostics::{Diagnostics, Severity},
    hir::HirProgram,
    lexer::LexOutput,
    mir::{
        MirProgram, MirProgramStatistics, MirVerificationErrors, PlannedMirProgram,
        PreliminaryMirProgram,
    },
    module::{MeasuredModuleGraphLoad, ModuleLoadMeasurements, ModuleParseStage},
    passes::{
        static_lifecycle::StaticLifecyclePlanningFailure, MeasuredMirPipeline,
        MirPipelineStatistics,
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
                count(planned.effects().summaries().len()),
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

pub(super) fn lifecycle_synthesis_metrics(
    result: &Result<MirProgram, MirVerificationErrors>,
) -> Vec<ReportMetric> {
    let Ok(program) = result else {
        return Vec::new();
    };
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
    let mut metrics = pipeline_execution_metrics(measured.statistics);
    if let Ok(program) = &measured.result {
        metrics.extend(mir_metrics(program.reporting_statistics()));
    }
    metrics
}

fn pipeline_execution_metrics(statistics: MirPipelineStatistics) -> Vec<ReportMetric> {
    vec![
        ReportMetric::count(
            "verification executions",
            statistics.verification_executions(),
        ),
        ReportMetric::count("pass executions", statistics.pass_executions()),
    ]
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
