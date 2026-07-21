use super::*;

const VALUE_PARAMETER_SOURCE: &str = concat!(
    "class Value {\n",
    "  field: i64;\n",
    "  init(field: i64) { self.field = field; }\n",
    "  destroy {}\n",
    "}\n",
    "fn consume(first: i64, value: Value, second: i64, ref alias: Value) -> i64 {\n",
    "  var local: Value = value;\n",
    "  value = alias;\n",
    "  return first + second + local.field + value.field;\n",
    "}\n",
    "fn main() -> i64 {\n",
    "  var source: Value = Value(20);\n",
    "  return consume(1, source, 2, source);\n",
    "}\n",
);

#[test]
fn lowers_owned_arguments_transfer_and_reverse_parameter_cleanup_explicitly() {
    let program = lower_text(VALUE_PARAMETER_SOURCE);
    verify_mir(&program).unwrap();

    let consume = program.definitions.get(FunctionId::new(0)).unwrap();
    assert_eq!(consume.storage[1].kind, MirStorageKind::Parameter);
    assert_eq!(consume.storage[1].ty, MirType::Class(ClassId::new(0)));
    let consume_block = consume.block(consume.body.entry).unwrap();
    let cleanup_places: Vec<_> = consume_block
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::Cleanup(cleanup) => Some(cleanup.destination.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        cleanup_places,
        [
            MirPlace::base(consume.storage[4].id),
            MirPlace::base(consume.storage[1].id)
        ]
    );

    let main = program.definitions.get(program.entry_function).unwrap();
    let block = main.block(main.body.entry).unwrap();
    let copy_index = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::CopyConstruct(_)))
        .unwrap();
    let call_index = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Call(_)))
        .unwrap();
    assert!(copy_index < call_index);
    let MirInstruction::Call(call) = &block.instructions[call_index] else {
        unreachable!()
    };
    let MirArgument::OwnedPlace(argument) = &call.arguments[1] else {
        panic!("class value argument must transfer an owned place");
    };
    let argument_storage = main.storage(argument.base.storage()).unwrap();
    assert_eq!(argument_storage.kind, MirStorageKind::Argument);
    assert!(argument_storage.source.is_none());
    assert!(main.values.iter().all(|value| value.ty.is_scalar_value()));
    assert!(dump_mir(&program).contains("argument <argument>"));
}

#[test]
fn rejects_missing_parameter_cleanup_and_non_owned_class_arguments() {
    let mut missing_cleanup = lower_text(VALUE_PARAMETER_SOURCE);
    let consume = missing_cleanup
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    consume.body.blocks[0].instructions.retain(|instruction| {
        !matches!(instruction, MirInstruction::Cleanup(cleanup) if cleanup.destination == MirPlace::base(consume.storage[1].id))
    });
    let errors = verify_mir(&missing_cleanup).unwrap_err().to_string();
    assert!(errors.contains("owning value parameter remains live"));

    let mut non_owned = lower_text(VALUE_PARAMETER_SOURCE);
    let main = non_owned
        .definitions
        .get_mut_for_test(non_owned.entry_function)
        .unwrap();
    let call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    let MirArgument::OwnedPlace(place) = &call.arguments[1] else {
        unreachable!()
    };
    call.arguments[1] = MirArgument::Place(place.clone());
    let errors = verify_mir(&non_owned).unwrap_err().to_string();
    assert!(errors.contains("must be a scalar value or owned place"));

    let mut scalar_argument_storage = lower_text(VALUE_PARAMETER_SOURCE);
    let main = scalar_argument_storage
        .definitions
        .get_mut_for_test(scalar_argument_storage.entry_function)
        .unwrap();
    main.storage
        .iter_mut()
        .find(|storage| storage.kind == MirStorageKind::Argument)
        .unwrap()
        .ty = MirType::I64;
    let errors = verify_mir(&scalar_argument_storage)
        .unwrap_err()
        .to_string();
    assert!(errors.contains("compiler-owned storage"));
}
