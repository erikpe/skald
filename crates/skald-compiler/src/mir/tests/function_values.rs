use super::*;
use crate::identity::FunctionTypeId;

const SCALAR_SOURCE: &str = concat!(
    "fn add(left: i64, right: i64) -> i64 { return left + right; }\n",
    "fn choose() -> fn(i64, i64) -> i64 { return add; }\n",
    "fn invoke(callback: fn(i64, i64) -> i64) -> i64 { return callback(40, 2); }\n",
    "fn retain_bool(callback: fn(bool) -> bool) -> unit {}\n",
    "fn main() -> i64 { var callback: fn(i64, i64) -> i64 = choose(); return invoke(callback); }\n",
);

fn first_callable_address_mut(program: &mut MirProgram) -> &mut MirCallableAddress {
    let (function, block_index, instruction_index) = program
        .definitions
        .iter()
        .find_map(|definition| {
            definition
                .body
                .blocks
                .iter()
                .enumerate()
                .find_map(|(block_index, block)| {
                    block.instructions.iter().enumerate().find_map(
                        |(instruction_index, instruction)| {
                            matches!(
                                instruction,
                                MirInstruction::Assign(MirAssignment {
                                    rvalue: MirRvalue {
                                        kind: MirRvalueKind::CallableAddress(_),
                                        ..
                                    },
                                    ..
                                })
                            )
                            .then_some((
                                definition.function,
                                block_index,
                                instruction_index,
                            ))
                        },
                    )
                })
        })
        .expect("fixture must form a callable address");
    let instruction = &mut program
        .definitions
        .get_mut_for_test(function)
        .unwrap()
        .body
        .blocks[block_index]
        .instructions[instruction_index];
    let MirInstruction::Assign(MirAssignment {
        rvalue:
            MirRvalue {
                kind: MirRvalueKind::CallableAddress(address),
                ..
            },
        ..
    }) = instruction
    else {
        unreachable!()
    };
    address
}

fn first_indirect_call_mut(program: &mut MirProgram) -> &mut MirCall {
    let (function, block_index, instruction_index) = program
        .definitions
        .iter()
        .find_map(|definition| {
            definition
                .body
                .blocks
                .iter()
                .enumerate()
                .find_map(|(block_index, block)| {
                    block.instructions.iter().enumerate().find_map(
                        |(instruction_index, instruction)| match instruction {
                            MirInstruction::Call(call)
                                if matches!(call.target, MirCallTarget::Indirect(_)) =>
                            {
                                Some((definition.function, block_index, instruction_index))
                            }
                            _ => None,
                        },
                    )
                })
        })
        .expect("fixture must contain an indirect call");
    let instruction = &mut program
        .definitions
        .get_mut_for_test(function)
        .unwrap()
        .body
        .blocks[block_index]
        .instructions[instruction_index];
    let MirInstruction::Call(call) = instruction else {
        unreachable!()
    };
    call
}

fn messages(program: &MirProgram) -> Vec<String> {
    verify_mir(program)
        .expect_err("mutated function-value MIR must be rejected")
        .iter()
        .map(|error| error.message.clone())
        .collect()
}

#[test]
fn lowers_canonical_addresses_storage_results_and_receiverless_indirect_calls() {
    let program = lower_text(SCALAR_SOURCE);
    verify_mir(&program).expect("lowered function-value MIR must verify");

    let signature = program.function_type(FunctionTypeId::new(0)).unwrap();
    assert_eq!(signature.parameters.len(), 2);
    assert_eq!(signature.result, MirType::I64);
    assert_eq!(
        signature.parameters,
        MirParameter::values([MirType::I64, MirType::I64])
    );

    let dump = dump_mir(&program);
    assert!(
        dump.contains("FunctionType ft0 (i64, i64) -> i64"),
        "{dump}"
    );
    assert!(
        dump.contains("callable-address f0 : ft0 : function ft0"),
        "{dump}"
    );
    assert!(dump.contains("call indirect"), "{dump}");
    assert!(!dump.contains("call indirect f2:v0 : ft0 on"), "{dump}");
    assert_eq!(dump, dump_mir(&program));
}

#[test]
fn lowers_indirect_calls_through_every_existing_result_plan() {
    let program = lower_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn identity(value: i64) -> i64 { return value; }\n",
        "fn choose() -> fn(i64) -> i64 { return identity; }\n",
        "fn make_item() -> Item { return Item(1); }\n",
        "fn make_values() -> i64[] { return i64[]{1}; }\n",
        "fn make_maybe() -> i64? { return 1; }\n",
        "fn make_owner() -> shared Item { return new Item(2); }\n",
        "fn object_result(callback: fn() -> Item) -> Item { return callback(); }\n",
        "fn array_result(callback: fn() -> i64[]) -> i64[] { return callback(); }\n",
        "fn optional_result(callback: fn() -> i64?) -> i64? { return callback(); }\n",
        "fn shared_result(callback: fn() -> shared Item) -> shared Item { return callback(); }\n",
        "fn function_result(callback: fn() -> fn(i64) -> i64) -> i64 { return callback()(4); }\n",
        "fn noop() -> unit {}\n",
        "fn unit_result(callback: fn() -> unit) -> unit { callback(); }\n",
        "fn main() -> i64 { return function_result(choose); }\n",
    ));
    verify_mir(&program).expect("all ordinary indirect result plans must verify");
    let dump = dump_mir(&program);
    assert!(dump.matches("call indirect").count() >= 7, "{dump}");
    assert!(dump.contains("shared-result call indirect"), "{dump}");
    assert!(dump.contains("<- call indirect"), "{dump}");
}

#[test]
fn lowers_indirect_arguments_through_every_existing_boundary_plan() {
    let program = lower_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn identity(value: i64) -> i64 { return value; }\n",
        "fn target(\n",
        "  value: i64, ref readonly: Item, mut ref writable: Item, copied: Item,\n",
        "  values: i64[], maybe: i64?, maybe_item: Item?, nested: i64??,\n",
        "  maybe_values: (i64[])?, maybe_owner: shared? Item, owner: shared Item,\n",
        "  callback: fn(i64) -> i64\n",
        ") -> i64 { return callback(value); }\n",
        "fn invoke(\n",
        "  callback: fn(i64, ref Item, mut ref Item, Item, i64[], i64?, Item?, i64??, (i64[])?, shared? Item, shared Item, fn(i64) -> i64) -> i64,\n",
        "  mut ref item: Item, values: i64[], maybe: i64?, maybe_item: Item?,\n",
        "  nested: i64??, maybe_values: (i64[])?, maybe_owner: shared? Item, owner: shared Item\n",
        ") -> i64 {\n",
        "  return callback(1, item, item, item, values, maybe, maybe_item, nested, maybe_values, maybe_owner, owner, identity);\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var item: Item = Item(1);\n",
        "  var values: i64[] = i64[]{1};\n",
        "  var maybe: i64? = 1;\n",
        "  var owner: shared Item = new Item(2);\n",
        "  var maybe_item: Item? = item;\n",
        "  var nested: i64?? = maybe;\n",
        "  var maybe_values: (i64[])? = values;\n",
        "  var maybe_owner: shared? Item = owner;\n",
        "  return invoke(target, item, values, maybe, maybe_item, nested, maybe_values, maybe_owner, owner);\n",
        "}\n",
    ));
    verify_mir(&program).expect("ordinary boundary plans must compose with indirect calls");
    let invoke = program.definitions.get(FunctionId::new(2)).unwrap();
    let call = invoke
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if matches!(call.target, MirCallTarget::Indirect(_)) => {
                Some(call)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(call.arguments.len(), 12);
    assert!(call
        .arguments
        .iter()
        .any(|argument| matches!(argument, MirArgument::OwnedPlace(_))));
    assert!(call
        .arguments
        .iter()
        .any(|argument| matches!(argument, MirArgument::SharedOwner(_))));
    assert!(call
        .arguments
        .iter()
        .any(|argument| matches!(argument, MirArgument::Place(_) | MirArgument::View(_))));
}

#[test]
fn secures_callee_selection_before_control_effectful_arguments() {
    let program = lower_text(concat!(
        "fn identity(value: i64) -> i64 { return value; }\n",
        "fn invoke(callback: fn(i64) -> i64, value: i64?) -> i64 { return callback(value!); }\n",
        "fn main() -> i64 { var value: i64? = 7; return invoke(identity, value); }\n",
    ));
    verify_mir(&program).expect("callee spill across optional control flow must verify");
    let invoke = program.definitions.get(FunctionId::new(1)).unwrap();
    let spill = invoke
        .storage
        .iter()
        .find(|storage| {
            storage.kind == MirStorageKind::ScalarSpill
                && storage.ty == MirType::Function(FunctionTypeId::new(0))
        })
        .expect("callee must be secured before the unwrap CFG");
    let dump = dump_mir(&program);
    let store = dump.find(&format!("store {},", spill.id)).unwrap();
    let unwrap = dump.find("optional-unwrap f1:s1").unwrap();
    let indirect = dump.find("call indirect f1:").unwrap();
    assert!(store < unwrap && unwrap < indirect, "{dump}");
}

#[test]
fn indirect_calls_preserve_loop_epochs_and_reverse_temporary_cleanup() {
    let program = lower_text(concat!(
        "class Leaf { value: i64; init(value: i64) { self.value = value; } destroy {} }\n",
        "class Holder { leaf: Leaf; init(value: i64) { self.leaf = Leaf(value); } destroy {} }\n",
        "fn inspect(ref first: Leaf, ref second: Leaf) -> unit {}\n",
        "fn invoke(callback: fn(ref Leaf, ref Leaf) -> unit, mut ref count: i64) -> unit {\n",
        "  while (count < 2) {\n",
        "    callback(Holder(count).leaf, Holder(count + 1).leaf);\n",
        "    count = count + 1;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { var count: i64 = 0; invoke(inspect, count); return count; }\n",
    ));
    verify_mir(&program).expect("indirect calls in loop epochs must verify");

    let invoke = program
        .definitions
        .iter()
        .find(|definition| {
            definition.body.blocks.iter().any(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(instruction, MirInstruction::Call(call)
                        if matches!(call.target, MirCallTarget::Indirect(_)))
                })
            })
        })
        .expect("fixture must contain the indirect loop body");
    assert!(invoke.body.blocks.iter().any(|block| {
        matches!(block.terminator, Some(MirTerminator::Goto { target, .. })
            if target.index() < block.id.index())
    }));

    let temporaries = invoke
        .storage
        .iter()
        .filter(|storage| storage.kind == MirStorageKind::Temporary)
        .map(|storage| storage.id)
        .collect::<Vec<_>>();
    assert_eq!(temporaries.len(), 2);
    let cleanup = invoke
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::EndFullExpression(end) if end.temporaries.len() == 2 => Some(end),
            _ => None,
        })
        .expect("both produced arguments must share one cleanup boundary");
    assert_eq!(
        cleanup
            .temporaries
            .iter()
            .map(|temporary| temporary.destination.base.expect_local_storage())
            .collect::<Vec<_>>(),
        [temporaries[1], temporaries[0]]
    );
}

#[test]
fn verifier_rejects_malformed_function_metadata_addresses_and_callees() {
    let mut sparse_type = lower_text(SCALAR_SOURCE);
    sparse_type.function_types.entries_mut_for_test()[0].id = FunctionTypeId::new(1);
    assert!(messages(&sparse_type)
        .iter()
        .any(|message| message.contains("function-type table index 0")));

    let mut recursive_type = lower_text(SCALAR_SOURCE);
    recursive_type.function_types.entries_mut_for_test()[0].parameters[0].ty =
        MirType::Function(FunctionTypeId::new(0));
    assert!(messages(&recursive_type)
        .iter()
        .any(|message| message.contains("does not reference bottom-up canonical metadata")));

    let mut unknown_target = lower_text(SCALAR_SOURCE);
    first_callable_address_mut(&mut unknown_target).target = FunctionId::new(99).into();
    assert!(messages(&unknown_target)
        .iter()
        .any(|message| message.contains("callable address target f99 is not declared")));

    let mut mismatched_address_type = lower_text(SCALAR_SOURCE);
    first_callable_address_mut(&mut mismatched_address_type).function_type = FunctionTypeId::new(1);
    assert!(messages(&mismatched_address_type)
        .iter()
        .any(|message| message.contains("callable address target f0 does not match")));

    let mut receiver = lower_text(SCALAR_SOURCE);
    first_indirect_call_mut(&mut receiver).receiver = Some(
        MirMethodReceiver::exact(
            MirPlace::base(StorageId::new(FunctionId::new(2), 0)),
            ClassId::new(0),
            MirAliasAccess::ReadOnly,
        )
        .into(),
    );
    assert!(messages(&receiver)
        .iter()
        .any(|message| message == "indirect function-value call must not have a receiver"));

    let mut wrong_type = lower_text(SCALAR_SOURCE);
    let call = first_indirect_call_mut(&mut wrong_type);
    let MirCallTarget::Indirect(target) = &mut call.target else {
        unreachable!()
    };
    target.function_type = FunctionTypeId::new(1);
    assert!(messages(&wrong_type)
        .iter()
        .any(|message| message.contains("wrong canonical function type")));

    let mut call_result_as_callee = lower_text(SCALAR_SOURCE);
    let call = first_indirect_call_mut(&mut call_result_as_callee);
    let result = call.result.expect("fixture indirect call returns a scalar");
    let MirCallTarget::Indirect(target) = &mut call.target else {
        unreachable!()
    };
    target.callee = result;
    assert!(messages(&call_result_as_callee)
        .iter()
        .any(|message| message.contains("used before it is defined")));
}

#[test]
fn verifier_rejects_arbitrary_scalar_function_value_construction() {
    let mut program = lower_text(SCALAR_SOURCE);
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let assignment = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(assignment.rvalue.kind, MirRvalueKind::CallableAddress(_)) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .unwrap();
    assignment.rvalue.kind = MirRvalueKind::ConstantU64(0);
    assert!(messages(&program)
        .iter()
        .any(|message| message.contains("must originate from a callable address or typed load")));
}

#[test]
fn verifier_rejects_function_storage_loaded_without_definite_initialization() {
    let mut program = lower_text(SCALAR_SOURCE);
    let main = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let store = main.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| match instruction {
            MirInstruction::Store(store) => main
                .storage(store.destination.base.expect_local_storage())
                .is_some_and(|storage| matches!(storage.ty, MirType::Function(_))),
            _ => false,
        })
        .unwrap();
    main.body.blocks[0].instructions.remove(store);

    assert!(messages(&program).iter().any(|message| message
        .contains("is loaded without non-null initialization on every incoming path")));
}

#[test]
fn verifier_rejects_ineligible_and_missing_callable_address_definitions() {
    let source = concat!(
        "class Math {\n",
        "  init() {}\n",
        "  fn instance(value: i64) -> i64 { return value; }\n",
        "  static fn selected(value: i64) -> i64 { return value; }\n",
        "}\n",
        "fn main() -> i64 { var callback: fn(i64) -> i64 = Math.selected; return callback(7); }\n",
    );
    let selected = crate::identity::MethodId::new(ClassId::new(0), 1);

    let mut instance = lower_text(source);
    first_callable_address_mut(&mut instance).target =
        crate::identity::MethodId::new(ClassId::new(0), 0).into();
    assert!(messages(&instance)
        .iter()
        .any(|message| message.contains("is not a static method")));

    let mut missing = lower_text(source);
    missing.member_definitions.remove_for_test(selected.into());
    assert!(messages(&missing).iter().any(
        |message| message.contains("callable address target c0:method1 has no MIR definition")
    ));
}

#[test]
fn indirect_calls_retain_ordinary_ownership_and_result_security_verification() {
    let source = concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn consume(value: Item) -> unit {}\n",
        "fn make() -> Item { return Item(1); }\n",
        "fn own() -> shared Item { return new Item(2); }\n",
        "fn forward(callback: fn(Item) -> unit, value: Item) -> unit { callback(value); }\n",
        "fn object_result(callback: fn() -> Item) -> Item { return callback(); }\n",
        "fn shared_result(callback: fn() -> shared Item) -> shared Item { return callback(); }\n",
        "fn main() -> i64 { var value: Item = make(); forward(consume, value); return 0; }\n",
    );

    let mut corrupt_owner = lower_text(source);
    let forward = corrupt_owner
        .definitions
        .get_mut_for_test(FunctionId::new(3))
        .unwrap();
    let call = forward.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if matches!(call.target, MirCallTarget::Indirect(_)) => {
                Some(call)
            }
            _ => None,
        })
        .unwrap();
    let MirArgument::OwnedPlace(place) = call.arguments[0].clone() else {
        panic!("class value argument must use ordinary ownership transfer")
    };
    call.arguments[0] = MirArgument::Place(place);
    assert!(messages(&corrupt_owner)
        .iter()
        .any(|message| message.contains("must be a scalar value or owned place")));

    let mut lost_object_result = lower_text(source);
    let definition = lost_object_result
        .definitions
        .get_mut_for_test(FunctionId::new(4))
        .unwrap();
    let call = definition.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if matches!(call.target, MirCallTarget::Indirect(_)) => {
                Some(call)
            }
            _ => None,
        })
        .unwrap();
    call.destination = None;
    assert!(messages(&lost_object_result)
        .iter()
        .any(|message| message.contains("object-returning call requires")));

    let mut lost_shared_result = lower_text(source);
    let definition = lost_shared_result
        .definitions
        .get_mut_for_test(FunctionId::new(5))
        .unwrap();
    let call = definition.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if matches!(call.target, MirCallTarget::Indirect(_)) => {
                Some(call)
            }
            _ => None,
        })
        .unwrap();
    call.shared_result = None;
    assert!(messages(&lost_shared_result)
        .iter()
        .any(|message| message.contains("shared-returning call requires")));
}

#[test]
fn preliminary_static_initializers_lower_and_dump_callable_addresses() {
    let source = concat!(
        "fn identity(value: i64) -> i64 { return value; }\n",
        "class Registry {\n",
        "  static callback: fn(i64) -> i64 = identity;\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return Registry.callback(7); }\n",
    );
    let checked = crate::test_support::type_check_source(source);
    let hir = checked.hir.unwrap();
    let preliminary = lower_preliminary_hir(&hir);
    verify_preliminary_mir(&preliminary)
        .expect("function-valued static initializer MIR must verify");
    let dump = dump_preliminary_mir(&preliminary);
    assert!(dump.contains("FunctionTypes"), "{dump}");
    assert!(dump.contains("callable-address f0 : ft0"), "{dump}");
    assert!(dump.contains("StaticInitializer"), "{dump}");
    assert_eq!(dump, dump_preliminary_mir(&preliminary));

    let final_mir = crate::test_support::lower_hir_to_final_mir(&hir);
    verify_mir(&final_mir).expect("synthesized function-valued static MIR must verify");
    let assembly = crate::test_support::emit_assembly_without_runtime_trace(
        crate::backend::Target::X86_64SysV,
        &final_mir,
    )
    .expect("function-valued static initializer MIR must reach x86-64");
    assert!(assembly.contains("lea rax, [rip + .Lska.fn.main.identity.f0]"));
    assert!(assembly.contains("call r11"));
}

#[test]
fn closed_generic_static_addresses_retain_exact_targets_and_signatures() {
    let program = crate::test_support::lower_generic_source_to_final_mir(concat!(
        "class Identity<T> { init() {} static fn apply(value: T) -> T { return value; } }\n",
        "fn main() -> i64 {\n",
        "  var integer: fn(i64) -> i64 = Identity<i64>.apply;\n",
        "  var boolean: fn(bool) -> bool = Identity<bool>.apply;\n",
        "  if (boolean(true)) { return integer(7); }\n",
        "  return 0;\n",
        "}\n",
    ));
    verify_mir(&program).expect("closed generic callable addresses must verify");
    let addresses = program
        .definitions
        .get(program.entry_function)
        .unwrap()
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Assign(MirAssignment {
                rvalue:
                    MirRvalue {
                        kind: MirRvalueKind::CallableAddress(address),
                        ..
                    },
                ..
            }) => Some(*address),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(addresses.len(), 2);
    assert_ne!(addresses[0].target, addresses[1].target);
    assert_ne!(addresses[0].function_type, addresses[1].function_type);
    assert!(matches!(
        addresses[0].target,
        crate::identity::CallableId::Method(_)
    ));
}
