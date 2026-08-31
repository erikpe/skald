use crate::{
    passes::{
        resolve_exact_mir_pass_schedule, run_mir_pipeline_measured,
        run_mir_pipeline_with_occurrences, MirPassMeasurement, MirPassOccurrenceOutcome,
        MirPassOccurrenceRecord,
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
    assert_eq!(unchanged.statistics.verification_executions(), 1);
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
    assert_eq!(changed.statistics.verification_executions(), 2);
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
fn composes_before_and_after_the_canary() {
    let source = "fn dead() -> i64 { var unused: i64 = 1 + 2; return 3; }
                  fn main() -> i64 { var unused: i64 = 4 + 5; return 0; }";
    let before = run_mir_pipeline_measured(
        lower_source_to_final_mir(source),
        &schedule(&[IDENTITY, dead_pure_definition_elimination::IDENTITY]),
    )
    .result
    .unwrap();
    let after = run_mir_pipeline_measured(
        lower_source_to_final_mir(source),
        &schedule(&[dead_pure_definition_elimination::IDENTITY, IDENTITY]),
    )
    .result
    .unwrap();

    assert_eq!(before, after);
    assert_eq!(before.definitions.len(), 1);
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
