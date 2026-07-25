use super::*;

const OPTIONAL_SOURCE: &str = "fn main() -> i64 {\n\
    var value: i64? = none;\n\
    if (value is none) { value = 40; } else { value = 41; }\n\
    var copied: i64? = value;\n\
    return copied! + 2;\n\
}\n";

#[test]
fn lowers_primitive_optional_state_and_checked_access_explicitly() {
    let program = lower_text(OPTIONAL_SOURCE);
    verify_mir(&program).expect("lowered primitive optionals must verify");
    let dump = dump_mir(&program);
    assert_eq!(dump, dump_mir(&lower_text(OPTIONAL_SOURCE)));

    assert!(dump.contains("local f0:l0 \"value\" : i64?"));
    assert!(dump.contains("optional-initialize"));
    assert!(dump.contains("optional-assign"));
    assert!(dump.contains("optional-presence none"));
    assert!(dump.contains("optional-unwrap"));
    assert!(dump.contains("terminate optional-access-failure"));
}

#[test]
fn optional_assignment_preserves_initialized_wrapper_state_across_cfg_joins() {
    let program = lower_text(
        "fn main() -> i64 {\n\
           var value: i64? = 1;\n\
           if (value is some) { value = none; } else { value = 2; }\n\
           if (value is none) { return 7; }\n\
           return value!;\n\
         }\n",
    );

    verify_mir(&program).expect("dynamic presence may differ across an initialized-wrapper join");
}

#[test]
fn verifier_rejects_uninitialized_use_and_mismatched_failure_edges() {
    let mut uninitialized = lower_text(OPTIONAL_SOURCE);
    let function = uninitialized
        .definitions
        .get_mut_for_test(uninitialized.entry_function)
        .unwrap();
    let initialize = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::OptionalInitialize(initialize) => Some(initialize.clone()),
            _ => None,
        })
        .unwrap();
    function.body.blocks[0]
        .instructions
        .retain(|instruction| !matches!(instruction, MirInstruction::OptionalInitialize(_)));
    function.body.blocks[0].instructions.insert(
        1,
        MirInstruction::OptionalAssign(MirOptionalAssign {
            destination: initialize.destination,
            source: initialize.source,
            span: initialize.span,
        }),
    );
    let errors =
        verify_mir(&uninitialized).expect_err("assignment before initialization must fail");
    assert!(errors
        .iter()
        .any(|error| error.message.contains("not definitely initialized")));

    let mut failure = lower_text(OPTIONAL_SOURCE);
    let function = failure
        .definitions
        .get_mut_for_test(failure.entry_function)
        .unwrap();
    let failure_target = function
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::OptionalUnwrap { failure_target, .. }) => Some(failure_target),
            _ => None,
        })
        .unwrap();
    function.body.blocks[failure_target.index()].terminator = Some(MirTerminator::Terminate {
        reason: MirTerminationReason::ObjectCastFailure,
        span: function.span,
    });
    let errors = verify_mir(&failure).expect_err("wrong unwrap failure reason must fail");
    assert!(errors
        .iter()
        .any(|error| error.message.contains("optional unwrap failure edge")));
}
