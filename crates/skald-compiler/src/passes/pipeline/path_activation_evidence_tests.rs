use crate::{
    backend::{emit_assembly, BackendInput, Target},
    mir::{dump_mir, MirInstruction, MirPlaceBase, MirStorageKind, StorageId},
    test_support::{
        assert_system_assembler_accepts, lower_source_to_final_mir, run_native_assembly,
    },
};

use super::{
    resolve_mir_pass_schedule, run_mir_pipeline_measured, MeasuredMirPipeline,
    MirOptimizationProfile, MirPassSchedule,
};

const SOURCE: &str = r#"
class Gate {
    static enabled: bool = true && (false || true);
    static permanent: Gate = Gate(true);
    value: bool;

    init(value: bool) {
        self.value = value && (false || true);
    }

    fn allows(other: bool) -> bool {
        return self.value && (other || false);
    }

    destroy {
        var observed: bool = self.value && true;
    }
}

fn identity(value: bool) -> bool {
    return value || false;
}

fn main() -> i64 {
    var callback: fn(bool) -> bool = identity;
    var maybe: i64? = some(7);
    var gate: Gate = Gate(true);
    var index: i64 = 0;
    var total: i64 = 0;

    while ((index < 2) && gate.allows(true)) {
        total = total + 1;
        index = index + 1;
    }

    if (false && callback(true)) {
        return 1;
    } elif (callback(Gate.enabled) && Gate.permanent.allows((maybe is some) || false)) {
        return maybe! + total + 33;
    }
    return 0;
}
"#;

const ALL_PASS_NAMES: [&str; 10] = [
    "checked-integer-constant-folding",
    "conservative-cfg-cleanup",
    "constant-short-circuit-folding",
    "dead-pure-definition-elimination",
    "post-proof-basic-block-merging",
    "post-proof-empty-block-forwarding",
    "post-proof-unreachable-block-elimination",
    "primitive-algebraic-simplification",
    "primitive-constant-folding",
    "whole-world-reachability",
];

#[test]
fn source_activation_provenance_is_stable_across_the_complete_profile_matrix() {
    let input = lower_source_to_final_mir(SOURCE);
    let schedules = [
        ("default", default_without([])),
        ("none", none()),
        (
            "logical-folding-disabled",
            default_without(["constant-short-circuit-folding"]),
        ),
        (
            "post-proof-unreachable-disabled",
            default_without(["post-proof-unreachable-block-elimination"]),
        ),
        (
            "post-proof-forwarding-disabled",
            default_without(["post-proof-empty-block-forwarding"]),
        ),
        (
            "post-proof-merging-disabled",
            default_without(["post-proof-basic-block-merging"]),
        ),
        (
            "reachability-disabled",
            default_without(["whole-world-reachability"]),
        ),
        ("all-passes-disabled", default_without(ALL_PASS_NAMES)),
    ];
    let mut outputs = Vec::new();

    for (name, schedule) in &schedules {
        let first = run_mir_pipeline_measured(input.clone(), schedule);
        let second = run_mir_pipeline_measured(input.clone(), schedule);
        assert_eq!(first.result, second.result, "{name}");
        assert_eq!(first.statistics, second.statistics, "{name}");
        assert_eq!(first.occurrences(), second.occurrences(), "{name}");

        let verified = first
            .result
            .as_ref()
            .unwrap_or_else(|error| panic!("{name} profile failed: {error}"));
        let dump = dump_mir(verified.program());
        assert!(
            dump.contains("normalized-path-activation <normalized-path-activation>"),
            "{name}: {dump}"
        );
        assert!(!dump.contains("path-condition <path-condition>"), "{name}");
        assert!(verified
            .program()
            .executable_definitions()
            .all(|definition| {
                definition.path_conditions().is_empty()
                    && definition.logical_expressions().is_empty()
                    && definition
                        .storage_entries()
                        .iter()
                        .all(|storage| storage.kind != MirStorageKind::PathCondition)
            }));

        let emitted = assembly(&first);
        assert_eq!(emitted, assembly(&second), "{name}");
        assert_system_assembler_accepts(&emitted);
        assert_eq!(run_native_assembly(&emitted).code(), Some(42), "{name}");
        outputs.push((name, dump, emitted));
    }

    let none = outputs
        .iter()
        .find(|(name, _, _)| **name == "none")
        .unwrap();
    let all_disabled = outputs
        .iter()
        .find(|(name, _, _)| **name == "all-passes-disabled")
        .unwrap();
    assert_eq!(none.1, all_disabled.1);
    assert_eq!(none.2, all_disabled.2);
}

#[test]
fn activation_references_and_lifetimes_fail_at_their_owning_contracts() {
    let verified = run_mir_pipeline_measured(lower_source_to_final_mir(SOURCE), &none())
        .result
        .expect("the unoptimized profile must produce verified final MIR");
    let base = verified.program().clone();
    let function = base.entry_function;
    let definition = base.definitions.get(function).unwrap();
    let activation = definition
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::NormalizedPathActivation)
        .map(|storage| storage.id)
        .expect("the source fixture must produce a normalized path activation");
    let callable = definition.callable();

    let mut invalid_place = base.clone();
    let definition = invalid_place
        .definitions
        .get_mut_for_test(function)
        .unwrap();
    let store = definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Store(store)
                if store.destination.base == MirPlaceBase::Storage(activation) =>
            {
                Some(store)
            }
            _ => None,
        })
        .expect("the activation must have an executable store");
    store.destination.base = MirPlaceBase::Storage(StorageId::new(callable, usize::MAX));
    let errors = crate::mir::check_normalized_mir(&invalid_place)
        .expect_err("an activation store cannot target an undeclared place")
        .to_string();
    assert!(
        errors.contains("place base") && errors.contains("is not declared"),
        "{errors}"
    );

    let mut invalid_lifetime = lower_source_to_final_mir(SOURCE);
    let function = invalid_lifetime.entry_function;
    let activation = invalid_lifetime
        .definitions
        .get(function)
        .unwrap()
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::PathCondition)
        .map(|storage| storage.id)
        .expect("the proof-rich source fixture must contain a path activation");
    let definition = invalid_lifetime
        .definitions
        .get_mut_for_test(function)
        .unwrap();
    let (block, live_index) = definition
        .body
        .blocks
        .iter_mut()
        .find_map(|block| {
            block
                .instructions
                .iter()
                .position(|instruction| {
                    matches!(
                        instruction,
                        MirInstruction::StorageLive(operation) if operation.storage == activation
                    )
                })
                .map(|index| (block, index))
        })
        .expect("the activation must have an explicit lifetime epoch");
    let duplicate_live = block.instructions[live_index].clone();
    block.instructions.insert(live_index + 1, duplicate_live);
    let errors = run_mir_pipeline_measured(invalid_lifetime, &none())
        .result
        .expect_err("an activation cannot become live twice in one lifetime epoch")
        .to_string();
    assert!(errors.contains("is already live"), "{errors}");
}

fn default_without<const N: usize>(disabled: [&str; N]) -> MirPassSchedule {
    resolve_mir_pass_schedule(MirOptimizationProfile::Default, disabled).unwrap()
}

fn none() -> MirPassSchedule {
    resolve_mir_pass_schedule(MirOptimizationProfile::None, std::iter::empty()).unwrap()
}

fn assembly(measured: &MeasuredMirPipeline) -> String {
    emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(measured.result.as_ref().unwrap()),
    )
    .unwrap()
}
