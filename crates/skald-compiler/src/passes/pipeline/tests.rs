use std::cell::{Cell, RefCell};

use crate::{
    backend::{emit_assembly, BackendInput, Target},
    identity::{CallableId, StaticFieldId},
    mir::{
        rewrite::{MirReferenceFailure, MirRewriteChangeSummary, MirRewriteError},
        BlockId, MirAssignment, MirBasicBlock, MirInstruction, MirPlace, MirRvalueKind,
        MirTerminator, ValueId,
    },
    test_support::{lower_source_to_final_mir, lower_source_to_mir},
};

use super::{
    execution::{MirPassCapability, MirPassData, MirPassFailure, MirPassOutcome},
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

static TEST_REGISTRATIONS: [MirPassRegistration; 12] = [
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
];

thread_local! {
    static EXECUTION_LOG: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    static FAIL_SECOND_CALLS: Cell<usize> = const { Cell::new(0) };
    static RETARGET_CONFIGURATION: Cell<Option<(CallableId, StaticFieldId)>> = const { Cell::new(None) };
    static REWRITTEN_CALLABLES: RefCell<Vec<CallableId>> = const { RefCell::new(Vec::new()) };
}

fn lowered_program() -> MirProgram {
    lower_source_to_mir("fn main() -> i64 { return 0; }")
}

fn default_schedule() -> MirPassSchedule {
    resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty()).unwrap()
}

fn test_schedule(identities: &[MirPassIdentity]) -> MirPassSchedule {
    resolve_test_mir_pass_schedule(&TEST_REGISTRATIONS, identities).unwrap()
}

fn clear_test_state() {
    EXECUTION_LOG.with(|log| log.borrow_mut().clear());
    FAIL_SECOND_CALLS.with(|calls| calls.set(0));
    RETARGET_CONFIGURATION.with(|configuration| configuration.set(None));
    REWRITTEN_CALLABLES.with(|callables| callables.borrow_mut().clear());
}

fn log_execution(name: &'static str) {
    EXECUTION_LOG.with(|log| log.borrow_mut().push(name));
}

fn execution_log() -> Vec<&'static str> {
    EXECUTION_LOG.with(|log| log.borrow().clone())
}

#[test]
fn empty_pipeline_preserves_valid_mir_and_reports_only_verification() {
    let mir = lowered_program();
    let expected = mir.clone();
    let measured = run_mir_pipeline_measured(mir, &default_schedule());

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
         fn main() -> i64 { return 0; }",
    );
    assert!(mir
        .definitions
        .iter()
        .any(|definition| !definition.body.path_conditions.is_empty()));
    let expected = mir.clone();

    assert_eq!(run_mir_pipeline(mir).unwrap().program(), &expected);
}

#[test]
fn pipeline_preserves_valid_multi_block_mir() {
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
    let expected = mir.clone();

    assert_eq!(run_mir_pipeline(mir).unwrap().program(), &expected);
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
         fn main() -> i64 { return helper(); }",
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
         fn main() -> i64 { return 0; }",
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
