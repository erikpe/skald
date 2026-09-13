use crate::{
    backend::Target,
    driver::compile_source_to_assembly_observed,
    reporting::{
        MetricValue, RecordingObserver, ReportDetail, ReportEvent, ReportMetric, ReportPhase,
    },
};

#[test]
fn details_publish_deterministic_phase_owned_metrics() {
    let source = "fn main() -> i64 { return 42; }";
    let mut observer = RecordingObserver::new(ReportDetail::Details);
    let artifact = compile_source_to_assembly_observed(
        "metrics.ska",
        source,
        Target::X86_64SysV,
        &mut observer,
    )
    .unwrap();
    let tokens = u64::try_from(crate::test_support::lex_source(source).2.tokens.len()).unwrap();

    assert_eq!(
        phase_metrics(observer.events(), ReportPhase::Lexing),
        &[
            ReportMetric::count("lex executions", 1),
            ReportMetric::bytes("source bytes", source.len() as u64),
            ReportMetric::count("tokens", tokens),
            ReportMetric::count("diagnostics", 0),
            ReportMetric::count("warnings", 0),
            ReportMetric::count("errors", 0),
        ]
    );
    assert_eq!(
        phase_metrics(observer.events(), ReportPhase::Resolution),
        &[
            ReportMetric::count("modules", 1),
            ReportMetric::count("function declarations", 1),
            ReportMetric::count("function definitions", 1),
            ReportMetric::count("class declarations", 0),
            ReportMetric::count("class definitions", 0),
            ReportMetric::count("interface declarations", 0),
            ReportMetric::count("semantic range discovery rounds", 0),
            ReportMetric::count("semantic range bodies revisited", 0),
            ReportMetric::count("semantic range interner copies", 0),
            ReportMetric::count("diagnostics", 0),
            ReportMetric::count("warnings", 0),
            ReportMetric::count("errors", 0),
        ]
    );
    assert_eq!(
        phase_metrics(observer.events(), ReportPhase::TypeChecking),
        &[
            ReportMetric::count("modules", 1),
            ReportMetric::count("function definitions", 1),
            ReportMetric::count("class definitions", 0),
            ReportMetric::count("diagnostics", 0),
            ReportMetric::count("warnings", 0),
            ReportMetric::count("errors", 0),
        ]
    );
    let preliminary = phase_metrics(observer.events(), ReportPhase::PreliminaryMirLowering);
    assert_eq!(preliminary[0], ReportMetric::count("definitions", 1));
    assert_eq!(preliminary[1], ReportMetric::count("blocks", 1));
    assert_eq!(
        metric_names(preliminary),
        ["definitions", "blocks", "instructions"]
    );
    assert_eq!(
        phase_metrics(observer.events(), ReportPhase::StaticLifecyclePlanning,),
        &[
            ReportMetric::count("effect summaries", 1),
            ReportMetric::count("dependencies", 0),
            ReportMetric::count("declared static fields", 0),
            ReportMetric::count("active static fields", 0),
            ReportMetric::count("inactive static fields", 0),
            ReportMetric::count("active explicit static fields", 0),
            ReportMetric::count("active zero-default static fields", 0),
            ReportMetric::count("inactive explicit static fields", 0),
            ReportMetric::count("activation execution nodes", 1),
            ReportMetric::count("activation edges", 0),
            ReportMetric::count("conservative activation targets", 0),
            ReportMetric::count("activation fields", 0),
            ReportMetric::count("shutdown fields", 0),
            ReportMetric::count("static initializers", 0),
        ]
    );
    let synthesis = phase_metrics(observer.events(), ReportPhase::StaticLifecycleSynthesis);
    assert_eq!(synthesis[0], ReportMetric::count("definitions", 1));
    assert_eq!(synthesis[1], ReportMetric::count("blocks", 1));
    let pipeline = phase_metrics(observer.events(), ReportPhase::MirPipeline);
    assert_eq!(
        pipeline[..14],
        [
            ReportMetric::count("verification executions", 2),
            ReportMetric::count("normalization executions", 1),
            ReportMetric::count("path-condition records consumed", 0),
            ReportMetric::count("logical-expression records consumed", 0),
            ReportMetric::count("path reads lowered", 0),
            ReportMetric::count("activation storage declarations reclassified", 0),
            ReportMetric::count("normalization changed callables", 0),
            ReportMetric::count("proof-protected blocks released", 0),
            ReportMetric::count("pass executions", 17),
            ReportMetric::count("processed callables", 17),
            ReportMetric::count("changed callables", 0),
            ReportMetric::count("retained MIR entities", 0),
            ReportMetric::count("inserted MIR entities", 0),
            ReportMetric::count("removed MIR entities", 0),
        ]
    );
    assert_eq!(
        pipeline[14],
        ReportMetric::pass_count(
            "dead-pure-definition-elimination",
            "removed assignment instructions",
            0,
        )
    );
    assert_eq!(
        pipeline[15],
        ReportMetric::pass_count(
            "dead-pure-definition-elimination",
            "removed value declarations",
            0,
        )
    );
    let folding = |name| ReportMetric::pass_count("primitive-constant-folding", name, 0);
    assert_eq!(
        pipeline[16..24],
        [
            folding("folded unary assignments"),
            folding("folded binary assignments"),
            folding("folded comparison assignments"),
            folding("folded cast assignments"),
            folding("folds crossing certified carriers"),
            folding("folds crossing checked protocols"),
            folding("folds crossing logical selections"),
            folding("maximum folded dependency depth"),
        ]
    );
    let algebra = |name| ReportMetric::pass_count("primitive-algebraic-simplification", name, 0);
    assert_eq!(
        pipeline[24..29],
        [
            algebra("constant-result rewrites"),
            algebra("forwarded value uses"),
            algebra("removed assignment instructions"),
            algebra("removed value declarations"),
            algebra("rejected protected-use candidates"),
        ]
    );
    let cast_chain =
        |name| ReportMetric::pass_count("integer-cast-chain-canonicalization", name, 0);
    assert_eq!(
        pipeline[29..37],
        [
            cast_chain("retargeted cast endpoints"),
            cast_chain("forwarded identity endpoints"),
            cast_chain("forwarded value uses"),
            cast_chain("removed assignment instructions"),
            cast_chain("removed value declarations"),
            cast_chain("eliminated cast steps"),
            cast_chain("rejected protected candidates"),
            cast_chain("maximum rewritten chain depth"),
        ]
    );
    let checked = |name| ReportMetric::pass_count("checked-integer-constant-folding", name, 0);
    assert_eq!(
        pipeline[37..43],
        [
            checked("folded quotient protocols"),
            checked("folded remainder protocols"),
            checked("folded shift protocols"),
            checked("folded protocols with propagated operands"),
            checked("removed protocol-load values"),
            checked("retained statically failing candidates"),
        ]
    );
    assert_eq!(
        pipeline[43..49],
        [
            ReportMetric::pass_count(
                "checked-f64-to-integer-constant-folding",
                "folded f64-to-i64 protocols",
                0,
            ),
            ReportMetric::pass_count(
                "checked-f64-to-integer-constant-folding",
                "folded f64-to-u64 protocols",
                0,
            ),
            ReportMetric::pass_count(
                "checked-f64-to-integer-constant-folding",
                "folded f64-to-u8 protocols",
                0,
            ),
            ReportMetric::pass_count(
                "checked-f64-to-integer-constant-folding",
                "folded protocols with propagated sources",
                0,
            ),
            ReportMetric::pass_count(
                "checked-f64-to-integer-constant-folding",
                "removed protocol values",
                0,
            ),
            ReportMetric::pass_count(
                "checked-f64-to-integer-constant-folding",
                "retained statically failing candidates",
                0,
            ),
        ]
    );
    let cfg = |name| ReportMetric::pass_count("conservative-cfg-cleanup", name, 0);
    assert_eq!(
        pipeline[49..54],
        [
            cfg("folded constant branches"),
            cfg("folded same-target branches"),
            cfg("removed blocks"),
            cfg("removed value declarations"),
            cfg("retained protected unreachable blocks"),
        ]
    );
    let logical = |name| ReportMetric::pass_count("constant-short-circuit-folding", name, 0);
    assert_eq!(
        pipeline[54..59],
        [
            logical("selected && short paths"),
            logical("selected && right paths"),
            logical("selected || short paths"),
            logical("selected || right paths"),
            logical("replaced selected-result loads"),
        ]
    );
    let final_cfg =
        |name| ReportMetric::pass_count("post-proof-unreachable-block-elimination", name, 0);
    assert_eq!(
        pipeline[59..62],
        [
            final_cfg("removed blocks"),
            final_cfg("removed value declarations"),
            final_cfg("retained permanent unreachable roots"),
        ]
    );
    let activation_cleanup =
        |name| ReportMetric::pass_count("dead-normalized-path-activation-cleanup", name, 0);
    assert_eq!(
        pipeline[62..71],
        [
            activation_cleanup("inspected normalized activation carriers"),
            activation_cleanup("removable normalized activation carriers"),
            activation_cleanup("protected normalized activation carriers"),
            activation_cleanup("removed storage declarations"),
            activation_cleanup("removed load instructions"),
            activation_cleanup("removed store instructions"),
            activation_cleanup("removed lifetime markers"),
            activation_cleanup("removed value declarations"),
            activation_cleanup("maximum removable protocol size"),
        ]
    );
    let forwarding = |name| ReportMetric::pass_count("post-proof-empty-block-forwarding", name, 0);
    assert_eq!(
        pipeline[71..75],
        [
            forwarding("removed forwarding blocks"),
            forwarding("redirected successor occurrences"),
            forwarding("retained cyclic forwarding blocks"),
            forwarding("retained permanent-attachment barriers"),
        ]
    );
    let merging = |name| ReportMetric::pass_count("post-proof-basic-block-merging", name, 0);
    assert_eq!(
        pipeline[75..80],
        [
            merging("merged block pairs"),
            merging("moved instructions"),
            merging("removed blocks"),
            merging("retained multiple-incoming-edge barriers"),
            merging("retained permanent-attachment barriers"),
        ]
    );
    let reachability =
        |name, value| ReportMetric::pass_count("whole-world-reachability", name, value);
    assert_eq!(
        pipeline[80..101],
        [
            reachability("examined definitions", 1),
            reachability("examined function definitions", 1),
            reachability("examined static-initializer definitions", 0),
            reachability("examined member definitions", 0),
            reachability("reachable definitions", 1),
            reachability("reachable function definitions", 1),
            reachability("reachable static-initializer definitions", 0),
            reachability("reachable member definitions", 0),
            reachability("removed definitions", 0),
            reachability("removed function definitions", 0),
            reachability("removed static-initializer definitions", 0),
            reachability("removed member definitions", 0),
            reachability("whole-program roots", 1),
            reachability("reachable execution nodes", 1),
            reachability("reachable callables", 1),
            reachability("dependency edges", 0),
            reachability("runtime entity targets", 0),
            reachability("virtual dispatch families", 0),
            reachability("interface dispatch requirements", 0),
            reachability("function-value signatures", 0),
            reachability("function-value targets", 0),
        ]
    );
    assert_eq!(
        pipeline[101..108],
        [
            ReportMetric::pass_count("local-constants", "analysis requests", 8),
            ReportMetric::pass_count("local-constants", "analysis computations", 1),
            ReportMetric::pass_count("local-constants", "analysis cache hits", 7),
            ReportMetric::pass_count("local-constants", "repeated snapshot requests", 7),
            ReportMetric::pass_count("local-constants", "results present before occurrences", 10,),
            ReportMetric::pass_count("local-constants", "results inserted", 1),
            ReportMetric::pass_count("local-constants", "results discarded", 1),
        ]
    );
    assert_eq!(pipeline[108], ReportMetric::count("definitions", 1));
    assert_eq!(pipeline[109], ReportMetric::count("blocks", 1));
    assert_eq!(
        phase_metrics(observer.events(), ReportPhase::BackendEmission),
        &[
            ReportMetric::bytes("assembly bytes", artifact.assembly.len() as u64),
            ReportMetric::count("assembly lines", artifact.assembly.lines().count() as u64,),
        ]
    );
}

#[test]
fn details_publish_productive_local_simplification_measurements() {
    let source = "
fn removed_target() -> i64 { return 99; }
fn identity(value: i64) -> i64 { return value + 0; }
fn floating() -> bool { return -((f64) 3) == -3.0; }
fn main() -> i64 {
    if (1 + 1 == 2) { return identity(6 * 7); }
    return removed_target();
}
";
    let mut observer = RecordingObserver::new(ReportDetail::Details);
    let artifact = compile_source_to_assembly_observed(
        "productive-local-simplification.ska",
        source,
        Target::X86_64SysV,
        &mut observer,
    )
    .unwrap();
    let metrics = phase_metrics(observer.events(), ReportPhase::MirPipeline);

    assert!(artifact.report.diagnostics.is_empty());
    assert_eq!(count_metric(metrics, "definitions"), Some(2));
    assert_eq!(count_metric(metrics, "blocks"), Some(2));
    assert!(
        pass_count_metric(
            metrics,
            "primitive-constant-folding",
            "folded binary assignments"
        ) > 0
    );
    for measurement in [
        "folded unary assignments",
        "folded comparison assignments",
        "folded cast assignments",
    ] {
        assert!(
            pass_count_metric(metrics, "primitive-constant-folding", measurement) > 0,
            "missing productive floating measurement {measurement}"
        );
    }
    assert!(
        pass_count_metric(
            metrics,
            "primitive-algebraic-simplification",
            "forwarded value uses"
        ) > 0
    );
    assert!(pass_count_metric(metrics, "conservative-cfg-cleanup", "removed blocks") > 0);
    assert_eq!(
        pass_count_metric(metrics, "whole-world-reachability", "removed definitions"),
        2
    );
}

#[test]
fn details_publish_productive_post_proof_cleanup_measurements() {
    let source = "
fn selected() -> bool { return true; }
fn main() -> i64 {
    if (true) { return 1; }
    if (false && selected()) { return 2; }
    return 3;
}
";
    let mut observer = RecordingObserver::new(ReportDetail::Details);
    let artifact = compile_source_to_assembly_observed(
        "productive-post-proof-cleanup.ska",
        source,
        Target::X86_64SysV,
        &mut observer,
    )
    .unwrap();
    let metrics = phase_metrics(observer.events(), ReportPhase::MirPipeline);

    assert!(artifact.report.diagnostics.is_empty());
    assert_eq!(count_metric(metrics, "normalization executions"), Some(1));
    assert_eq!(count_metric(metrics, "pass executions"), Some(17));
    assert_eq!(
        pass_count_metric(
            metrics,
            "constant-short-circuit-folding",
            "selected && short paths"
        ),
        1
    );
    assert_eq!(
        pass_count_metric(
            metrics,
            "constant-short-circuit-folding",
            "replaced selected-result loads"
        ),
        1
    );
    assert_eq!(
        pass_count_metric(
            metrics,
            "conservative-cfg-cleanup",
            "retained protected unreachable blocks"
        ),
        9
    );
    assert_eq!(
        pass_count_metric(
            metrics,
            "post-proof-unreachable-block-elimination",
            "removed blocks"
        ),
        9
    );
    assert_eq!(
        pass_count_metric(
            metrics,
            "post-proof-unreachable-block-elimination",
            "removed value declarations"
        ),
        10
    );
    assert_eq!(
        pass_count_metric(
            metrics,
            "dead-normalized-path-activation-cleanup",
            "inspected normalized activation carriers"
        ),
        1
    );
    assert_eq!(
        pass_count_metric(
            metrics,
            "dead-normalized-path-activation-cleanup",
            "removable normalized activation carriers"
        ),
        1
    );
    assert_eq!(
        pass_count_metric(
            metrics,
            "dead-normalized-path-activation-cleanup",
            "removed storage declarations"
        ),
        1
    );
    assert_eq!(
        pass_count_metric(
            metrics,
            "post-proof-empty-block-forwarding",
            "removed forwarding blocks"
        ),
        0
    );
    assert_eq!(
        pass_count_metric(
            metrics,
            "post-proof-empty-block-forwarding",
            "redirected successor occurrences"
        ),
        0
    );
    assert_eq!(
        pass_count_metric(
            metrics,
            "post-proof-basic-block-merging",
            "merged block pairs"
        ),
        1
    );
    assert_eq!(
        pass_count_metric(
            metrics,
            "post-proof-basic-block-merging",
            "moved instructions"
        ),
        1
    );
    assert_eq!(
        pass_count_metric(metrics, "post-proof-basic-block-merging", "removed blocks"),
        1
    );
    assert_eq!(count_metric(metrics, "removed MIR entities"), Some(22));
    assert_eq!(
        pass_count_metric(metrics, "whole-world-reachability", "removed definitions"),
        1
    );
}

#[test]
fn details_attribute_checked_integer_folding_and_followup_cfg_cleanup() {
    let mut observer = RecordingObserver::new(ReportDetail::Details);
    let artifact = compile_source_to_assembly_observed(
        "checked-integer-folding.ska",
        "fn main() -> i64 { return (6 * 7) / (1 + 1); }",
        Target::X86_64SysV,
        &mut observer,
    )
    .unwrap();
    let metrics = phase_metrics(observer.events(), ReportPhase::MirPipeline);

    assert!(artifact.report.diagnostics.is_empty());
    assert_eq!(count_metric(metrics, "pass executions"), Some(17));
    assert_eq!(
        pass_count_metric(
            metrics,
            "checked-integer-constant-folding",
            "folded quotient protocols"
        ),
        1
    );
    assert_eq!(
        pass_count_metric(
            metrics,
            "checked-integer-constant-folding",
            "removed protocol-load values"
        ),
        2
    );
    assert!(pass_count_metric(metrics, "conservative-cfg-cleanup", "removed blocks") > 0);
}

#[test]
fn details_attribute_checked_f64_to_integer_folding_and_followup_cleanup() {
    let mut observer = RecordingObserver::new(ReportDetail::Details);
    let artifact = compile_source_to_assembly_observed(
        "checked-f64-to-integer-folding.ska",
        "fn main() -> i64 { return (i64) 4.5; }",
        Target::X86_64SysV,
        &mut observer,
    )
    .unwrap();
    let metrics = phase_metrics(observer.events(), ReportPhase::MirPipeline);

    assert!(artifact.report.diagnostics.is_empty());
    assert_eq!(count_metric(metrics, "pass executions"), Some(17));
    assert_eq!(
        pass_count_metric(
            metrics,
            "checked-f64-to-integer-constant-folding",
            "folded f64-to-i64 protocols"
        ),
        1
    );
    assert_eq!(
        pass_count_metric(
            metrics,
            "checked-f64-to-integer-constant-folding",
            "removed protocol values"
        ),
        1
    );
    assert!(pass_count_metric(metrics, "conservative-cfg-cleanup", "removed blocks") > 0);
}

pub(super) fn phase_metrics(events: &[ReportEvent], phase: ReportPhase) -> &[ReportMetric] {
    events
        .iter()
        .find_map(|event| match event {
            ReportEvent::PhaseFinished {
                phase: finished,
                metrics,
                ..
            } if *finished == phase => Some(metrics.as_slice()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing finished phase {phase:?}"))
}

pub(super) fn metric_names(metrics: &[ReportMetric]) -> Vec<&'static str> {
    metrics.iter().map(ReportMetric::name).collect()
}

pub(super) fn count_metric(metrics: &[ReportMetric], name: &str) -> Option<u64> {
    metrics.iter().find_map(|metric| {
        (metric.name() == name).then(|| match metric.value() {
            MetricValue::Count(value) => value,
            MetricValue::Bytes(_) => panic!("`{name}` must be a count metric"),
        })
    })
}

fn pass_count_metric(metrics: &[ReportMetric], owner: &str, name: &str) -> u64 {
    metrics
        .iter()
        .find_map(|metric| {
            (metric.owner() == Some(owner) && metric.name() == name).then(|| match metric.value() {
                MetricValue::Count(value) => value,
                MetricValue::Bytes(_) => panic!("`{owner}: {name}` must be a count metric"),
            })
        })
        .unwrap_or_else(|| panic!("missing `{owner}: {name}` pass metric"))
}
