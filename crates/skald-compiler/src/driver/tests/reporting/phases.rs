use std::fs;

use crate::{
    backend::Target,
    driver::{
        compile_request_to_assembly_observed_inspected, compile_source_to_assembly_observed,
        CompilationInspectors, EntrySelector,
    },
    mir::dump_mir,
    passes::{
        available_mir_passes, static_lifecycle::StaticActivationInspectionLabel, MirPassStage,
        MirPipelineCheckpoint,
    },
    reporting::{
        RecordingObserver, ReportDetail, ReportEvent, ReportMetric, ReportModuleStage,
        ReportOutcome, ReportPhase, ReportScope,
    },
    test_support::TemporaryDirectory,
};

use super::{inspection::default_mir_checkpoint_labels, metrics::phase_metrics, request::request};

pub(super) const SINGLETON_SUCCESS_PHASES: [ReportPhase; 11] = [
    ReportPhase::Lexing,
    ReportPhase::Parsing,
    ReportPhase::Resolution,
    ReportPhase::TypeChecking,
    ReportPhase::PreliminaryMirLowering,
    ReportPhase::PreliminaryMirVerification,
    ReportPhase::StaticLifecyclePlanning,
    ReportPhase::PlannedMirVerification,
    ReportPhase::StaticLifecycleSynthesis,
    ReportPhase::MirPipeline,
    ReportPhase::BackendEmission,
];

const REQUEST_SUCCESS_PHASES: [ReportPhase; 11] = [
    ReportPhase::ProviderNormalization,
    ReportPhase::ModuleLoading,
    ReportPhase::Resolution,
    ReportPhase::TypeChecking,
    ReportPhase::PreliminaryMirLowering,
    ReportPhase::PreliminaryMirVerification,
    ReportPhase::StaticLifecyclePlanning,
    ReportPhase::PlannedMirVerification,
    ReportPhase::StaticLifecycleSynthesis,
    ReportPhase::MirPipeline,
    ReportPhase::BackendEmission,
];

#[test]
fn singleton_success_observes_every_owned_phase_and_compilation_total() {
    let mut observer = RecordingObserver::new(ReportDetail::Phases);
    let artifact = compile_source_to_assembly_observed(
        "observed.ska",
        "fn main() -> i64 { return 42; }",
        Target::X86_64SysV,
        &mut observer,
    )
    .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains("mov rax, 42"));
    assert_observation(
        observer.events(),
        &completed(&SINGLETON_SUCCESS_PHASES),
        ReportOutcome::Completed,
    );
    assert!(observer.events().iter().all(|event| !matches!(
        event,
        ReportEvent::PhaseFinished { metrics, .. } if !metrics.is_empty()
    )));
}

#[test]
fn request_success_observes_loading_and_the_shared_compiler_pipeline() {
    let workspace = TemporaryDirectory::new("observed-request").unwrap();
    let root = workspace.join("modules");
    fs::create_dir_all(&root).unwrap();
    let source = "fn main() -> i64 { return 42; }\n";
    fs::write(root.join("app.ska"), source).unwrap();
    let request = request(
        &workspace,
        root,
        EntrySelector::Module("app".parse().unwrap()),
    );
    let mut observer = RecordingObserver::new(ReportDetail::Trace);
    let mut activation_labels = Vec::new();
    let mut activation_inspector =
        |inspection: crate::passes::static_lifecycle::StaticActivationInspection<'_>| {
            activation_labels.push(inspection.label());
        };
    let mut mir_labels = Vec::new();
    let mut mir_inspector = |checkpoint: MirPipelineCheckpoint<'_>| {
        mir_labels.push(checkpoint.label());
        let _dump = match checkpoint {
            MirPipelineCheckpoint::ProofRich(checkpoint) => dump_mir(checkpoint.verified()),
            MirPipelineCheckpoint::Final(checkpoint) => dump_mir(checkpoint.verified()),
        };
    };

    let artifact = compile_request_to_assembly_observed_inspected(
        &request,
        &mut observer,
        CompilationInspectors::new()
            .with_static_activation(&mut activation_inspector)
            .with_mir_pipeline(&mut mir_inspector),
    )
    .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains("mov rax, 42"));
    assert_eq!(
        activation_labels,
        [StaticActivationInspectionLabel::VerifiedPlanning]
    );
    assert_eq!(mir_labels, default_mir_checkpoint_labels());
    assert_observation(
        observer.events(),
        &completed(&REQUEST_SUCCESS_PHASES),
        ReportOutcome::Completed,
    );
    assert_eq!(
        observer
            .events()
            .iter()
            .filter_map(|event| match event {
                ReportEvent::MirPassFinished { occurrence } => Some((
                    occurrence.position(),
                    occurrence.stage(),
                    occurrence.name(),
                    occurrence.occurrence(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            (
                0,
                MirPassStage::ProofRich,
                "dead-pure-definition-elimination",
                0
            ),
            (1, MirPassStage::ProofRich, "primitive-constant-folding", 0),
            (
                2,
                MirPassStage::ProofRich,
                "primitive-algebraic-simplification",
                0
            ),
            (3, MirPassStage::ProofRich, "primitive-constant-folding", 1),
            (
                4,
                MirPassStage::ProofRich,
                "integer-cast-chain-canonicalization",
                0
            ),
            (
                5,
                MirPassStage::ProofRich,
                "checked-integer-constant-folding",
                0
            ),
            (
                6,
                MirPassStage::ProofRich,
                "checked-f64-to-integer-constant-folding",
                0
            ),
            (7, MirPassStage::ProofRich, "primitive-constant-folding", 2),
            (
                8,
                MirPassStage::ProofRich,
                "dead-pure-definition-elimination",
                1
            ),
            (9, MirPassStage::ProofRich, "conservative-cfg-cleanup", 0),
            (
                10,
                MirPassStage::ProofRich,
                "dead-pure-definition-elimination",
                2
            ),
            (
                11,
                MirPassStage::ProofTransition,
                "constant-short-circuit-folding",
                0
            ),
            (
                12,
                MirPassStage::Final,
                "post-proof-unreachable-block-elimination",
                0
            ),
            (
                13,
                MirPassStage::Final,
                "dead-normalized-path-activation-cleanup",
                0
            ),
            (
                14,
                MirPassStage::Final,
                "post-proof-empty-block-forwarding",
                0
            ),
            (15, MirPassStage::Final, "post-proof-basic-block-merging", 0),
            (16, MirPassStage::Final, "whole-world-reachability", 0),
        ]
    );
    let descriptors = available_mir_passes();
    for occurrence in observer.events().iter().filter_map(|event| match event {
        ReportEvent::MirPassFinished { occurrence } => Some(occurrence),
        _ => None,
    }) {
        let descriptor = descriptors
            .iter()
            .find(|descriptor| descriptor.name() == occurrence.name())
            .expect("every reported pass occurrence is registered");
        assert_eq!(occurrence.identity(), descriptor.identity());
        assert_eq!(occurrence.stage(), descriptor.stage());
    }
    let tokens = u64::try_from(crate::test_support::lex_source(source).2.tokens.len()).unwrap();
    assert_eq!(
        observer
            .events()
            .iter()
            .filter(|event| matches!(event, ReportEvent::ModuleParsed { .. }))
            .cloned()
            .collect::<Vec<_>>(),
        vec![
            ReportEvent::ModuleParsed {
                module: "app".to_owned(),
                stage: ReportModuleStage::Discovery,
                tokens,
                outcome: ReportOutcome::Completed,
            },
            ReportEvent::ModuleParsed {
                module: "app".to_owned(),
                stage: ReportModuleStage::Final,
                tokens,
                outcome: ReportOutcome::Completed,
            },
        ]
    );
    let loading_start = observer
        .events()
        .iter()
        .position(|event| {
            matches!(
                event,
                ReportEvent::PhaseStarted {
                    phase: ReportPhase::ModuleLoading,
                }
            )
        })
        .unwrap();
    let loading_finish = observer
        .events()
        .iter()
        .position(|event| {
            matches!(
                event,
                ReportEvent::PhaseFinished {
                    phase: ReportPhase::ModuleLoading,
                    ..
                }
            )
        })
        .unwrap();
    assert!(observer.events()[loading_start + 1..loading_finish]
        .iter()
        .all(|event| matches!(event, ReportEvent::ModuleParsed { .. })));
    assert_eq!(
        phase_metrics(observer.events(), ReportPhase::ModuleLoading),
        &[
            ReportMetric::count("reached modules", 1),
            ReportMetric::count("source reads", 1),
            ReportMetric::bytes("source bytes", source.len() as u64),
            ReportMetric::count("discovery lex executions", 1),
            ReportMetric::count("discovery parse executions", 1),
            ReportMetric::count("discovery tokens", tokens),
            ReportMetric::count("final lex executions", 1),
            ReportMetric::count("final parse executions", 1),
            ReportMetric::count("final tokens", tokens),
        ]
    );
}

pub(super) fn completed(phases: &[ReportPhase]) -> Vec<(ReportPhase, ReportOutcome)> {
    phases
        .iter()
        .copied()
        .map(|phase| (phase, ReportOutcome::Completed))
        .collect()
}

pub(super) fn assert_observation(
    events: &[ReportEvent],
    expected: &[(ReportPhase, ReportOutcome)],
    run_outcome: ReportOutcome,
) {
    let phase_events = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                ReportEvent::PhaseStarted { .. } | ReportEvent::PhaseFinished { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(phase_events.len(), expected.len() * 2, "{events:#?}");
    for (index, (phase, outcome)) in expected.iter().copied().enumerate() {
        let offset = index * 2;
        assert_eq!(*phase_events[offset], ReportEvent::PhaseStarted { phase });
        let ReportEvent::PhaseFinished {
            phase: finished,
            outcome: actual,
            ..
        } = phase_events[offset + 1]
        else {
            panic!("phase start was not followed by a finish: {events:#?}");
        };
        assert_eq!(*finished, phase);
        assert_eq!(*actual, outcome);
    }
    assert!(matches!(
        events.last(),
        Some(ReportEvent::RunFinished {
            scope: ReportScope::Compilation,
            outcome,
            ..
        }) if *outcome == run_outcome
    ));
}
