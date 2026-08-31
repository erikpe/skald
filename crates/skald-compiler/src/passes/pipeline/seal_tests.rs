//! Focused tests for the verified program-plus-reachability product.

use crate::{
    identity::FunctionId,
    passes::reachability::{analyze_reachability, dump_reachability},
    test_support::lower_source_to_final_mir,
};

use super::{reachability_verification_errors, verify_final_mir};

fn source() -> &'static str {
    "fn leaf() -> i64 { return 1; }
     fn dead() -> i64 { return 9; }
     fn main() -> i64 { return leaf(); }"
}

#[test]
fn final_verification_binds_facts_derived_from_the_exact_program() {
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
fn cloning_a_verified_product_keeps_its_program_and_facts_coherent() {
    let verified = verify_final_mir(lower_source_to_final_mir(source())).unwrap();
    let cloned = verified.clone();

    assert_eq!(cloned, verified);
    assert_eq!(cloned.program(), verified.program());
    assert_eq!(cloned.reachability(), verified.reachability());
}

#[test]
fn public_debug_output_preserves_the_program_only_shape() {
    let verified = verify_final_mir(lower_source_to_final_mir(source())).unwrap();
    let debug = format!("{verified:?}");

    assert!(debug.starts_with("VerifiedFinalMirProgram { program:"));
    assert!(!debug.contains("reachability"));
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
