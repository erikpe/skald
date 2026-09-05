use crate::{
    identity::{CallableId, FunctionId},
    mir::{
        rewrite::local_cfg_facts_for_definition, MirDefinitionRef, MirInstruction, MirProgram,
        MirRvalueKind, MirStorageKind, MirTerminator, PathConditionId, ValueId,
    },
    passes::{
        run_mir_pipeline_with_occurrences, MirPassMeasurement, MirPassOccurrenceOutcome,
        MirPassStage,
    },
    test_support::lower_source_to_final_mir,
};

use super::{plan::LogicalSelectionPlanError, *};
use crate::passes::pipeline::optimizations::{
    local_constant::LogicalSelectionKind,
    logical_topology::{observe_logical_topologies, LogicalTopologyObservation},
};
use crate::passes::pipeline::{
    normalization::MirProofTransitionPlan,
    policy::{resolve_test_mir_pass_schedule, MirPassRegistration},
    seal::{transition_proof_mir, MirProofTransitionError},
    verify_proof_mir,
};

static REGISTRATIONS: [MirPassRegistration; 1] = [REGISTRATION];

fn schedule() -> crate::passes::MirPassSchedule {
    resolve_test_mir_pass_schedule(&REGISTRATIONS, &[IDENTITY]).unwrap()
}

fn callable_named(program: &MirProgram, name: &str) -> CallableId {
    CallableId::Function(
        program
            .declarations
            .iter()
            .find(|declaration| declaration.name == name)
            .unwrap()
            .id,
    )
}

fn definition(program: &MirProgram, callable: CallableId) -> MirDefinitionRef<'_> {
    program
        .executable_definitions()
        .find(|definition| definition.callable() == callable)
        .unwrap()
}

fn assigned_kind(definition: MirDefinitionRef<'_>, value: ValueId) -> &MirRvalueKind {
    definition
        .body()
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) if assignment.result == value => {
                Some(&assignment.rvalue.kind)
            }
            _ => None,
        })
        .unwrap()
}

fn goto_target(terminator: &Option<MirTerminator>) -> crate::mir::BlockId {
    match terminator {
        Some(MirTerminator::Goto { target, .. }) => *target,
        other => panic!("expected goto, found {other:?}"),
    }
}

fn terminator_span(terminator: &Option<MirTerminator>) -> crate::source::Span {
    match terminator {
        Some(MirTerminator::Branch { span, .. } | MirTerminator::Goto { span, .. }) => *span,
        other => panic!("expected branch or goto, found {other:?}"),
    }
}

#[test]
fn registration_has_the_stable_transition_identity() {
    let descriptor = REGISTRATION.descriptor();

    assert_eq!(descriptor.identity(), IDENTITY);
    assert_eq!(descriptor.name(), "constant-short-circuit-folding");
    assert_eq!(descriptor.stage(), MirPassStage::ProofTransition);
}

#[test]
fn all_constant_left_rules_select_the_exact_existing_path() {
    let input = lower_source_to_final_mir(concat!(
        "fn and_short(rhs: bool) -> bool { return false && rhs; }\n",
        "fn and_right(rhs: bool) -> bool { return true && rhs; }\n",
        "fn or_short(rhs: bool) -> bool { return true || rhs; }\n",
        "fn or_right(rhs: bool) -> bool { return false || rhs; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let plan = LogicalSelectionPlan::prepare(&input).unwrap();
    assert_eq!(plan.selection_count(), 4);
    assert_eq!(plan.changed_callable_count(), 4);

    let cases = [
        ("and_short", LogicalSelectionKind::Short, Some(false)),
        ("and_right", LogicalSelectionKind::Right, None),
        ("or_short", LogicalSelectionKind::Short, Some(true)),
        ("or_right", LogicalSelectionKind::Right, None),
    ];
    let before = cases
        .iter()
        .map(|(name, kind, result)| {
            let callable = callable_named(&input, name);
            let definition = definition(&input, callable);
            let topology = match &observe_logical_topologies(definition).unwrap()[0] {
                LogicalTopologyObservation::Protocol(topology) => (**topology).clone(),
                LogicalTopologyObservation::Rejected { .. } => unreachable!(),
            };
            (
                callable,
                *kind,
                *result,
                definition.body().blocks.len(),
                terminator_span(&definition.block(topology.split).unwrap().terminator),
                terminator_span(&definition.block(topology.selection).unwrap().terminator),
                topology,
            )
        })
        .collect::<Vec<_>>();

    let measured = run_mir_pipeline_with_occurrences(input, &schedule());
    let output = measured.result.as_ref().unwrap();
    assert_eq!(measured.occurrences().len(), 1);
    assert_eq!(
        measured.occurrences()[0].outcome(),
        MirPassOccurrenceOutcome::Changed
    );
    assert_eq!(measured.occurrences()[0].processed_callables(), Some(5));
    assert_eq!(measured.occurrences()[0].changed_callables(), Some(4));
    assert_eq!(
        measured.occurrences()[0].measurements(),
        [
            MirPassMeasurement::count(SELECTED_AND_SHORT_PATHS, 1),
            MirPassMeasurement::count(SELECTED_AND_RIGHT_PATHS, 1),
            MirPassMeasurement::count(SELECTED_OR_SHORT_PATHS, 1),
            MirPassMeasurement::count(SELECTED_OR_RIGHT_PATHS, 1),
            MirPassMeasurement::count(REPLACED_SELECTED_RESULTS, 2),
        ]
    );

    for (callable, kind, result, block_count, split_span, selection_span, topology) in before {
        let definition = definition(output.program(), callable);
        let expected_split = match kind {
            LogicalSelectionKind::Short => topology.inactive_predecessor,
            LogicalSelectionKind::Right => topology.active_predecessor,
        };
        let expected_selection = match kind {
            LogicalSelectionKind::Short => topology.short,
            LogicalSelectionKind::Right => topology.right_entry,
        };
        assert_eq!(
            goto_target(&definition.block(topology.split).unwrap().terminator),
            expected_split
        );
        assert_eq!(
            goto_target(&definition.block(topology.selection).unwrap().terminator),
            expected_selection
        );
        assert_eq!(
            terminator_span(&definition.block(topology.split).unwrap().terminator),
            split_span
        );
        assert_eq!(
            terminator_span(&definition.block(topology.selection).unwrap().terminator),
            selection_span
        );
        match result {
            Some(value) => assert_eq!(
                assigned_kind(definition, topology.selected_result),
                &MirRvalueKind::ConstantBool(value)
            ),
            None => assert!(matches!(
                assigned_kind(definition, topology.selected_result),
                MirRvalueKind::Load(_)
            )),
        }
        assert_eq!(definition.body().blocks.len(), block_count);
        assert!(definition.logical_expressions().is_empty());
        assert!(definition.path_conditions().is_empty());
        assert_eq!(
            definition.storage(topology.activation).unwrap().kind,
            MirStorageKind::ScalarSpill
        );
    }
}

#[test]
fn selected_right_constants_replace_only_the_protocol_result_load() {
    let input = lower_source_to_final_mir(concat!(
        "fn and_value() -> bool { return true && false; }\n",
        "fn or_value() -> bool { return false || true; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let logical = input
        .executable_definitions()
        .filter(|definition| !definition.logical_expressions().is_empty())
        .flat_map(|definition| {
            observe_logical_topologies(definition)
                .unwrap()
                .into_iter()
                .map(move |observation| (definition.callable(), observation))
        })
        .map(|(callable, observation)| match observation {
            LogicalTopologyObservation::Protocol(topology) => (callable, *topology),
            LogicalTopologyObservation::Rejected { .. } => unreachable!(),
        })
        .collect::<Vec<_>>();

    let output = run_mir_pipeline_with_occurrences(input, &schedule())
        .result
        .unwrap();
    for (callable, topology) in logical {
        assert!(matches!(
            assigned_kind(
                definition(output.program(), callable),
                topology.selected_result
            ),
            MirRvalueKind::ConstantBool(_)
        ));
    }
}

#[test]
fn nested_derived_and_checked_facts_select_in_one_transaction() {
    let input = lower_source_to_final_mir(concat!(
        "fn choose(flag: bool) -> bool {\n",
        "  return (((1 + 1) == 2) && ((8 / 2) == 4)) || flag;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let callable = callable_named(&input, "choose");
    let original = definition(&input, callable);
    let topologies = observe_logical_topologies(original)
        .unwrap()
        .into_iter()
        .map(|observation| match observation {
            LogicalTopologyObservation::Protocol(topology) => *topology,
            LogicalTopologyObservation::Rejected { .. } => unreachable!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        LogicalSelectionPlan::prepare(&input)
            .unwrap()
            .selection_count(),
        2
    );

    let output = run_mir_pipeline_with_occurrences(input, &schedule())
        .result
        .unwrap();
    let output = definition(output.program(), callable);
    for topology in topologies {
        assert!(matches!(
            output.block(topology.split).unwrap().terminator,
            Some(MirTerminator::Goto { .. })
        ));
        assert!(matches!(
            output.block(topology.selection).unwrap().terminator,
            Some(MirTerminator::Goto { .. })
        ));
    }
}

#[test]
fn skipped_failure_is_unreachable_while_selected_failure_remains_reachable() {
    let source = concat!(
        "fn skipped() -> bool { return false && ((1 / 0) == 0); }\n",
        "fn selected() -> bool { return true && ((1 / 0) == 0); }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let input = lower_source_to_final_mir(source);
    let checks = ["skipped", "selected"].map(|name| {
        let callable = callable_named(&input, name);
        let definition = definition(&input, callable);
        let check = definition
            .body()
            .blocks
            .iter()
            .find(|block| {
                matches!(
                    block.terminator,
                    Some(MirTerminator::IntegerDivisorCheck { .. })
                )
            })
            .unwrap()
            .id;
        (callable, check, definition.body().blocks.len())
    });

    let output = run_mir_pipeline_with_occurrences(input, &schedule())
        .result
        .unwrap();
    for (index, (callable, check, block_count)) in checks.into_iter().enumerate() {
        let definition = definition(output.program(), callable);
        let cfg = local_cfg_facts_for_definition(definition).unwrap();
        assert_eq!(definition.body().blocks.len(), block_count);
        assert_eq!(cfg.entry_reachable().contains(&check), index == 1);
        assert!(matches!(
            definition.block(check).unwrap().terminator,
            Some(MirTerminator::IntegerDivisorCheck { .. })
        ));
    }
}

#[test]
fn dynamic_rhs_calls_are_preserved_and_reached_only_on_the_selected_path() {
    let source = concat!(
        "fn effect() -> bool { return true; }\n",
        "fn skipped() -> bool { return true || effect(); }\n",
        "fn selected() -> bool { return false || effect(); }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let input = lower_source_to_final_mir(source);
    let calls = ["skipped", "selected"].map(|name| {
        let callable = callable_named(&input, name);
        let definition = definition(&input, callable);
        let call_block = definition
            .body()
            .blocks
            .iter()
            .find(|block| {
                block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, MirInstruction::Call(_)))
            })
            .unwrap()
            .id;
        (callable, call_block)
    });

    let output = run_mir_pipeline_with_occurrences(input, &schedule())
        .result
        .unwrap();
    for (index, (callable, call_block)) in calls.into_iter().enumerate() {
        let definition = definition(output.program(), callable);
        let cfg = local_cfg_facts_for_definition(definition).unwrap();
        assert_eq!(cfg.entry_reachable().contains(&call_block), index == 1);
        assert!(definition
            .block(call_block)
            .unwrap()
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, MirInstruction::Call(_))));
    }
}

#[test]
fn disconnected_skipped_rhs_does_not_poison_later_function_value_provenance() {
    let input = lower_source_to_final_mir(concat!(
        "fn effect() -> bool { return true; }\n",
        "fn invoke(callback: fn() -> bool) -> bool { return callback(); }\n",
        "fn main() -> i64 {\n",
        "  var callback: fn() -> bool = effect;\n",
        "  var preserved: i64 = 41;\n",
        "  var skipped: bool = false && effect();\n",
        "  if (invoke(callback) && !skipped) { return preserved - 41; }\n",
        "  return 1;\n",
        "}\n",
    ));

    let output = run_mir_pipeline_with_occurrences(input, &schedule())
        .result
        .expect("disconnected skipped paths must not become executable provenance inputs");

    assert!(output
        .executable_definitions()
        .all(|definition| definition.logical_expressions().is_empty()));
}

#[test]
fn unresolved_left_is_a_normalized_no_op_transition() {
    let input = lower_source_to_final_mir(
        "fn choose(left: bool, right: bool) -> bool { return left && right; } fn main() -> i64 { return 0; }",
    );
    assert!(LogicalSelectionPlan::prepare(&input).unwrap().is_empty());

    let measured = run_mir_pipeline_with_occurrences(input, &schedule());
    assert!(measured.result.is_ok());
    assert_eq!(measured.occurrences().len(), 1);
    assert_eq!(
        measured.occurrences()[0].outcome(),
        MirPassOccurrenceOutcome::Unchanged
    );
    assert_eq!(measured.occurrences()[0].changed_callables(), Some(0));
    assert_eq!(
        measured.occurrences()[0].measurements(),
        [
            MirPassMeasurement::count(SELECTED_AND_SHORT_PATHS, 0),
            MirPassMeasurement::count(SELECTED_AND_RIGHT_PATHS, 0),
            MirPassMeasurement::count(SELECTED_OR_SHORT_PATHS, 0),
            MirPassMeasurement::count(SELECTED_OR_RIGHT_PATHS, 0),
            MirPassMeasurement::count(REPLACED_SELECTED_RESULTS, 0),
        ]
    );
    assert_eq!(measured.statistics.normalization_executions(), 1);
    assert_eq!(measured.statistics.verification_executions(), 2);
}

#[test]
fn functions_members_and_static_lifecycle_bodies_share_one_plan() {
    let input = lower_source_to_final_mir(
        "class State {
           static selected: bool = true && false;
           initialized: bool;
           init() { self.initialized = true && false; }
           fn value() -> bool { return false || true; }
           destroy { var finished: bool = true && false; }
         }
         fn choose() -> bool { return true && false; }
         fn main() -> i64 {
           var state: State = State();
           if (state.value() || State.selected || choose()) { return 1; }
           return 0;
         }",
    );
    let plan = LogicalSelectionPlan::prepare(&input).unwrap();
    assert_eq!(plan.selection_count(), 5);

    let output = run_mir_pipeline_with_occurrences(input, &schedule())
        .result
        .unwrap();
    assert!(output
        .executable_definitions()
        .all(|definition| definition.logical_expressions().is_empty()
            && definition.path_conditions().is_empty()));
}

#[test]
fn stale_conflicting_and_malformed_plans_fail_without_publication() {
    let source =
        "fn choose(rhs: bool) -> bool { return false && rhs; } fn main() -> i64 { return 0; }";
    let input = lower_source_to_final_mir(source);
    let stale_plan = LogicalSelectionPlan::prepare(&input).unwrap();
    let mut changed = input.clone();
    let callable = callable_named(&changed, "choose");
    let definition = changed
        .definitions
        .get_mut_for_test(match callable {
            CallableId::Function(id) => id,
            _ => unreachable!(),
        })
        .unwrap();
    definition.body.blocks[0].span = definition.span;
    definition.body.blocks[0].instructions.reverse();
    assert!(matches!(
        stale_plan.validate_program(&changed),
        Err(crate::mir::rewrite::MirRewriteError::StaleCallableSnapshot { .. })
    ));

    let mut conflicting = LogicalSelectionPlan::prepare(&input).unwrap();
    conflicting.duplicate_first_candidate_for_test();
    let proof = verify_proof_mir(input.clone()).unwrap();
    assert!(matches!(
        transition_proof_mir(proof, Some(MirProofTransitionPlan::logical(conflicting))),
        Err(MirProofTransitionError::OptionalPlanRewrite(_))
    ));
    assert_eq!(input, lower_source_to_final_mir(source));

    let mut malformed = input.clone();
    let definition = malformed
        .definitions
        .get_mut_for_test(match callable {
            CallableId::Function(id) => id,
            _ => unreachable!(),
        })
        .unwrap();
    definition.body.path_conditions[0].merge = definition.body.logical_expressions[0].join;
    assert!(matches!(
        LogicalSelectionPlan::prepare(&malformed),
        Err(LogicalSelectionPlanError::RejectedTopology { .. })
    ));

    let mut foreign = input;
    let definition = foreign
        .definitions
        .get_mut_for_test(match callable {
            CallableId::Function(id) => id,
            _ => unreachable!(),
        })
        .unwrap();
    let CallableId::Function(function) = callable else {
        unreachable!()
    };
    definition.body.logical_expressions[0].condition =
        PathConditionId::new(FunctionId::new(function.index() + 1), 0);
    assert!(LogicalSelectionPlan::prepare(&foreign).is_err());
}

#[test]
fn independent_runs_are_dense_and_idempotent() {
    let source = "fn choose() -> bool { return (false && true) || (true && false); } fn main() -> i64 { return 0; }";
    let first = run_mir_pipeline_with_occurrences(lower_source_to_final_mir(source), &schedule())
        .result
        .unwrap();
    let second = run_mir_pipeline_with_occurrences(lower_source_to_final_mir(source), &schedule())
        .result
        .unwrap();

    assert_eq!(first, second);
    for definition in first.executable_definitions() {
        assert!(definition
            .values()
            .iter()
            .enumerate()
            .all(|(index, value)| value.id == ValueId::new(definition.callable(), index)));
        assert!(definition.body().blocks.iter().enumerate().all(
            |(index, block)| block.id == crate::mir::BlockId::new(definition.callable(), index)
        ));
    }
}
