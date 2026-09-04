use crate::{
    passes::{
        resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule, run_mir_pipeline_measured,
        run_mir_pipeline_with_occurrences, MirOptimizationProfile, MirPassMeasurement,
        MirPassOccurrenceOutcome, MirPassOccurrenceRecord,
    },
    test_support::{lower_source_to_final_mir, lower_source_to_mir},
};

use super::{super::dead_pure_definition_elimination, IDENTITY};

#[test]
fn reports_exact_unchanged_and_changed_accounting() {
    let unchanged = run_mir_pipeline_with_occurrences(
        lower_source_to_mir("fn main() -> i64 { return 0; }"),
        &schedule(&[IDENTITY]),
    );
    assert!(unchanged.result.is_ok());
    assert_eq!(unchanged.statistics.verification_executions(), 2);
    assert_eq!(unchanged.statistics.processed_callables(), 1);
    assert_eq!(unchanged.statistics.changed_callables(), 0);
    assert_eq!(
        unchanged.occurrences()[0].outcome(),
        MirPassOccurrenceOutcome::Unchanged
    );
    assert_eq!(
        measurement_value(&unchanged.occurrences()[0], "examined definitions"),
        1
    );
    assert_eq!(
        measurement_value(&unchanged.occurrences()[0], "reachable definitions"),
        1
    );
    assert_eq!(
        measurement_value(&unchanged.occurrences()[0], "removed definitions"),
        0
    );

    let changed = run_mir_pipeline_with_occurrences(
        lower_source_to_final_mir(
            "fn first_dead() -> i64 { return 1; }
             fn second_dead() -> i64 { return 2; }
             fn main() -> i64 { return 0; }",
        ),
        &schedule(&[IDENTITY, IDENTITY]),
    );
    let verified = changed.result.as_ref().unwrap();
    assert_eq!(verified.definitions.len(), 1);
    assert_eq!(changed.statistics.verification_executions(), 3);
    assert_eq!(changed.statistics.processed_callables(), 4);
    assert_eq!(changed.statistics.changed_callables(), 2);
    assert_eq!(
        changed
            .occurrences()
            .iter()
            .map(|record| (record.occurrence(), record.outcome()))
            .collect::<Vec<_>>(),
        [
            (0, MirPassOccurrenceOutcome::Changed),
            (1, MirPassOccurrenceOutcome::Unchanged),
        ]
    );
    assert_eq!(
        measurement_value(&changed.occurrences()[0], "examined definitions"),
        3
    );
    assert_eq!(
        measurement_value(&changed.occurrences()[0], "reachable definitions"),
        1
    );
    assert_eq!(
        measurement_value(&changed.occurrences()[0], "removed definitions"),
        2
    );
    assert_eq!(
        measurement_value(&changed.occurrences()[1], "removed definitions"),
        0
    );
    assert_eq!(
        changed
            .statistics
            .pass_measurements()
            .find(|(_, _, measurement)| measurement.name() == "removed definitions")
            .unwrap()
            .2,
        MirPassMeasurement::count("removed definitions", 2)
    );
}

#[test]
fn composes_after_proof_rich_cleanup() {
    let source = "fn dead() -> i64 { var unused: i64 = 1 + 2; return 3; }
                  fn main() -> i64 { var unused: i64 = 4 + 5; return 0; }";
    let reachability_only =
        run_mir_pipeline_measured(lower_source_to_final_mir(source), &schedule(&[IDENTITY]))
            .result
            .unwrap();
    let after = run_mir_pipeline_measured(
        lower_source_to_final_mir(source),
        &schedule(&[dead_pure_definition_elimination::IDENTITY, IDENTITY]),
    )
    .result
    .unwrap();

    assert_eq!(reachability_only, after);
    assert_eq!(after.definitions.len(), 1);
}

#[test]
fn exact_and_repeated_schedules_preserve_static_activation_authority() {
    let complete = lower_source_to_final_mir(
        "fn active_value() -> i64 { return 1; }
         fn inactive_value() -> i64 { return 2; }
         class State {
           static active: i64 = active_value();
           static inactive: i64 = inactive_value();
           init() {}
         }
         fn main() -> i64 { var unused: i64 = 3; return State.active; }",
    );
    let expected = complete
        .static_lifecycle
        .as_ref()
        .unwrap()
        .lifecycle()
        .proof()
        .activation()
        .clone();
    assert_eq!(expected.len(), 1);

    let dead = dead_pure_definition_elimination::IDENTITY;
    for identities in [
        Vec::new(),
        vec![dead],
        vec![IDENTITY],
        vec![dead, IDENTITY],
        vec![IDENTITY, IDENTITY],
        vec![dead, IDENTITY, IDENTITY],
    ] {
        let output = run_mir_pipeline_measured(complete.clone(), &schedule(&identities))
            .result
            .unwrap();
        assert_eq!(
            output
                .static_lifecycle
                .as_ref()
                .unwrap()
                .lifecycle()
                .proof()
                .activation(),
            &expected
        );
    }
}

#[test]
fn supported_profiles_preserve_complete_mir_unless_reachability_is_enabled() {
    let source = concat!(
        "class Dormant {\n",
        "  init() {}\n",
        "  copy(ref other: Dormant) {}\n",
        "  assign(ref other: Dormant) {}\n",
        "  destroy {}\n",
        "  fn method() -> i64 { return 1; }\n",
        "  static fn static_method() -> i64 { return 2; }\n",
        "}\n",
        "fn dead() -> i64 { return 3; }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let complete = lower_source_to_final_mir(source);
    let none = run_profile(complete.clone(), MirOptimizationProfile::None, &[]);
    let reachability_disabled = run_profile(
        complete.clone(),
        MirOptimizationProfile::Default,
        &["whole-world-reachability"],
    );
    let all_disabled = run_profile(
        complete.clone(),
        MirOptimizationProfile::Default,
        &[
            "conservative-cfg-cleanup",
            "dead-pure-definition-elimination",
            "primitive-algebraic-simplification",
            "primitive-constant-folding",
            "whole-world-reachability",
        ],
    );
    let default = run_profile(complete.clone(), MirOptimizationProfile::Default, &[]);

    assert_eq!(none.program(), &complete);
    assert_eq!(reachability_disabled, none);
    assert_eq!(all_disabled, none);
    assert_eq!(none.program().executable_definitions().count(), 8);
    assert_eq!(default.program().executable_definitions().count(), 1);
    assert_eq!(default.declarations, none.declarations);
    assert_eq!(default.classes, none.classes);
    assert_eq!(default.interfaces, none.interfaces);
}

fn run_profile(
    program: crate::mir::MirProgram,
    profile: MirOptimizationProfile,
    disabled: &[&str],
) -> crate::passes::VerifiedFinalMirProgram {
    let schedule = resolve_mir_pass_schedule(profile, disabled.iter().copied()).unwrap();
    run_mir_pipeline_measured(program, &schedule)
        .result
        .expect("profile must accept valid final MIR")
}

fn schedule(identities: &[crate::passes::MirPassIdentity]) -> crate::passes::MirPassSchedule {
    resolve_exact_mir_pass_schedule(identities).unwrap()
}

fn measurement_value(record: &MirPassOccurrenceRecord, name: &str) -> u64 {
    record
        .measurements()
        .iter()
        .find(|measurement| measurement.name() == name)
        .unwrap_or_else(|| panic!("missing `{name}` pass measurement"))
        .value()
}
