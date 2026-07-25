use super::*;

#[test]
fn carries_named_and_produced_owners_through_parameters_and_results() {
    let program = lower_text(concat!(
        "class Item { init() {} }\n",
        "fn make() -> shared Item { return new Item(); }\n",
        "fn forward(value: shared Item) -> shared Item { return value; }\n",
        "fn replace(value: shared Item, replacement: shared Item) -> shared Item {\n",
        "  value = replacement;\n",
        "  return value;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var first: shared Item = make();\n",
        "  var copied: shared Item = forward(first);\n",
        "  var produced: shared Item = forward(make());\n",
        "  var replaced: shared Item = replace(copied, produced);\n",
        "  return 0;\n",
        "}\n",
    ));
    verify_mir(&program).expect("shared call ownership must verify");

    let dump = dump_mir(&program);
    assert!(dump.contains("shared-owner("));
    assert!(dump.contains("return-shared"));
    assert!(dump.contains("shared-result"));
}

#[test]
fn rejects_corrupt_call_handoffs_parameter_cleanup_and_shared_returns() {
    let source = concat!(
        "class Item { init() {} }\n",
        "fn make() -> shared Item { return new Item(); }\n",
        "fn forward(value: shared Item) -> shared Item { return value; }\n",
        "fn main() -> i64 {\n",
        "  var first: shared Item = make();\n",
        "  var second: shared Item = forward(first);\n",
        "  return 0;\n",
        "}\n",
    );
    let program = lower_text(source);

    let mut missing_parameter_cleanup = program.clone();
    missing_parameter_cleanup
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap()
        .body
        .blocks[0]
        .instructions
        .retain(|instruction| !matches!(instruction, MirInstruction::SharedRelease(_)));
    assert!(has_error(
        &missing_parameter_cleanup,
        "shared owner remains live on normal return"
    ));

    let mut reused_result = program.clone();
    let main = reused_result
        .definitions
        .get_mut_for_test(FunctionId::new(2))
        .unwrap();
    let first = main
        .storage
        .iter()
        .find(|storage| storage.name == "first")
        .unwrap()
        .id;
    let call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call)
                if call.target == MirCallTarget::Direct(FunctionId::new(1)) =>
            {
                Some(call)
            }
            _ => None,
        })
        .unwrap();
    call.shared_result = Some(first);
    assert!(has_error(
        &reused_result,
        "shared call result storage is already initialized"
    ));

    let mut wrong_return = program;
    let forward = wrong_return
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let parameter = forward.parameters[0];
    forward.body.blocks[0].terminator = Some(MirTerminator::ReturnShared {
        owner: parameter,
        span: forward.span,
    });
    assert!(has_error(
        &wrong_return,
        "shared return must transfer the definition's matching return owner"
    ));
}
