use super::*;

const OBJECT_RESULT_SOURCE: &str = concat!(
    "class Value {\n",
    "  field: i64;\n",
    "  init(field: i64) { self.field = field; }\n",
    "  destroy {}\n",
    "}\n",
    "fn produce(ref source: Value) -> Value {\n",
    "  var local: Value = source;\n",
    "  return local;\n",
    "}\n",
    "fn main() -> i64 {\n",
    "  var source: Value = Value(7);\n",
    "  var result: Value = produce(source);\n",
    "  return result.field;\n",
    "}\n",
);

#[test]
fn lowers_explicit_return_storage_call_destinations_and_cleanup_order() {
    let program = lower_text(OBJECT_RESULT_SOURCE);
    verify_mir(&program).unwrap();

    let produce = program.definitions.get(FunctionId::new(0)).unwrap();
    let return_storage = produce.return_storage.expect("expected return storage");
    assert_eq!(
        produce.storage[return_storage.index()].kind,
        MirStorageKind::Return
    );
    assert_eq!(produce.storage[return_storage.index()].source, None);
    let block = produce.block(produce.body.entry).unwrap();
    let copy_index = block
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::CopyConstruct(copy) if copy.destination == MirPlace::base(return_storage))
        })
        .expect("return storage must be copy-constructed");
    let cleanup_index = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Cleanup(_)))
        .expect("local must be cleaned");
    assert!(copy_index < cleanup_index);

    let main = program.definitions.get(program.entry_function).unwrap();
    let call = main.body.blocks[0]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call)
                if call.target == MirCallTarget::Direct(FunctionId::new(0)) =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("expected object-returning call");
    assert!(call.result.is_none());
    let destination = call
        .destination
        .as_ref()
        .expect("expected call destination");
    assert_eq!(
        main.storage(destination.base.storage()).unwrap().kind,
        MirStorageKind::Local
    );
    assert!(main.values.iter().all(|value| value.ty.is_scalar_value()));

    let dump = dump_mir(&program);
    assert!(dump.contains("ReturnStorage f0:s0"));
    assert!(dump.contains("return <return>"));
    assert!(dump.contains("<- call f0"));
    assert_eq!(dump, dump_mir(&program));
}

#[test]
fn rejects_missing_result_initialization_and_malformed_call_destinations() {
    let mut missing_result = lower_text(OBJECT_RESULT_SOURCE);
    let produce = missing_result
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let return_storage = produce.return_storage.unwrap();
    produce.body.blocks[0].instructions.retain(|instruction| {
        !matches!(instruction, MirInstruction::CopyConstruct(copy) if copy.destination == MirPlace::base(return_storage))
    });
    let errors = verify_mir(&missing_result).unwrap_err().to_string();
    assert!(errors.contains("object return storage is not initialized"));

    let mut missing_slot = lower_text(OBJECT_RESULT_SOURCE);
    missing_slot
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap()
        .return_storage = None;
    let errors = verify_mir(&missing_slot).unwrap_err().to_string();
    assert!(errors.contains("object-returning definition has no return storage"));

    let mut scalar_destination = lower_text(OBJECT_RESULT_SOURCE);
    let main = scalar_destination
        .definitions
        .get_mut_for_test(scalar_destination.entry_function)
        .unwrap();
    let call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if call.destination.is_some() => Some(call),
            _ => None,
        })
        .unwrap();
    call.destination = Some(
        call.destination
            .clone()
            .unwrap()
            .project_field(FieldId::new(ClassId::new(0), 0)),
    );
    let errors = verify_mir(&scalar_destination).unwrap_err().to_string();
    assert!(errors.contains("complete exact-class local or temporary destination storage"));
}
