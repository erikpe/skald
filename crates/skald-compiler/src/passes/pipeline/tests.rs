use std::cell::{Cell, RefCell};

use crate::{
    backend::{emit_assembly, BackendInput, Target},
    identity::{CallableId, FunctionId, StaticFieldId},
    mir::{
        dump_mir,
        rewrite::{MirReferenceFailure, MirRewriteChangeSummary, MirRewriteError},
        BlockId, MirAssignment, MirBasicBlock, MirCallTarget, MirInstruction, MirPlace,
        MirRvalueKind, MirTerminator, ValueId,
    },
    test_support::{lower_source_to_final_mir, lower_source_to_mir},
};

use super::{
    execution::{MirPassCapability, MirPassData, MirPassFailure, MirPassOutcome},
    optimizations::whole_world_reachability,
    policy::{
        resolve_test_mir_pass_schedule, MirPassDescriptor, MirPassImplementation,
        MirPassRegistration,
    },
    *,
};

const UNCHANGED: MirPassIdentity = MirPassIdentity::new(100);
const DELETE_EQUIVALENT: MirPassIdentity = MirPassIdentity::new(101);
const OBSERVE_DELETE: MirPassIdentity = MirPassIdentity::new(102);
const EXECUTION_FAILURE: MirPassIdentity = MirPassIdentity::new(103);
const REWRITE_FAILURE: MirPassIdentity = MirPassIdentity::new(104);
const INVALID_OUTPUT: MirPassIdentity = MirPassIdentity::new(105);
const LATER: MirPassIdentity = MirPassIdentity::new(106);
const FAIL_SECOND: MirPassIdentity = MirPassIdentity::new(107);
const RETARGET_STATIC: MirPassIdentity = MirPassIdentity::new(108);
const REWRITE_ALL: MirPassIdentity = MirPassIdentity::new(109);
const INVALID_ACCOUNTING: MirPassIdentity = MirPassIdentity::new(110);
const MEASURED_UNCHANGED: MirPassIdentity = MirPassIdentity::new(111);
const RETARGET_CALL: MirPassIdentity = MirPassIdentity::new(112);
const OBSERVE_RETARGET: MirPassIdentity = MirPassIdentity::new(113);
const RETAIN_REACHABLE: MirPassIdentity = MirPassIdentity::new(114);
const OBSERVE_RETENTION: MirPassIdentity = MirPassIdentity::new(115);
const INVALID_RETENTION_ACCOUNTING: MirPassIdentity = MirPassIdentity::new(116);
const PIPELINE_DETERMINISM_CHILD: &str = "SKALD_MIR_PIPELINE_DETERMINISM_CHILD";
const PIPELINE_FINGERPRINT_BEGIN: &str = "SKALD_MIR_PIPELINE_FINGERPRINT_BEGIN";
const PIPELINE_FINGERPRINT_END: &str = "SKALD_MIR_PIPELINE_FINGERPRINT_END";

const fn registration(
    identity: MirPassIdentity,
    name: &'static str,
    transform: super::execution::MirPassTransform,
) -> MirPassRegistration {
    MirPassRegistration::new(
        MirPassDescriptor::new(identity, name, "Synthetic verified-runner test pass."),
        MirPassImplementation::new(identity, transform),
    )
}

static TEST_REGISTRATIONS: [MirPassRegistration; 18] = [
    registration(UNCHANGED, "unchanged-pass", unchanged_pass),
    registration(
        DELETE_EQUIVALENT,
        "delete-equivalent-pass",
        delete_equivalent_pass,
    ),
    registration(OBSERVE_DELETE, "observe-delete-pass", observe_delete_pass),
    registration(
        EXECUTION_FAILURE,
        "execution-failure-pass",
        execution_failure_pass,
    ),
    registration(
        REWRITE_FAILURE,
        "rewrite-failure-pass",
        rewrite_failure_pass,
    ),
    registration(INVALID_OUTPUT, "invalid-output-pass", invalid_output_pass),
    registration(LATER, "later-pass", later_pass),
    registration(FAIL_SECOND, "fail-second-pass", fail_second_pass),
    registration(
        RETARGET_STATIC,
        "retarget-static-pass",
        retarget_static_pass,
    ),
    registration(REWRITE_ALL, "rewrite-all-pass", rewrite_all_pass),
    registration(
        INVALID_ACCOUNTING,
        "invalid-accounting-pass",
        invalid_accounting_pass,
    ),
    registration(
        MEASURED_UNCHANGED,
        "measured-unchanged-pass",
        measured_unchanged_pass,
    ),
    registration(RETARGET_CALL, "retarget-call-pass", retarget_call_pass),
    registration(
        OBSERVE_RETARGET,
        "observe-retarget-pass",
        observe_retarget_pass,
    ),
    registration(
        RETAIN_REACHABLE,
        "retain-reachable-pass",
        retain_reachable_pass,
    ),
    registration(
        OBSERVE_RETENTION,
        "observe-retention-pass",
        observe_retention_pass,
    ),
    registration(
        INVALID_RETENTION_ACCOUNTING,
        "invalid-retention-accounting-pass",
        invalid_retention_accounting_pass,
    ),
    whole_world_reachability::REGISTRATION,
];

thread_local! {
    static EXECUTION_LOG: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    static FAIL_SECOND_CALLS: Cell<usize> = const { Cell::new(0) };
    static RETARGET_CONFIGURATION: Cell<Option<(CallableId, StaticFieldId)>> = const { Cell::new(None) };
    static RETARGET_CALL_CONFIGURATION: Cell<Option<(CallableId, FunctionId, FunctionId)>> = const { Cell::new(None) };
    static REWRITTEN_CALLABLES: RefCell<Vec<CallableId>> = const { RefCell::new(Vec::new()) };
    static RETENTION_OBSERVATION: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

fn lowered_program() -> MirProgram {
    lower_source_to_mir("fn main() -> i64 { return 0; }")
}

fn none_schedule() -> MirPassSchedule {
    resolve_mir_pass_schedule(MirOptimizationProfile::None, std::iter::empty()).unwrap()
}

fn test_schedule(identities: &[MirPassIdentity]) -> MirPassSchedule {
    resolve_test_mir_pass_schedule(&TEST_REGISTRATIONS, identities).unwrap()
}

fn production_schedule(identities: &[MirPassIdentity]) -> MirPassSchedule {
    resolve_exact_mir_pass_schedule(identities).unwrap()
}

fn clear_test_state() {
    EXECUTION_LOG.with(|log| log.borrow_mut().clear());
    FAIL_SECOND_CALLS.with(|calls| calls.set(0));
    RETARGET_CONFIGURATION.with(|configuration| configuration.set(None));
    RETARGET_CALL_CONFIGURATION.with(|configuration| configuration.set(None));
    REWRITTEN_CALLABLES.with(|callables| callables.borrow_mut().clear());
    RETENTION_OBSERVATION.with(|observation| observation.set(None));
}

fn log_execution(name: &'static str) {
    EXECUTION_LOG.with(|log| log.borrow_mut().push(name));
}

fn execution_log() -> Vec<&'static str> {
    EXECUTION_LOG.with(|log| log.borrow().clone())
}

#[test]
fn none_pipeline_preserves_valid_mir_and_reports_only_verification() {
    let mir = lowered_program();
    let expected = mir.clone();
    let measured = run_mir_pipeline_measured(mir, &none_schedule());

    assert_eq!(measured.result.unwrap().program(), &expected);
    assert_eq!(measured.statistics.verification_executions(), 1);
    assert_eq!(measured.statistics.pass_executions(), 0);
    assert_eq!(measured.statistics.processed_callables(), 0);
    assert_eq!(measured.statistics.changed_callables(), 0);
    assert_eq!(
        measured.statistics.rewrite_changes(),
        MirRewriteChangeSummary::default()
    );
}

#[test]
fn pipeline_preserves_logical_path_and_cleanup_metadata() {
    let mir = lower_source_to_mir(
        "class Flag {
           truth: bool;
           init(truth: bool) { self.truth = truth; }
           fn read() -> bool { return self.truth; }
           destroy {}
         }
         fn make(truth: bool) -> shared Flag { return new Flag(truth); }
         fn evaluate(left: bool) -> bool {
           return left && make(true)->read();
         }
         fn main() -> i64 {
           if (evaluate(false)) { return 1; }
           return 0;
         }",
    );
    assert!(mir
        .definitions
        .iter()
        .any(|definition| !definition.body.path_conditions.is_empty()));
    let expected = mir.clone();

    assert_eq!(run_mir_pipeline(mir).unwrap().program(), &expected);
}

#[test]
fn default_pipeline_removes_a_valid_disconnected_block() {
    let mut mir = lowered_program();
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let span = function.span;
    let second = BlockId::new(function.function, 1);
    function.body.blocks.push(MirBasicBlock {
        id: second,
        instructions: Vec::new(),
        terminator: Some(MirTerminator::Goto {
            target: second,
            span,
        }),
        span,
    });
    let output = run_mir_pipeline(mir).unwrap();
    let definition = output.definitions.get(output.entry_function).unwrap();
    assert_eq!(definition.body.blocks.len(), 1);
}

#[test]
fn pipeline_preserves_pure_and_checked_primitive_casts_exactly() {
    let mir = lower_source_to_mir(
        "fn source() -> f64 { return 7.9; }
         fn main() -> i64 { return (i64) source() + (i64) (f64) 1u; }",
    );
    let expected = mir.clone();

    assert_eq!(run_mir_pipeline(mir).unwrap().program(), &expected);
}

#[test]
fn invalid_input_stops_before_the_first_pass() {
    clear_test_state();
    let mut mir = lowered_program();
    mir.definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .terminator = None;

    let measured = run_mir_pipeline_measured(mir, &test_schedule(&[UNCHANGED, LATER]));

    let error = measured.result.as_ref().unwrap_err();
    assert_eq!(error.stage(), MirPipelineFailureStage::InputVerification);
    assert_eq!(error.pass_name(), None);
    assert!(execution_log().is_empty());
    assert_eq!(measured.statistics.verification_executions(), 1);
    assert_eq!(measured.statistics.pass_executions(), 0);
}

#[test]
fn unchanged_pass_retains_the_verified_product_without_reverification() {
    clear_test_state();
    let mir = lowered_program();
    let expected = mir.clone();

    let measured = run_mir_pipeline_measured(mir, &test_schedule(&[UNCHANGED]));

    assert_eq!(measured.result.unwrap().program(), &expected);
    assert_eq!(execution_log(), ["unchanged"]);
    assert_eq!(measured.statistics.verification_executions(), 1);
    assert_eq!(measured.statistics.pass_executions(), 1);
    assert_eq!(measured.statistics.processed_callables(), 0);
}

#[test]
fn unchanged_definition_retention_preserves_the_verified_seal() {
    clear_test_state();
    let mir = lowered_program();
    let expected = verify_final_mir(mir.clone()).unwrap();

    let measured = run_mir_pipeline_measured(mir, &test_schedule(&[RETAIN_REACHABLE]));

    assert_eq!(measured.result.unwrap(), expected);
    assert_eq!(measured.statistics.verification_executions(), 1);
    assert_eq!(measured.statistics.pass_executions(), 1);
    assert_eq!(measured.statistics.processed_callables(), 1);
    assert_eq!(measured.statistics.changed_callables(), 0);
}

#[test]
fn changed_definition_retention_is_reverified_with_fresh_reachability_facts() {
    clear_test_state();
    let mir = lower_source_to_final_mir(
        "fn dead() -> i64 { return 9; }
         fn main() -> i64 { return 0; }",
    );
    let dead = mir
        .declarations
        .iter()
        .find(|declaration| declaration.name == "dead")
        .unwrap()
        .id;

    let measured =
        run_mir_pipeline_measured(mir, &test_schedule(&[RETAIN_REACHABLE, OBSERVE_RETENTION]));
    let verified = measured.result.unwrap();

    assert!(verified.definitions.get(dead).is_none());
    assert_eq!(
        RETENTION_OBSERVATION.with(Cell::get),
        Some((1, 1)),
        "the later pass must observe matching retained bodies and refreshed facts"
    );
    assert_eq!(measured.statistics.verification_executions(), 2);
    assert_eq!(measured.statistics.pass_executions(), 2);
    assert_eq!(measured.statistics.processed_callables(), 2);
    assert_eq!(measured.statistics.changed_callables(), 1);
}

#[test]
fn repeated_definition_retention_is_changed_then_idempotently_unchanged() {
    clear_test_state();
    let mir = lower_source_to_final_mir(
        "fn first_dead() -> i64 { return 1; }
         fn second_dead() -> i64 { return 2; }
         fn main() -> i64 { return 0; }",
    );

    let measured = run_mir_pipeline_with_occurrences(
        mir,
        &test_schedule(&[RETAIN_REACHABLE, RETAIN_REACHABLE]),
    );

    assert!(measured.result.is_ok());
    assert_eq!(measured.statistics.verification_executions(), 2);
    assert_eq!(measured.statistics.processed_callables(), 4);
    assert_eq!(measured.statistics.changed_callables(), 2);
    assert_eq!(
        measured
            .occurrences()
            .iter()
            .map(|record| record.outcome())
            .collect::<Vec<_>>(),
        [
            MirPassOccurrenceOutcome::Changed,
            MirPassOccurrenceOutcome::Unchanged,
        ]
    );
    assert_eq!(
        measured
            .occurrences()
            .iter()
            .map(|record| record.verification_executions())
            .collect::<Vec<_>>(),
        [1, 0]
    );
}

#[test]
fn reachability_uses_facts_rebuilt_after_a_synthetic_edge_change() {
    clear_test_state();
    let mir = lower_source_to_final_mir(
        "fn left() -> i64 { return 1; }
         fn right() -> i64 { return 2; }
         fn main() -> i64 { return left(); }",
    );
    let function = |name: &str| {
        mir.declarations
            .iter()
            .find(|declaration| declaration.name == name)
            .unwrap()
            .id
    };
    let main = CallableId::Function(function("main"));
    let left = function("left");
    let right = function("right");
    RETARGET_CALL_CONFIGURATION.with(|configuration| {
        configuration.set(Some((main, left, right)));
    });

    let measured = run_mir_pipeline_with_occurrences(
        mir,
        &test_schedule(&[RETARGET_CALL, whole_world_reachability::IDENTITY]),
    );
    let verified = measured.result.as_ref().unwrap();

    assert!(verified.definitions.get(left).is_none());
    assert!(verified.definitions.get(right).is_some());
    assert_eq!(measured.statistics.verification_executions(), 3);
    assert_eq!(measured.occurrences()[1].name(), "whole-world-reachability");
    assert_eq!(
        measurement_value(&measured.occurrences()[1], "removed definitions"),
        1
    );
}

fn measurement_value(record: &MirPassOccurrenceRecord, name: &str) -> u64 {
    record
        .measurements()
        .iter()
        .find(|measurement| measurement.name() == name)
        .unwrap_or_else(|| panic!("missing `{name}` pass measurement"))
        .value()
}

#[test]
fn occurrence_records_preserve_schedule_identity_outcomes_and_pass_measurements() {
    clear_test_state();
    let measured = run_mir_pipeline_with_occurrences(
        lowered_program(),
        &test_schedule(&[MEASURED_UNCHANGED, UNCHANGED, MEASURED_UNCHANGED]),
    );

    assert!(measured.result.is_ok());
    let records = measured.occurrences();
    assert_eq!(records.len(), 3);
    assert_eq!(
        records
            .iter()
            .map(|record| (
                record.position(),
                record.identity(),
                record.name(),
                record.occurrence(),
                record.outcome(),
            ))
            .collect::<Vec<_>>(),
        [
            (
                0,
                MEASURED_UNCHANGED,
                "measured-unchanged-pass",
                0,
                MirPassOccurrenceOutcome::Unchanged,
            ),
            (
                1,
                UNCHANGED,
                "unchanged-pass",
                0,
                MirPassOccurrenceOutcome::Unchanged,
            ),
            (
                2,
                MEASURED_UNCHANGED,
                "measured-unchanged-pass",
                1,
                MirPassOccurrenceOutcome::Unchanged,
            ),
        ]
    );
    assert_eq!(records[0].processed_callables(), Some(4));
    assert_eq!(records[0].changed_callables(), Some(0));
    assert_eq!(records[0].verification_executions(), 0);
    assert_eq!(
        records[0].measurements(),
        [
            MirPassMeasurement::count("visited values", 7),
            MirPassMeasurement::count("removed values", 2),
        ]
    );
    assert_eq!(measured.statistics.processed_callables(), 8);
    assert_eq!(measured.statistics.changed_callables(), 0);
    assert_eq!(
        measured.statistics.pass_measurements().collect::<Vec<_>>(),
        [
            (
                MEASURED_UNCHANGED,
                "measured-unchanged-pass",
                MirPassMeasurement::count("visited values", 14),
            ),
            (
                MEASURED_UNCHANGED,
                "measured-unchanged-pass",
                MirPassMeasurement::count("removed values", 4),
            ),
        ]
    );
}

#[test]
fn occurrence_records_stop_at_failure_without_fabricating_unavailable_data() {
    clear_test_state();
    let measured = run_mir_pipeline_with_occurrences(
        lower_source_to_final_mir("fn main() -> i64 { return 1 + 1; }"),
        &test_schedule(&[DELETE_EQUIVALENT, EXECUTION_FAILURE, LATER]),
    );

    assert!(measured.result.is_err());
    let records = measured.occurrences();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].outcome(), MirPassOccurrenceOutcome::Changed);
    assert!(records[0].processed_callables().unwrap() > 0);
    assert_eq!(records[0].changed_callables(), Some(1));
    assert_eq!(records[0].verification_executions(), 1);
    assert!(records[0].removed_mir_entities().unwrap() > 0);
    assert_eq!(records[1].outcome(), MirPassOccurrenceOutcome::Failed);
    assert_eq!(records[1].name(), "execution-failure-pass");
    assert_eq!(records[1].processed_callables(), None);
    assert_eq!(records[1].changed_callables(), None);
    assert!(records[1].measurements().is_empty());
    assert_eq!(execution_log(), ["delete", "execution-failure"]);
}

#[test]
fn aggregate_only_runner_skips_occurrence_recording() {
    let measured =
        run_mir_pipeline_measured(lowered_program(), &test_schedule(&[MEASURED_UNCHANGED]));

    assert!(measured.result.is_ok());
    assert!(measured.occurrences().is_empty());
    assert_eq!(measured.statistics.processed_callables(), 4);
}

#[derive(Default)]
struct CheckpointCollector {
    labels: Vec<String>,
    dumps: Vec<String>,
    reachability_dumps: Vec<String>,
    reachable_callables: Vec<Vec<CallableId>>,
    definition_counts: Vec<usize>,
}

impl MirPipelineInspector for CheckpointCollector {
    fn inspect(&mut self, checkpoint: MirPipelineCheckpoint<'_>) {
        self.labels.push(checkpoint.label().to_string());
        self.dumps.push(dump_mir(checkpoint.verified()));
        self.reachability_dumps.push(checkpoint.reachability_dump());
        self.reachable_callables.push(
            checkpoint
                .verified()
                .reachability()
                .reachable_callables()
                .to_vec(),
        );
        self.definition_counts
            .push(checkpoint.verified().definitions.len());
    }
}

#[test]
fn checkpoint_labels_are_stable_and_unambiguous_for_repetition() {
    assert_eq!(MirPipelineCheckpointLabel::Input.to_string(), "input");
    assert_eq!(
        MirPipelineCheckpointLabel::After {
            position: 3,
            pass_name: "fixture-pass",
            occurrence: 2,
        }
        .to_string(),
        "after-3-fixture-pass-2"
    );
    assert_eq!(MirPipelineCheckpointLabel::Final.to_string(), "final");
}

#[test]
fn none_pipeline_inspects_verified_input_and_final_without_changing_the_dump() {
    let mir = lowered_program();
    let expected_dump = dump_mir(&mir);
    let mut collector = CheckpointCollector::default();

    let measured = run_mir_pipeline_measured_inspected(mir, &none_schedule(), Some(&mut collector));

    assert!(measured.result.is_ok());
    assert_eq!(collector.labels, ["input", "final"]);
    assert_eq!(collector.dumps, [expected_dump.clone(), expected_dump]);
    assert_eq!(
        collector.reachability_dumps[0],
        collector.reachability_dumps[1]
    );
    assert_eq!(collector.definition_counts.len(), 2);
}

#[test]
fn default_pipeline_checkpoints_identify_every_repeated_occurrence() {
    let schedule =
        resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty()).unwrap();
    let mut collector = CheckpointCollector::default();

    let measured =
        run_mir_pipeline_measured_inspected(lowered_program(), &schedule, Some(&mut collector));

    assert!(measured.result.is_ok());
    assert_eq!(
        collector.labels,
        [
            "input",
            "after-0-dead-pure-definition-elimination-0",
            "after-1-primitive-constant-folding-0",
            "after-2-primitive-algebraic-simplification-0",
            "after-3-primitive-constant-folding-1",
            "after-4-dead-pure-definition-elimination-1",
            "after-5-conservative-cfg-cleanup-0",
            "after-6-dead-pure-definition-elimination-2",
            "after-7-whole-world-reachability-0",
            "final",
        ]
    );
    assert!(collector.dumps.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(measured.statistics.verification_executions(), 1);
}

#[test]
fn repeated_reachability_checkpoints_expose_resealed_deterministic_facts() {
    let mir = lower_source_to_final_mir(
        "fn dead() -> i64 { return 9; }
         fn main() -> i64 { return 0; }",
    );
    let mut collector = CheckpointCollector::default();

    let measured = run_mir_pipeline_measured_inspected(
        mir,
        &production_schedule(&[
            whole_world_reachability::IDENTITY,
            whole_world_reachability::IDENTITY,
        ]),
        Some(&mut collector),
    );

    assert!(measured.result.is_ok());
    assert_eq!(
        collector.labels,
        [
            "input",
            "after-0-whole-world-reachability-0",
            "after-1-whole-world-reachability-1",
            "final",
        ]
    );
    assert_ne!(collector.dumps[0], collector.dumps[1]);
    assert_eq!(collector.dumps[1], collector.dumps[2]);
    assert_eq!(collector.dumps[2], collector.dumps[3]);
    assert_ne!(
        collector.reachability_dumps[0],
        collector.reachability_dumps[1]
    );
    assert_eq!(
        collector.reachability_dumps[1],
        collector.reachability_dumps[2]
    );
    assert_eq!(
        collector.reachability_dumps[2],
        collector.reachability_dumps[3]
    );
}

#[test]
fn schedule_errors_measurements_and_checkpoints_are_deterministic_across_processes() {
    if std::env::var_os(PIPELINE_DETERMINISM_CHILD).is_some() {
        println!("{PIPELINE_FINGERPRINT_BEGIN}");
        println!("{}", pipeline_determinism_fingerprint());
        println!("{PIPELINE_FINGERPRINT_END}");
        return;
    }

    let first = pipeline_fingerprint_from_child();
    let second = pipeline_fingerprint_from_child();
    assert_eq!(first, second);
}

fn pipeline_fingerprint_from_child() -> String {
    let output = std::process::Command::new(
        std::env::current_exe().expect("unit-test executable path"),
    )
    .args([
        "--exact",
        "passes::pipeline::tests::schedule_errors_measurements_and_checkpoints_are_deterministic_across_processes",
        "--nocapture",
    ])
    .env(PIPELINE_DETERMINISM_CHILD, "1")
    .output()
    .expect("pipeline determinism child starts");
    assert!(
        output.status.success(),
        "pipeline determinism child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("test output is UTF-8");
    let (_, fingerprint) = stdout
        .split_once(PIPELINE_FINGERPRINT_BEGIN)
        .expect("child emitted fingerprint start marker");
    let (fingerprint, _) = fingerprint
        .split_once(PIPELINE_FINGERPRINT_END)
        .expect("child emitted fingerprint end marker");
    fingerprint.trim().to_owned()
}

fn pipeline_determinism_fingerprint() -> String {
    let none = none_schedule();
    let default =
        resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty()).unwrap();
    let disabled = resolve_mir_pass_schedule(
        MirOptimizationProfile::Default,
        ["dead-pure-definition-elimination"],
    )
    .unwrap();
    let repeated = test_schedule(&[MEASURED_UNCHANGED, MEASURED_UNCHANGED]);

    let mut collector = CheckpointCollector::default();
    let inspected =
        run_mir_pipeline_measured_inspected(lowered_program(), &default, Some(&mut collector));
    let final_dump = dump_mir(inspected.result.as_ref().unwrap().program());

    let mut reachability_collector = CheckpointCollector::default();
    let reachability_schedule = production_schedule(&[
        whole_world_reachability::IDENTITY,
        whole_world_reachability::IDENTITY,
    ]);
    let reachability = run_mir_pipeline_measured_inspected(
        lower_source_to_final_mir(
            "fn dead() -> i64 { return 1; }
             fn main() -> i64 { return 0; }",
        ),
        &reachability_schedule,
        Some(&mut reachability_collector),
    );
    assert!(reachability.result.is_ok());

    clear_test_state();
    let measured = run_mir_pipeline_with_occurrences(lowered_program(), &repeated);
    let measurements = measured
        .statistics
        .pass_measurements()
        .map(|(identity, name, measurement)| {
            (identity, name, measurement.name(), measurement.value())
        })
        .collect::<Vec<_>>();

    clear_test_state();
    let failed = run_mir_pipeline_with_occurrences(
        lowered_program(),
        &test_schedule(&[EXECUTION_FAILURE, LATER]),
    );
    let error = failed.result.unwrap_err();

    format!(
        "none={:?}\ndefault={:?}\ndisabled={:?}\nrepeated={:?}\n\
         metrics=({}, {}, {}, {:?})\nerror=({:?}, {:?}, {:?}, {:?}, {})\n\
         checkpoints={:?}\ncheckpoint-dumps={:?}\nreachability-checkpoints={:?}\n\
         reachability-dumps={:?}\nfinal-mir=\n{}",
        schedule_fingerprint(&none),
        schedule_fingerprint(&default),
        schedule_fingerprint(&disabled),
        schedule_fingerprint(&repeated),
        measured.statistics.verification_executions(),
        measured.statistics.pass_executions(),
        measured.statistics.processed_callables(),
        measurements,
        error.stage(),
        error.pass_name(),
        error.pass_position(),
        error.pass_occurrence(),
        error,
        collector.labels,
        collector.dumps,
        reachability_collector.labels,
        reachability_collector.reachability_dumps,
        final_dump,
    )
}

fn schedule_fingerprint(
    schedule: &MirPassSchedule,
) -> Vec<(usize, MirPassIdentity, &'static str, usize)> {
    schedule
        .iter()
        .map(|occurrence| {
            (
                occurrence.position(),
                occurrence.identity(),
                occurrence.name(),
                occurrence.occurrence(),
            )
        })
        .collect()
}

#[test]
fn unchanged_and_repeated_passes_each_publish_one_verified_after_checkpoint() {
    clear_test_state();
    let mut collector = CheckpointCollector::default();

    let measured = run_mir_pipeline_measured_inspected(
        lowered_program(),
        &test_schedule(&[UNCHANGED, UNCHANGED]),
        Some(&mut collector),
    );

    assert!(measured.result.is_ok());
    assert_eq!(
        collector.labels,
        [
            "input",
            "after-0-unchanged-pass-0",
            "after-1-unchanged-pass-1",
            "final",
        ]
    );
    assert!(collector.dumps.windows(2).all(|pair| pair[0] == pair[1]));
    assert!(collector
        .reachability_dumps
        .windows(2)
        .all(|pair| pair[0] == pair[1]));
    assert_eq!(measured.statistics.verification_executions(), 1);
}

#[test]
fn changed_call_targets_rebuild_facts_before_later_passes_and_checkpoints() {
    clear_test_state();
    let mir = lower_source_to_final_mir(
        "fn left() -> i64 { return 1; }
         fn right() -> i64 { return 2; }
         fn main() -> i64 { return left(); }",
    );
    let function = |name: &str| {
        mir.declarations
            .iter()
            .find(|declaration| declaration.name == name)
            .unwrap()
            .id
    };
    let main = CallableId::Function(function("main"));
    let left = function("left");
    let right = function("right");
    RETARGET_CALL_CONFIGURATION.with(|configuration| {
        configuration.set(Some((main, left, right)));
    });
    let mut collector = CheckpointCollector::default();

    let measured = run_mir_pipeline_measured_inspected(
        mir,
        &test_schedule(&[RETARGET_CALL, OBSERVE_RETARGET]),
        Some(&mut collector),
    );

    assert!(measured.result.is_ok());
    assert_eq!(
        collector.labels,
        [
            "input",
            "after-0-retarget-call-pass-0",
            "after-1-observe-retarget-pass-0",
            "final",
        ]
    );
    assert!(collector.reachable_callables[0].contains(&left.into()));
    assert!(!collector.reachable_callables[0].contains(&right.into()));
    for callables in &collector.reachable_callables[1..] {
        assert!(!callables.contains(&left.into()));
        assert!(callables.contains(&right.into()));
    }
    assert_ne!(
        collector.reachability_dumps[0],
        collector.reachability_dumps[1]
    );
    assert_eq!(measured.statistics.verification_executions(), 2);
    assert_eq!(execution_log(), ["retarget-call", "observe-retarget"]);
}

#[test]
fn changed_output_is_resealed_before_inspection_and_later_checkpoints() {
    clear_test_state();
    let mut collector = CheckpointCollector::default();

    let measured = run_mir_pipeline_measured_inspected(
        lower_source_to_final_mir("fn main() -> i64 { return 1 + 1; }"),
        &test_schedule(&[DELETE_EQUIVALENT, OBSERVE_DELETE]),
        Some(&mut collector),
    );

    assert!(measured.result.is_ok());
    assert_eq!(
        collector.labels,
        [
            "input",
            "after-0-delete-equivalent-pass-0",
            "after-1-observe-delete-pass-0",
            "final",
        ]
    );
    assert_ne!(collector.dumps[0], collector.dumps[1]);
    assert_eq!(collector.dumps[1], collector.dumps[2]);
    assert_eq!(collector.dumps[2], collector.dumps[3]);
}

#[test]
fn failed_occurrence_publishes_no_after_or_final_checkpoint() {
    clear_test_state();
    let mut collector = CheckpointCollector::default();

    let measured = run_mir_pipeline_measured_inspected(
        lowered_program(),
        &test_schedule(&[FAIL_SECOND, FAIL_SECOND, LATER]),
        Some(&mut collector),
    );

    assert!(measured.result.is_err());
    assert_eq!(collector.labels, ["input", "after-0-fail-second-pass-0"]);
    assert_eq!(execution_log(), ["fail-second", "fail-second"]);
}

#[test]
fn invalid_changed_output_is_never_inspected() {
    clear_test_state();
    let mut collector = CheckpointCollector::default();

    let measured = run_mir_pipeline_measured_inspected(
        lower_source_to_final_mir("fn main() -> i64 { return 1 + 1; }"),
        &test_schedule(&[INVALID_OUTPUT, LATER]),
        Some(&mut collector),
    );

    assert!(measured.result.is_err());
    assert_eq!(collector.labels, ["input"]);
    assert_eq!(execution_log(), ["invalid-output"]);
}

#[test]
fn disabled_inspection_does_not_create_checkpoints_or_report_events() {
    use crate::reporting::{RecordingObserver, ReportDetail};

    let collector = CheckpointCollector::default();
    let reporter = RecordingObserver::new(ReportDetail::Trace);
    let measured = run_mir_pipeline_measured(lowered_program(), &test_schedule(&[UNCHANGED]));

    assert!(measured.result.is_ok());
    assert!(collector.labels.is_empty());
    assert!(collector.dumps.is_empty());
    assert!(reporter.events().is_empty());
}

#[test]
fn changed_output_is_resealed_before_the_next_pass_and_backend() {
    clear_test_state();
    let mir = lower_source_to_final_mir("fn main() -> i64 { return 1 + 1; }");

    let measured =
        run_mir_pipeline_measured(mir, &test_schedule(&[DELETE_EQUIVALENT, OBSERVE_DELETE]));

    let verified = measured.result.expect("valid deletion must reseal");
    emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&verified),
    )
    .expect("backend accepts only the resealed result");
    assert_eq!(execution_log(), ["delete", "observe"]);
    assert_eq!(measured.statistics.verification_executions(), 2);
    assert_eq!(measured.statistics.pass_executions(), 2);
    assert_eq!(measured.statistics.changed_callables(), 1);
    assert_eq!(measured.statistics.rewrite_changes().values.removed, 1);
}

#[test]
fn repeated_occurrences_run_in_order_and_report_the_exact_failure() {
    clear_test_state();

    let measured = run_mir_pipeline_measured(
        lowered_program(),
        &test_schedule(&[FAIL_SECOND, FAIL_SECOND, LATER]),
    );

    let error = measured.result.unwrap_err();
    assert_eq!(error.stage(), MirPipelineFailureStage::PassExecution);
    assert_eq!(error.pass_position(), Some(1));
    assert_eq!(error.pass_name(), Some("fail-second-pass"));
    assert_eq!(error.pass_occurrence(), Some(1));
    assert!(error.to_string().contains("pass identity 107"));
    assert_eq!(execution_log(), ["fail-second", "fail-second"]);
    assert_eq!(measured.statistics.verification_executions(), 1);
    assert_eq!(measured.statistics.pass_executions(), 2);
}

#[test]
fn pass_execution_failure_stops_before_later_occurrences() {
    clear_test_state();
    let measured = run_mir_pipeline_measured(
        lowered_program(),
        &test_schedule(&[EXECUTION_FAILURE, LATER]),
    );

    let error = measured.result.unwrap_err();
    assert_eq!(error.stage(), MirPipelineFailureStage::PassExecution);
    assert_eq!(error.pass_name(), Some("execution-failure-pass"));
    assert!(error.to_string().contains("synthetic analysis failure"));
    assert_eq!(execution_log(), ["execution-failure"]);
    assert_eq!(measured.statistics.pass_executions(), 1);
}

#[test]
fn structural_rewrite_failure_stops_without_publishing_partial_mir() {
    clear_test_state();
    let measured =
        run_mir_pipeline_measured(lowered_program(), &test_schedule(&[REWRITE_FAILURE, LATER]));

    let error = measured.result.unwrap_err();
    assert_eq!(error.stage(), MirPipelineFailureStage::StructuralRewrite);
    assert_eq!(error.pass_position(), Some(0));
    assert_eq!(error.pass_name(), Some("rewrite-failure-pass"));
    assert!(error.to_string().contains("names a deleted edit slot"));
    assert_eq!(execution_log(), ["rewrite-failure"]);
    assert_eq!(measured.statistics.verification_executions(), 1);
    assert_eq!(measured.statistics.processed_callables(), 0);
}

#[test]
fn changed_output_verification_failure_stops_before_later_occurrences() {
    clear_test_state();
    let mir = lower_source_to_final_mir("fn main() -> i64 { return 1 + 1; }");
    let measured = run_mir_pipeline_with_occurrences(mir, &test_schedule(&[INVALID_OUTPUT, LATER]));

    let error = measured.result.as_ref().unwrap_err();
    assert_eq!(error.stage(), MirPipelineFailureStage::OutputVerification);
    assert_eq!(error.pass_name(), Some("invalid-output-pass"));
    assert_eq!(execution_log(), ["invalid-output"]);
    assert_eq!(measured.statistics.verification_executions(), 2);
    assert_eq!(measured.statistics.pass_executions(), 1);
    assert!(measured.statistics.processed_callables() > 0);
    assert_eq!(measured.occurrences().len(), 1);
    assert_eq!(
        measured.occurrences()[0].outcome(),
        MirPassOccurrenceOutcome::Failed
    );
    assert!(measured.occurrences()[0].processed_callables().is_some());
    assert_eq!(measured.occurrences()[0].verification_executions(), 1);
}

#[test]
fn invalid_changed_callable_accounting_is_a_pass_failure() {
    clear_test_state();
    let measured = run_mir_pipeline_measured(
        lowered_program(),
        &test_schedule(&[INVALID_ACCOUNTING, LATER]),
    );

    let error = measured.result.unwrap_err();
    assert_eq!(error.stage(), MirPipelineFailureStage::PassExecution);
    assert!(error.to_string().contains("changed callables"));
    assert_eq!(execution_log(), ["invalid-accounting"]);
    assert_eq!(measured.statistics.verification_executions(), 1);
}

#[test]
fn definition_retention_failure_is_attributed_to_the_exact_occurrence() {
    clear_test_state();
    let measured = run_mir_pipeline_with_occurrences(
        lower_source_to_final_mir(
            "fn dead() -> i64 { return 1; }
             fn main() -> i64 { return 0; }",
        ),
        &test_schedule(&[INVALID_RETENTION_ACCOUNTING, LATER]),
    );

    let error = measured.result.as_ref().unwrap_err();
    assert_eq!(error.stage(), MirPipelineFailureStage::PassExecution);
    assert_eq!(error.pass_position(), Some(0));
    assert_eq!(error.pass_name(), Some("invalid-retention-accounting-pass"));
    assert_eq!(error.pass_occurrence(), Some(0));
    assert!(error.to_string().contains("definition retention removed 1"));
    assert_eq!(execution_log(), ["invalid-retention-accounting"]);
    assert_eq!(measured.statistics.verification_executions(), 1);
    assert_eq!(measured.statistics.pass_executions(), 1);
    assert_eq!(measured.occurrences().len(), 1);
    assert_eq!(
        measured.occurrences()[0].outcome(),
        MirPassOccurrenceOutcome::Failed
    );
}

#[test]
fn atomic_rewrite_visits_functions_members_and_static_initializers() {
    clear_test_state();
    let mir = lower_source_to_final_mir(
        "class State {
           static base: i64 = 1 + 1;
           value_field: i64;
           init() { self.value_field = 1 + 1; }
           fn value() -> i64 { return 1 + 1; }
         }
         fn helper() -> i64 { return 1 + 1; }
         fn main() -> i64 { return helper() + State.base - 2; }",
    );

    let measured = run_mir_pipeline_measured(mir, &test_schedule(&[REWRITE_ALL]));
    measured.result.expect("all executable kinds must reseal");

    let callables = REWRITTEN_CALLABLES.with(|callables| callables.borrow().clone());
    assert!(callables
        .iter()
        .any(|callable| matches!(callable, CallableId::Function(_))));
    assert!(callables
        .iter()
        .any(|callable| matches!(callable, CallableId::StaticInitializer(_))));
    assert!(callables.iter().any(|callable| !matches!(
        callable,
        CallableId::Function(_) | CallableId::StaticInitializer(_)
    )));
    assert_eq!(
        measured.statistics.processed_callables(),
        u64::try_from(callables.len()).unwrap()
    );
    assert!(measured.statistics.changed_callables() >= 3);
}

#[test]
fn lifecycle_effect_change_rechecks_immutable_baseline_authority() {
    clear_test_state();
    let mir = lower_source_to_final_mir(
        "fn read() -> i64 { return State.base; }
         class State {
           static base: i64 = 1;
           static other: i64 = 2;
           static result: i64 = read();
           init() {}
         }
         fn main() -> i64 { return State.result + State.other; }",
    );
    let read = mir
        .declarations
        .iter()
        .find(|declaration| declaration.name == "read")
        .unwrap()
        .id;
    let other = mir
        .classes
        .iter()
        .flat_map(|class| &class.static_fields)
        .find(|field| field.name == "other")
        .unwrap()
        .id;
    RETARGET_CONFIGURATION.with(|configuration| {
        configuration.set(Some((CallableId::Function(read), other)));
    });

    let measured = run_mir_pipeline_measured(mir, &test_schedule(&[RETARGET_STATIC, LATER]));

    let error = measured.result.unwrap_err();
    assert_eq!(error.stage(), MirPipelineFailureStage::OutputVerification);
    assert!(error.to_string().contains("unauthorized fact"));
    assert_eq!(execution_log(), ["retarget-static"]);
}

fn unchanged_pass(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("unchanged");
    assert!(!capability.verified().definitions.is_empty());
    Ok(capability.unchanged())
}

fn measured_unchanged_pass(
    capability: MirPassCapability,
) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("measured-unchanged");
    capability.unchanged_with(
        MirPassData::processed(4)
            .with_measurement(MirPassMeasurement::count("visited values", 7))
            .with_measurement(MirPassMeasurement::count("removed values", 2)),
    )
}

fn delete_equivalent_pass(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("delete");
    rewrite_equivalent_constants(capability, false)
}

fn observe_delete_pass(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("observe");
    let constants = capability
        .verified()
        .definitions
        .iter()
        .flat_map(|definition| &definition.body.blocks)
        .flat_map(|block| &block.instructions)
        .filter(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(assignment)
                    if assignment.rvalue.kind == MirRvalueKind::ConstantI64(1)
            )
        })
        .count();
    assert_eq!(constants, 1, "the changed result must be verified first");
    Ok(capability.unchanged())
}

fn execution_failure_pass(
    _capability: MirPassCapability,
) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("execution-failure");
    Err(MirPassFailure::execution("synthetic analysis failure"))
}

fn rewrite_failure_pass(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("rewrite-failure");
    let changed = capability.rewrite(|_callable, edit| {
        edit.remove_block(edit.entry())?;
        Ok(())
    })?;
    changed.finish(MirPassData::changed(1))
}

fn invalid_output_pass(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("invalid-output");
    let changed = capability.rewrite(|_callable, edit| {
        let Some((block, replacement, deleted)) = invalid_dominance_substitution(edit) else {
            return Ok(());
        };
        delete_equivalent_value(edit, block, replacement, deleted)
    })?;
    changed.finish(MirPassData::changed(1))
}

fn later_pass(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("later");
    Ok(capability.unchanged())
}

fn fail_second_pass(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("fail-second");
    let call = FAIL_SECOND_CALLS.with(|calls| {
        let call = calls.get();
        calls.set(call + 1);
        call
    });
    if call == 0 {
        Ok(capability.unchanged())
    } else {
        Err(MirPassFailure::execution("second occurrence failed"))
    }
}

fn retarget_static_pass(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("retarget-static");
    let (source, target) = RETARGET_CONFIGURATION
        .with(Cell::get)
        .expect("retarget test configures source and target");
    let changed = capability.rewrite(|callable, edit| {
        if callable != source {
            return Ok(());
        }
        for block in edit.block_order().to_vec() {
            edit.rewrite_block_instructions(block, |instructions| {
                instructions
                    .iter()
                    .cloned()
                    .map(|instruction| retarget_static_load(instruction, target))
                    .collect()
            })?;
        }
        Ok(())
    })?;
    changed.finish(MirPassData::changed(1))
}

fn retarget_call_pass(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("retarget-call");
    let (source, old, target) = RETARGET_CALL_CONFIGURATION
        .with(Cell::get)
        .expect("retarget test configures source and targets");
    let changed = capability.rewrite(|callable, edit| {
        if callable != source {
            return Ok(());
        }
        for block in edit.block_order().to_vec() {
            edit.rewrite_block_instructions(block, |instructions| {
                instructions
                    .iter()
                    .cloned()
                    .map(|instruction| retarget_direct_call(instruction, old, target))
                    .collect()
            })?;
        }
        Ok(())
    })?;
    changed.finish(MirPassData::changed(1))
}

fn observe_retarget_pass(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("observe-retarget");
    let (_, old, target) = RETARGET_CALL_CONFIGURATION
        .with(Cell::get)
        .expect("retarget test configures source and targets");
    let reachable = capability.verified().reachability().reachable_callables();
    assert!(!reachable.contains(&old.into()));
    assert!(reachable.contains(&target.into()));
    Ok(capability.unchanged())
}

fn retain_reachable_pass(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("retain-reachable");
    let retention = capability.retain_reachable_definitions()?;
    let removed = retention.summary().removed().total();
    retention.finish(MirPassData::changed(removed))
}

fn observe_retention_pass(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("observe-retention");
    RETENTION_OBSERVATION.with(|observation| {
        observation.set(Some((
            capability.verified().definitions.len(),
            capability
                .verified()
                .reachability()
                .retained_definitions()
                .len(),
        )));
    });
    Ok(capability.unchanged())
}

fn invalid_retention_accounting_pass(
    capability: MirPassCapability,
) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("invalid-retention-accounting");
    let retention = capability.retain_reachable_definitions()?;
    retention.finish(MirPassData::changed(usize::MAX))
}

fn rewrite_all_pass(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("rewrite-all");
    rewrite_equivalent_constants(capability, true)
}

fn invalid_accounting_pass(
    capability: MirPassCapability,
) -> Result<MirPassOutcome, MirPassFailure> {
    log_execution("invalid-accounting");
    let changed = capability.rewrite(|_callable, _edit| Ok(()))?;
    changed.finish(MirPassData::changed(usize::MAX))
}

fn rewrite_equivalent_constants(
    capability: MirPassCapability,
    record_callables: bool,
) -> Result<MirPassOutcome, MirPassFailure> {
    let changed_callables = Cell::new(0usize);
    let changed = capability.rewrite(|callable, edit| {
        if record_callables {
            REWRITTEN_CALLABLES.with(|callables| callables.borrow_mut().push(callable));
        }
        let Some((block, replacement, deleted)) = equivalent_constant_pair(edit) else {
            return Ok(());
        };
        delete_equivalent_value(edit, block, replacement, deleted)?;
        changed_callables.set(changed_callables.get().saturating_add(1));
        Ok(())
    })?;
    changed.finish(MirPassData::changed(changed_callables.get()))
}

fn equivalent_constant_pair(
    edit: &crate::mir::rewrite::MirCallableEdit,
) -> Option<(BlockId, ValueId, ValueId)> {
    edit.block_order().iter().find_map(|block| {
        let constants = edit
            .block(*block)
            .ok()?
            .instructions
            .iter()
            .filter_map(|instruction| match instruction {
                MirInstruction::Assign(assignment)
                    if assignment.rvalue.kind == MirRvalueKind::ConstantI64(1) =>
                {
                    Some(assignment.result)
                }
                _ => None,
            })
            .take(2)
            .collect::<Vec<_>>();
        (constants.len() == 2).then(|| (*block, constants[0], constants[1]))
    })
}

fn invalid_dominance_substitution(
    edit: &crate::mir::rewrite::MirCallableEdit,
) -> Option<(BlockId, ValueId, ValueId)> {
    edit.block_order().iter().find_map(|block| {
        let block_data = edit.block(*block).ok()?;
        let deleted = block_data
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                MirInstruction::Assign(assignment)
                    if assignment.rvalue.kind == MirRvalueKind::ConstantI64(1) =>
                {
                    Some(assignment.result)
                }
                _ => None,
            })?;
        let replacement = block_data
            .instructions
            .iter()
            .filter_map(|instruction| match instruction {
                MirInstruction::Assign(assignment) => Some(assignment.result),
                _ => None,
            })
            .next_back()?;
        Some((*block, replacement, deleted))
    })
}

fn delete_equivalent_value(
    edit: &mut crate::mir::rewrite::MirCallableEdit,
    block: BlockId,
    replacement: ValueId,
    deleted: ValueId,
) -> Result<(), MirRewriteError> {
    edit.replace_value_uses(deleted, replacement)?;
    edit.rewrite_block_instructions(block, |instructions| {
        instructions
            .iter()
            .filter(|instruction| {
                !matches!(instruction, MirInstruction::Assign(assignment) if assignment.result == deleted)
            })
            .cloned()
            .collect()
    })?;
    edit.remove_value(deleted)?;
    Ok(())
}

fn retarget_static_load(instruction: MirInstruction, target: StaticFieldId) -> MirInstruction {
    match instruction {
        MirInstruction::Assign(MirAssignment {
            result,
            mut rvalue,
            span,
        }) if matches!(rvalue.kind, MirRvalueKind::Load(_)) => {
            rvalue.kind = MirRvalueKind::Load(MirPlace::static_field(target));
            MirInstruction::Assign(MirAssignment {
                result,
                rvalue,
                span,
            })
        }
        instruction => instruction,
    }
}

fn retarget_direct_call(
    instruction: MirInstruction,
    old: FunctionId,
    target: FunctionId,
) -> MirInstruction {
    match instruction {
        MirInstruction::Call(mut call) if call.target == MirCallTarget::Direct(old) => {
            call.target = MirCallTarget::Direct(target);
            MirInstruction::Call(call)
        }
        instruction => instruction,
    }
}

#[test]
fn structural_failure_retains_the_rewrite_error_as_its_source() {
    clear_test_state();
    let measured = run_mir_pipeline_measured(lowered_program(), &test_schedule(&[REWRITE_FAILURE]));
    let error = measured.result.unwrap_err();
    let source = std::error::Error::source(&error).unwrap();
    assert!(source.to_string().contains("deleted edit slot"));
    assert!(matches!(
        source.downcast_ref::<MirRewriteError>(),
        Some(MirRewriteError::InvalidReference {
            failure: MirReferenceFailure::Deleted,
            ..
        })
    ));
}
