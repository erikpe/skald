use super::*;

fn primitive_alias_mir() -> MirProgram {
    lower_text(concat!(
        "fn observe(ref value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { return observe(40 + 2); }\n",
    ))
}

fn verification_messages(program: &MirProgram) -> Vec<String> {
    verify_mir(program)
        .expect_err("mutated produced primitive alias MIR must fail verification")
        .iter()
        .map(|error| error.message.clone())
        .collect()
}

fn alias_storage(program: &MirProgram) -> StorageId {
    program
        .definitions
        .get(program.entry_function)
        .unwrap()
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::PrimitiveAlias)
        .expect("fixture must contain produced primitive alias storage")
        .id
}

fn entry_instructions_mut(program: &mut MirProgram) -> &mut Vec<MirInstruction> {
    &mut program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .instructions
}

#[test]
fn lowers_one_initialized_bounded_readonly_alias_storage() {
    let program = primitive_alias_mir();
    verify_mir(&program).expect("lowered produced primitive alias MIR must verify");
    let storage = alias_storage(&program);
    let function = program.definitions.get(program.entry_function).unwrap();
    let metadata = function.storage(storage).unwrap();
    assert_eq!(metadata.ty, MirType::I64);
    assert_eq!(metadata.source, None);

    let instructions = &function.body.blocks[0].instructions;
    let live = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::StorageLive(event) if event.storage == storage)
        })
        .unwrap();
    let store = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Store(store) if store.destination == MirPlace::base(storage))
        })
        .unwrap();
    let call = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Call(call)
                if call.arguments == [MirArgument::Place(MirPlace::base(storage))])
        })
        .unwrap();
    let dead = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::StorageDead(event) if event.storage == storage)
        })
        .unwrap();
    assert!(live < store && store < call && call < dead);

    let dump = dump_mir(&program);
    assert!(dump.contains("primitive-alias <primitive-alias>"), "{dump}");
    assert_eq!(dump, dump_mir(&program));
}

#[test]
fn rejects_missing_duplicate_and_reordered_alias_initialization() {
    let mut missing = primitive_alias_mir();
    let storage = alias_storage(&missing);
    entry_instructions_mut(&mut missing).retain(|instruction| {
        !matches!(instruction, MirInstruction::Store(store) if store.destination == MirPlace::base(storage))
    });
    assert!(verification_messages(&missing)
        .iter()
        .any(|message| message.contains("must be initialized exactly once, found 0 stores")));

    let mut duplicate = primitive_alias_mir();
    let storage = alias_storage(&duplicate);
    let instructions = entry_instructions_mut(&mut duplicate);
    let store = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Store(store) if store.destination == MirPlace::base(storage))
        })
        .unwrap();
    let duplicate_store = instructions[store].clone();
    instructions.insert(store + 1, duplicate_store);
    assert!(verification_messages(&duplicate)
        .iter()
        .any(|message| message.contains("must be initialized exactly once, found 2 stores")));

    let mut reordered = primitive_alias_mir();
    let storage = alias_storage(&reordered);
    let instructions = entry_instructions_mut(&mut reordered);
    let store = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Store(store) if store.destination == MirPlace::base(storage))
        })
        .unwrap();
    let call = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Call(_)))
        .unwrap();
    instructions.swap(store, call);
    assert!(verification_messages(&reordered)
        .iter()
        .any(|message| message.contains("must be initialized before alias use")));
}

#[test]
fn rejects_mutation_escape_mutable_borrow_wrong_type_and_early_end() {
    let mut mutation = primitive_alias_mir();
    let storage = alias_storage(&mutation);
    let instructions = entry_instructions_mut(&mut mutation);
    let store = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Store(store) if store.destination == MirPlace::base(storage))
        })
        .unwrap();
    let second_store = instructions[store].clone();
    instructions.insert(store + 1, second_store);
    assert!(verification_messages(&mutation)
        .iter()
        .any(|message| message.contains("initialized exactly once")));

    let mut escape = primitive_alias_mir();
    let storage = alias_storage(&escape);
    let function = escape
        .definitions
        .get_mut_for_test(escape.entry_function)
        .unwrap();
    let value = ValueId::new(escape.entry_function, function.values.len());
    function.values.push(MirValue {
        id: value,
        ty: MirType::I64,
        span: function.span,
    });
    let dead = function.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::StorageDead(event) if event.storage == storage)
        })
        .unwrap();
    function.body.blocks[0].instructions.insert(
        dead,
        MirInstruction::Assign(MirAssignment {
            result: value,
            rvalue: MirRvalue {
                kind: MirRvalueKind::Load(MirPlace::base(storage)),
                ty: MirType::I64,
            },
            span: function.span,
        }),
    );
    assert!(verification_messages(&escape)
        .iter()
        .any(|message| message.contains("may only be initialized and passed once")));

    let mut mutable = primitive_alias_mir();
    let observe = FunctionId::new(0);
    mutable.declarations.entries_mut_for_test()[observe.index()].parameters[0].mode =
        MirParameterMode::MutableAlias;
    let definition = mutable.definitions.get_mut_for_test(observe).unwrap();
    definition.storage[0].kind = MirStorageKind::AliasParameter(MirAliasAccess::Mutable);
    assert!(verification_messages(&mutable)
        .iter()
        .any(|message| message.contains("cannot mutably borrow produced primitive alias storage")));

    let mut wrong_type = primitive_alias_mir();
    let storage = alias_storage(&wrong_type);
    wrong_type
        .definitions
        .get_mut_for_test(wrong_type.entry_function)
        .unwrap()
        .storage[storage.index()]
    .ty = MirType::Function(crate::identity::FunctionTypeId::new(0));
    assert!(verification_messages(&wrong_type)
        .iter()
        .any(|message| message.contains("has non-primitive type")));

    let mut early_end = primitive_alias_mir();
    let storage = alias_storage(&early_end);
    let instructions = entry_instructions_mut(&mut early_end);
    let call = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Call(_)))
        .unwrap();
    let dead = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::StorageDead(event) if event.storage == storage)
        })
        .unwrap();
    instructions.swap(call, dead);
    let messages = verification_messages(&early_end);
    assert!(messages
        .iter()
        .any(|message| message.contains("remain live until after the call")));
    assert!(messages
        .iter()
        .any(|message| message.contains("used outside a live lifetime epoch")));
}

#[test]
fn keeps_alias_storage_live_across_later_checked_control_flow() {
    let program = lower_text(concat!(
        "fn combine(ref first: i64, second: i64) -> i64 { return first + second; }\n",
        "fn main() -> i64 { return combine(40 + 2, (i64) 1.0); }\n",
    ));
    verify_mir(&program).expect("later checked primitive cast must retain the earlier alias");
    let storage = alias_storage(&program);
    let function = program.definitions.get(program.entry_function).unwrap();
    let store_block = function
        .body
        .blocks
        .iter()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(instruction, MirInstruction::Store(store)
                    if store.destination == MirPlace::base(storage))
            })
        })
        .unwrap()
        .id;
    let call_block = function
        .body
        .blocks
        .iter()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(instruction, MirInstruction::Call(call)
                    if call.arguments.iter().any(|argument|
                        *argument == MirArgument::Place(MirPlace::base(storage))))
            })
        })
        .unwrap()
        .id;
    assert_ne!(store_block, call_block);
}
