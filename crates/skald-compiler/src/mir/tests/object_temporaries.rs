use super::*;

const TEMPORARY_SOURCE: &str = concat!(
    "class Value {\n",
    "  field: i64;\n",
    "  init(field: i64) { self.field = field; }\n",
    "  destroy {}\n",
    "}\n",
    "fn produce(field: i64) -> Value { return Value(field); }\n",
    "fn grouped(field: i64) -> Value { return (Value(field)); }\n",
    "fn consume(value: Value) -> unit {}\n",
    "fn main() -> i64 {\n",
    "  var direct: Value = Value(1);\n",
    "  var copied: Value = (Value(2));\n",
    "  direct = Value(3);\n",
    "  consume(Value(4));\n",
    "  direct = produce(5);\n",
    "  var result: Value = produce(6);\n",
    "  return result.field;\n",
    "}\n",
);

#[test]
fn lowers_elided_destinations_and_bounded_materialized_sources() {
    let program = lower_text(TEMPORARY_SOURCE);
    verify_mir(&program).unwrap();

    let produce = program.definitions.get(FunctionId::new(0)).unwrap();
    let return_storage = produce.return_storage.unwrap();
    assert!(produce.body.blocks[0]
        .instructions
        .iter()
        .any(|instruction| {
            matches!(instruction, MirInstruction::Initialize(initialize)
            if initialize.destination == MirPlace::base(return_storage))
        }));
    assert!(produce
        .storage
        .iter()
        .all(|storage| storage.kind != MirStorageKind::Temporary));

    let grouped = program.definitions.get(FunctionId::new(1)).unwrap();
    assert_eq!(
        grouped
            .storage
            .iter()
            .filter(|storage| storage.kind == MirStorageKind::Temporary)
            .count(),
        1
    );
    let grouped_instructions = &grouped.body.blocks[0].instructions;
    let initialize = grouped_instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Initialize(_)))
        .unwrap();
    let copy = grouped_instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::CopyConstruct(_)))
        .unwrap();
    let boundary = grouped_instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndFullExpression(_)))
        .unwrap();
    assert!(initialize < copy && copy < boundary);

    let main = program.definitions.get(program.entry_function).unwrap();
    let temporary_count = main
        .storage
        .iter()
        .filter(|storage| storage.kind == MirStorageKind::Temporary)
        .count();
    let boundaries: Vec<_> = main.body.blocks[0]
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::EndFullExpression(end) => Some(end),
            _ => None,
        })
        .collect();
    assert_eq!(temporary_count, 4);
    assert_eq!(boundaries.len(), 4);
    assert!(boundaries
        .iter()
        .all(|boundary| boundary.temporaries.len() == 1));
    assert!(main.values.iter().all(|value| value.ty.is_scalar_value()));
}

#[test]
fn materialized_call_arguments_remain_live_through_the_outer_call() {
    let program = lower_text(concat!(
        "class Value { init() {} destroy {} }\n",
        "fn produce() -> Value { return Value(); }\n",
        "fn consume(first: Value, second: Value) -> unit {}\n",
        "fn main() -> i64 { consume(Value(), produce()); return 0; }\n",
    ));
    verify_mir(&program).unwrap();

    let main = program.definitions.get(program.entry_function).unwrap();
    let instructions = &main.body.blocks[0].instructions;
    let first_producer = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Initialize(_)))
        .unwrap();
    let producer_call = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Call(call)
                if call.target == MirCallTarget::Direct(FunctionId::new(0)))
        })
        .unwrap();
    let consumer_call = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Call(call)
                if call.target == MirCallTarget::Direct(FunctionId::new(1)))
        })
        .unwrap();
    let boundary_index = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndFullExpression(_)))
        .unwrap();
    assert!(first_producer < producer_call && producer_call < consumer_call);
    assert!(consumer_call < boundary_index);

    let temporary_places: Vec<_> = main
        .storage
        .iter()
        .filter(|storage| storage.kind == MirStorageKind::Temporary)
        .map(|storage| MirPlace::base(storage.id))
        .collect();
    let MirInstruction::EndFullExpression(boundary) = &instructions[boundary_index] else {
        unreachable!();
    };
    assert_eq!(
        boundary
            .temporaries
            .iter()
            .map(|cleanup| cleanup.destination.clone())
            .collect::<Vec<_>>(),
        temporary_places.into_iter().rev().collect::<Vec<_>>()
    );
}
