use std::collections::BTreeSet;

use crate::{
    identity::{CallableId, FunctionId},
    mir::{
        check_normalized_mir, MirInstruction, MirPlace, MirRvalueKind, MirStorageKind,
        PathConditionId,
    },
    passes::verify_proof_mir,
    test_support::lower_source_to_final_mir,
};

use super::{
    normalize_program, normalize_proof_provenance, MirProofNormalizationErrorKind,
    MirProofNormalizationStatistics,
};

fn verified(source: &str) -> super::VerifiedProofMirProgram {
    verify_proof_mir(lower_source_to_final_mir(source))
        .expect("fixture must produce proof-rich MIR")
}

fn assert_no_consumed_proof(program: &crate::mir::MirProgram) {
    check_normalized_mir(program).expect("normalization must establish its executable invariant");
    for definition in program.executable_definitions() {
        assert!(definition.path_conditions().is_empty());
        assert!(definition.logical_expressions().is_empty());
        assert!(definition
            .storage_entries()
            .iter()
            .all(|storage| storage.kind != MirStorageKind::PathCondition));
        assert!(definition
            .body()
            .blocks
            .iter()
            .all(
                |block| block.instructions.iter().all(|instruction| !matches!(
                    instruction,
                    MirInstruction::Assign(assignment)
                        if matches!(assignment.rvalue.kind, MirRvalueKind::PathCondition(_))
                ))
            ));
    }
}

fn source_with_two_logical_functions() -> &'static str {
    "fn first() -> bool { return true && false; }
     fn main() -> i64 { if (false || true) { return 1; } return 0; }"
}

#[test]
fn empty_proof_inventory_preserves_the_program_exactly() {
    let verified = verified("fn main() -> i64 { return 0; }");
    let expected = verified.program().clone();
    let normalized = normalize_proof_provenance(verified).unwrap();

    assert_eq!(normalized.program(), &expected);
    assert_eq!(
        normalized.statistics(),
        MirProofNormalizationStatistics::default()
    );
    assert_no_consumed_proof(normalized.program());
}

#[test]
fn path_reads_and_activation_storage_are_reclassified_without_other_edits() {
    let verified = verified(
        "fn choose() -> bool { return true && false; }
         fn main() -> i64 { if (choose()) { return 1; } return 0; }",
    );
    let original = verified.program().clone();
    let normalized_function = FunctionId::new(0);
    let definition = original
        .definitions
        .get(normalized_function)
        .expect("logical helper definition must exist");
    let (block_index, instruction_index, assignment, read) =
        definition
            .body
            .blocks
            .iter()
            .enumerate()
            .find_map(|(block_index, block)| {
                block.instructions.iter().enumerate().find_map(
                    |(instruction_index, instruction)| {
                        let MirInstruction::Assign(assignment) = instruction else {
                            return None;
                        };
                        let MirRvalueKind::PathCondition(read) = assignment.rvalue.kind else {
                            return None;
                        };
                        Some((block_index, instruction_index, assignment.clone(), read))
                    },
                )
            })
            .expect("logical lowering must contain a path read");
    let original_storage = definition.storage(read.activation).unwrap().clone();
    let original_blocks = definition
        .body
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<Vec<_>>();
    let original_values = definition
        .values
        .iter()
        .map(|value| value.id)
        .collect::<Vec<_>>();

    let normalized = normalize_proof_provenance(verified).unwrap();
    let output = normalized
        .program()
        .definitions
        .get(normalized_function)
        .unwrap();
    let MirInstruction::Assign(replacement) =
        &output.body.blocks[block_index].instructions[instruction_index]
    else {
        panic!("normalization must preserve the assignment")
    };
    assert_eq!(replacement.result, assignment.result);
    assert_eq!(replacement.rvalue.ty, assignment.rvalue.ty);
    assert_eq!(replacement.span, assignment.span);
    assert_eq!(
        replacement.rvalue.kind,
        MirRvalueKind::Load(MirPlace::base(read.activation))
    );

    let mut expected_storage = original_storage;
    expected_storage.kind = MirStorageKind::ScalarSpill;
    assert_eq!(output.storage(read.activation), Some(&expected_storage));
    assert_eq!(
        output
            .body
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        original_blocks
    );
    assert_eq!(
        output
            .values
            .iter()
            .map(|value| value.id)
            .collect::<Vec<_>>(),
        original_values
    );
    assert_no_consumed_proof(normalized.program());
}

#[test]
fn nested_parented_and_mixed_logical_provenance_is_consumed_together() {
    let verified = verified(
        "fn main() -> i64 {
           var selected: bool = true && (false || true);
           if (selected) { return 1; }
           return 0;
         }",
    );
    let expected_conditions = verified
        .program()
        .executable_definitions()
        .map(|definition| definition.path_conditions().len())
        .sum::<usize>();
    let expected_logical = verified
        .program()
        .executable_definitions()
        .map(|definition| definition.logical_expressions().len())
        .sum::<usize>();
    let expected_reads = verified
        .program()
        .executable_definitions()
        .map(|definition| {
            definition
                .body()
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter(|instruction| {
                    matches!(
                        instruction,
                        MirInstruction::Assign(assignment)
                            if matches!(assignment.rvalue.kind, MirRvalueKind::PathCondition(_))
                    )
                })
                .count()
        })
        .sum::<usize>();
    assert!(verified
        .program()
        .executable_definitions()
        .flat_map(|definition| definition.path_conditions())
        .any(|condition| condition.parent.is_some()));

    let normalized = normalize_proof_provenance(verified).unwrap();
    let statistics = normalized.statistics();
    assert_eq!(statistics.path_condition_records(), expected_conditions);
    assert_eq!(statistics.logical_expression_records(), expected_logical);
    assert_eq!(statistics.path_reads(), expected_reads);
    assert_eq!(statistics.activation_storage(), expected_conditions);
    assert_eq!(statistics.changed_callables(), 1);
    assert!(statistics.released_proof_blocks() > 0);
    assert_no_consumed_proof(normalized.program());
}

#[test]
fn logical_and_cleanup_generated_path_reads_share_the_exact_rewrite() {
    let verified = verified(
        "class Item {
           truth: bool;
           init(truth: bool) { self.truth = truth; }
         }
         fn choose(flag: bool) -> bool { return flag && Item(true).truth; }
         fn main() -> i64 { if (choose(true)) { return 1; } return 0; }",
    );
    let logical_selection_blocks = verified
        .program()
        .executable_definitions()
        .flat_map(|definition| definition.logical_expressions())
        .map(|logical| logical.selection)
        .collect::<BTreeSet<_>>();
    let path_sites = verified
        .program()
        .executable_definitions()
        .flat_map(|definition| {
            definition.body().blocks.iter().flat_map(|block| {
                block
                    .instructions
                    .iter()
                    .filter_map(move |instruction| match instruction {
                        MirInstruction::Assign(assignment)
                            if matches!(
                                assignment.rvalue.kind,
                                MirRvalueKind::PathCondition(_)
                            ) =>
                        {
                            Some((block.id, assignment.result))
                        }
                        _ => None,
                    })
            })
        })
        .collect::<BTreeSet<_>>();
    assert!(path_sites
        .iter()
        .any(|(block, _)| logical_selection_blocks.contains(block)));
    assert!(path_sites
        .iter()
        .any(|(block, _)| !logical_selection_blocks.contains(block)));

    let normalized = normalize_proof_provenance(verified).unwrap();
    assert_eq!(normalized.statistics().path_reads(), path_sites.len());
    assert_no_consumed_proof(normalized.program());
}

#[test]
fn every_executable_definition_kind_uses_the_same_transaction() {
    let verified = verified(
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
    let changed_kinds = verified
        .program()
        .executable_definitions()
        .filter(|definition| !definition.path_conditions().is_empty())
        .map(|definition| definition.callable())
        .collect::<BTreeSet<_>>();
    assert!(changed_kinds
        .iter()
        .any(|callable| matches!(callable, CallableId::Function(_))));
    assert!(changed_kinds
        .iter()
        .any(|callable| matches!(callable, CallableId::Initializer(_))));
    assert!(changed_kinds
        .iter()
        .any(|callable| matches!(callable, CallableId::Method(_))));
    assert!(changed_kinds
        .iter()
        .any(|callable| matches!(callable, CallableId::Destructor(_))));
    assert!(changed_kinds
        .iter()
        .any(|callable| matches!(callable, CallableId::StaticInitializer(_))));

    let normalized = normalize_proof_provenance(verified).unwrap();
    assert_eq!(
        normalized.statistics().changed_callables(),
        changed_kinds.len()
    );
    assert_no_consumed_proof(normalized.program());
}

#[test]
fn malformed_foreign_activation_ownership_is_rejected_before_rewrite() {
    let mut program = verified(source_with_two_logical_functions())
        .program()
        .clone();
    let foreign_activation = program
        .definitions
        .get(FunctionId::new(1))
        .unwrap()
        .body
        .path_conditions[0]
        .activation;
    program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap()
        .body
        .path_conditions[0]
        .activation = foreign_activation;

    let error = normalize_program(program).unwrap_err();
    assert!(matches!(
        error.kind.as_ref(),
        MirProofNormalizationErrorKind::ForeignActivationStorage { .. }
    ));
}

#[test]
fn unknown_path_read_is_rejected_without_exposing_partial_output() {
    let mut program = verified(source_with_two_logical_functions())
        .program()
        .clone();
    let entry = program.entry_function;
    let definition = program.definitions.get_mut_for_test(entry).unwrap();
    let read = definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) => match &mut assignment.rvalue.kind {
                MirRvalueKind::PathCondition(read) => Some(read),
                _ => None,
            },
            _ => None,
        })
        .expect("entry fixture must contain a path read");
    read.condition = PathConditionId::new(entry, 999);
    let unchanged_original = program.clone();

    let error = normalize_program(program.clone()).unwrap_err();
    assert!(matches!(
        error.kind.as_ref(),
        MirProofNormalizationErrorKind::UnknownPathReadCondition { .. }
    ));
    assert_eq!(program, unchanged_original);
    assert!(!program
        .definitions
        .get(FunctionId::new(0))
        .unwrap()
        .body
        .path_conditions
        .is_empty());
}

#[test]
fn repeated_normalization_from_equal_seals_is_deterministic() {
    let first = verified(source_with_two_logical_functions());
    let second = first.clone();

    let first = normalize_proof_provenance(first).unwrap();
    let second = normalize_proof_provenance(second).unwrap();

    assert_eq!(first, second);
    assert_no_consumed_proof(first.program());
}
