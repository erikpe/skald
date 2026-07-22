use crate::{
    mir::{verify_mir, MirArgument, MirCall, MirInstruction, MirPlace},
    test_support::lower_source_to_mir,
};

fn call_mut(program: &mut crate::mir::MirProgram) -> &mut MirCall {
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap()
}

fn messages(program: &crate::mir::MirProgram) -> Vec<String> {
    verify_mir(program)
        .unwrap_err()
        .iter()
        .map(|error| error.message.clone())
        .collect()
}

#[test]
fn value_parameters_reject_place_arguments_with_the_exact_contract_message() {
    let mut program = lower_source_to_mir(concat!(
        "fn one(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { var local: i64 = 1; return one(local); }\n",
    ));
    let storage = program
        .definitions
        .get(program.entry_function)
        .unwrap()
        .storage[0]
        .id;
    call_mut(&mut program).arguments[0] = MirArgument::Place(MirPlace::base(storage));

    assert!(messages(&program)
        .iter()
        .any(|message| message == "call argument 0 must be a scalar value or owned place"));
}

#[test]
fn owned_arguments_reject_duplicate_transfers_with_the_exact_contract_message() {
    let mut program = lower_source_to_mir(concat!(
        "class Value { init() {} }\n",
        "fn consume(first: Value, second: Value) -> i64 { return 0; }\n",
        "fn main() -> i64 { var first: Value = Value(); var second: Value = Value(); ",
        "return consume(first, second); }\n",
    ));
    let call = call_mut(&mut program);
    let MirArgument::OwnedPlace(first) = &call.arguments[0] else {
        panic!("expected owned first argument");
    };
    call.arguments[1] = MirArgument::OwnedPlace(first.clone());

    assert!(messages(&program)
        .iter()
        .any(|message| message == "call argument 1 transfers storage more than once"));
}
