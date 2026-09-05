//! Focused tests for the proof-rich and normalized MIR trust products.

use crate::{
    identity::FunctionId,
    mir::{check_normalized_mir, MirStorageKind},
    passes::reachability::{analyze_reachability, dump_reachability, extract_final_dependencies},
    test_support::lower_source_to_final_mir,
};

use super::{
    reachability_verification_errors,
    seal::{finalize_proof_mir, reseal_final_mir, verify_proof_mir, UnverifiedFinalMirProgram},
    verify_final_mir,
};

fn source() -> &'static str {
    "fn leaf() -> i64 { return 1; }
     fn dead() -> i64 { return 9; }
     fn main() -> i64 { return leaf(); }"
}

fn proof_source() -> &'static str {
    "fn choose(left: bool, right: bool) -> bool { return left && right; }
     fn main() -> i64 { if (choose(false, true)) { return 1; } return 0; }"
}

#[test]
fn proof_verification_preserves_provenance_without_publishing_final_facts() {
    let program = lower_source_to_final_mir(proof_source());
    let expected_program = program.clone();

    let verified = verify_proof_mir(program).unwrap();

    assert_eq!(verified.program(), &expected_program);
    assert!(verified
        .program()
        .executable_definitions()
        .any(|definition| !definition.path_conditions().is_empty()));
}

#[test]
fn normalization_preserves_the_complete_dependency_inventory() {
    let program = lower_source_to_final_mir(
        "fn target() -> bool { return true; }
         fn invoke(callback: fn() -> bool) -> bool { return callback(); }
         class State { static enabled: bool = true; init() {} }
         fn main() -> i64 {
           if (invoke(target) && State.enabled) { return 1; }
           return 0;
         }",
    );
    let proof_dependencies = extract_final_dependencies(&program).unwrap();

    let final_program = verify_final_mir(program).unwrap();
    let normalized_dependencies = extract_final_dependencies(final_program.program()).unwrap();

    assert_eq!(normalized_dependencies, proof_dependencies);
    assert_eq!(
        final_program.reachability(),
        &analyze_reachability(final_program.program()).unwrap()
    );
}

#[test]
fn proof_to_final_transition_consumes_provenance_and_rebinds_facts() {
    let program = lower_source_to_final_mir(proof_source());
    let proof = verify_proof_mir(program.clone()).unwrap();
    let (finalized, statistics) = finalize_proof_mir(proof).unwrap();
    let public = verify_final_mir(program).unwrap();
    let expected_reachability = analyze_reachability(finalized.program()).unwrap();

    assert_eq!(finalized, public);
    assert_eq!(finalized.reachability(), &expected_reachability);
    check_normalized_mir(finalized.program()).unwrap();
    assert!(statistics.path_condition_records() > 0);
    assert!(statistics.logical_expression_records() > 0);
    assert!(statistics.path_reads() > 0);
    assert!(statistics.activation_storage() > 0);
    assert!(finalized
        .program()
        .executable_definitions()
        .all(|definition| definition.path_conditions().is_empty()
            && definition.logical_expressions().is_empty()
            && definition
                .storage_entries()
                .iter()
                .all(|storage| storage.kind != MirStorageKind::PathCondition)));
    assert!(finalized
        .program()
        .executable_definitions()
        .flat_map(|definition| definition.storage_entries())
        .any(|storage| storage.kind.is_normalized_path_activation()));
}

#[test]
fn final_verification_binds_facts_derived_from_the_exact_normalized_program() {
    let program = lower_source_to_final_mir(source());
    let expected_program = program.clone();
    let expected_reachability = analyze_reachability(&program).unwrap();

    let verified = verify_final_mir(program).unwrap();

    assert_eq!(verified.program(), &expected_program);
    assert_eq!(verified.reachability(), &expected_reachability);
    assert_eq!(
        dump_reachability(verified.reachability()),
        dump_reachability(&expected_reachability)
    );
}

#[test]
fn cloning_each_verified_product_preserves_its_stage_contract() {
    let proof = verify_proof_mir(lower_source_to_final_mir(proof_source())).unwrap();
    let proof_clone = proof.clone();
    assert_eq!(proof_clone, proof);
    assert_eq!(proof_clone.program(), proof.program());

    let final_program = verify_final_mir(lower_source_to_final_mir(proof_source())).unwrap();
    let final_clone = final_program.clone();
    assert_eq!(final_clone, final_program);
    assert_eq!(final_clone.program(), final_program.program());
    assert_eq!(final_clone.reachability(), final_program.reachability());
}

#[test]
fn final_resealing_rechecks_normalized_activation_structure() {
    let finalized = verify_final_mir(lower_source_to_final_mir(
        "fn main() -> i64 { if (false && true) { return 1; } return 0; }",
    ))
    .unwrap();
    let (mut program, authority) = finalized.invalidate_for_final_transformation().into_parts();
    let definition = program.definitions.get(program.entry_function).unwrap();
    let activation = definition
        .storage
        .iter()
        .find(|storage| storage.kind.is_normalized_path_activation())
        .unwrap()
        .id;
    let entry = program.entry_function;
    let definition = program.definitions.get_mut_for_test(entry).unwrap();
    definition.storage[activation.index()].kind = MirStorageKind::PathCondition;

    let errors = reseal_final_mir(UnverifiedFinalMirProgram::from_parts(program, authority))
        .expect_err("fresh final seals must recheck activation phase legality")
        .to_string();
    assert!(
        errors.contains("retains executable carrier with proof provenance"),
        "{errors}"
    );
}

#[test]
fn public_debug_output_identifies_each_seal_without_exposing_authority() {
    let proof = verify_proof_mir(lower_source_to_final_mir(proof_source())).unwrap();
    let proof_debug = format!("{proof:?}");
    assert!(proof_debug.starts_with("VerifiedProofMirProgram { program:"));
    assert!(!proof_debug.contains("reachability"));

    let final_program = verify_final_mir(lower_source_to_final_mir(proof_source())).unwrap();
    let final_debug = format!("{final_program:?}");
    assert!(final_debug.starts_with("VerifiedFinalMirProgram { program:"));
    assert!(!final_debug.contains("reachability"));
    assert!(!final_debug.contains("consumed_proof"));
}

#[test]
fn reachability_failures_are_attributed_as_program_verification_errors() {
    let errors = reachability_verification_errors(
        crate::passes::reachability::MirDependencyExtractionError::UnknownFunction(
            FunctionId::new(19),
        ),
    );
    let error = errors.iter().next().unwrap();

    assert_eq!(error.callable, None);
    assert_eq!(error.block, None);
    assert_eq!(
        error.message,
        "reachability analysis failed: unknown function f19"
    );
}
