use super::*;

#[test]
fn lowers_each_primitive_assignment_to_the_existing_local_storage() {
    let program = lower_text(concat!(
        "fn signed_source() -> i64 { return 7; }\n",
        "fn main() -> i64 {\n",
        "  var signed: i64 = 0;\n",
        "  var unsigned: u64 = 0u;\n",
        "  var byte: u8 = 0u8;\n",
        "  var float: f64 = 0.0;\n",
        "  var flag: bool = false;\n",
        "  signed = signed_source();\n",
        "  (unsigned) = 2u;\n",
        "  byte = 3u8;\n",
        "  float = 4.0;\n",
        "  flag = true;\n",
        "  signed = signed + 1;\n",
        "  return signed;\n",
        "}\n",
    ));
    verify_mir(&program).unwrap();
    let main = program.definitions.get(program.entry_function).unwrap();
    let stores: Vec<_> = main.body.blocks[0]
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::Store(store) => Some(store),
            _ => None,
        })
        .collect();

    // Five declaration stores followed by the six reassignment stores.
    assert_eq!(stores.len(), 11);
    assert_eq!(
        stores[5..]
            .iter()
            .map(|store| {
                let MirPlaceBase::Storage(storage) = store.destination.base else {
                    panic!("primitive local assignment must target storage");
                };
                storage.index()
            })
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 0]
    );
    assert_eq!(
        main.storage
            .iter()
            .map(|storage| storage.ty)
            .collect::<Vec<_>>(),
        [
            MirType::I64,
            MirType::U64,
            MirType::U8,
            MirType::F64,
            MirType::Bool
        ]
    );
}

#[test]
fn evaluates_the_source_before_storing_and_cleans_temporaries_afterward() {
    let program = lower_text(concat!(
        "class Value {\n",
        "  field: i64;\n",
        "  init(field: i64) { self.field = field; }\n",
        "  destroy {}\n",
        "}\n",
        "fn read(value: Value) -> i64 { return value.field; }\n",
        "fn main() -> i64 {\n",
        "  var result: i64 = 0;\n",
        "  result = read(Value(42));\n",
        "  return result;\n",
        "}\n",
    ));
    verify_mir(&program).unwrap();
    let main = program.definitions.get(program.entry_function).unwrap();
    let instructions = &main.body.blocks[0].instructions;
    let call = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Call(_)))
        .unwrap();
    let store = instructions
        .iter()
        .enumerate()
        .skip(call + 1)
        .find_map(|(index, instruction)| {
            matches!(instruction, MirInstruction::Store(_)).then_some(index)
        })
        .unwrap();
    let boundary = instructions
        .iter()
        .enumerate()
        .skip(store + 1)
        .find_map(|(index, instruction)| {
            matches!(instruction, MirInstruction::EndFullExpression(_)).then_some(index)
        })
        .unwrap();

    assert!(call < store && store < boundary);
    let MirInstruction::EndFullExpression(end) = &instructions[boundary] else {
        unreachable!();
    };
    assert_eq!(end.temporaries.len(), 1);
}
