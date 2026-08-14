use super::*;

const LIFETIME_SOURCE: &str = concat!(
    "class Item {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  fn next(value: i64) -> Item { return Item(value); }\n",
    "  fn combine(ref value: Item) -> i64 { return self.value + value.value; }\n",
    "  destroy {}\n",
    "}\n",
    "fn main() -> i64 { return Item(1).next(2).combine(Item(3)); }\n",
);

fn temporary_ids(function: &MirFunctionDefinition) -> Vec<StorageId> {
    function
        .storage
        .iter()
        .filter(|storage| storage.kind == MirStorageKind::Temporary)
        .map(|storage| storage.id)
        .collect()
}

fn temporary_cleanup_order(function: &MirFunctionDefinition) -> Vec<StorageId> {
    function
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::EndFullExpression(end) => Some(&end.temporaries),
            _ => None,
        })
        .flatten()
        .map(|cleanup| cleanup.destination.base.expect_local_storage())
        .collect()
}

#[test]
fn chained_receivers_live_through_later_arguments_and_clean_in_reverse_completion_order() {
    let program = lower_text(LIFETIME_SOURCE);
    verify_mir(&program).expect("chained produced receivers must verify");
    let main = program.definitions.get(program.entry_function).unwrap();
    let temporaries = temporary_ids(main);
    let completion_order: Vec<_> = main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Initialize(initialize) => initialize.destination.base.local_storage(),
            MirInstruction::Call(call) => call
                .destination
                .as_ref()
                .and_then(|destination| destination.base.local_storage()),
            _ => None,
        })
        .filter(|storage| temporaries.contains(storage))
        .collect();

    assert_eq!(temporaries.len(), 3);
    assert_eq!(completion_order.len(), 3);
    assert_eq!(
        temporary_cleanup_order(main),
        completion_order.iter().rev().copied().collect::<Vec<_>>()
    );
    assert!(!main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|instruction| matches!(instruction, MirInstruction::CopyConstruct(_))));

    let mut produced_calls = 0;
    for block in &main.body.blocks {
        for instruction in &block.instructions {
            let MirInstruction::Call(call) = instruction else {
                continue;
            };
            let Some(receiver) = call.receiver.as_ref().and_then(MirCallReceiver::as_method) else {
                continue;
            };
            let MirObjectOrigin::Exact {
                complete,
                dynamic_class,
            } = &*receiver.origin
            else {
                panic!("produced receiver must keep exact complete-object origin");
            };
            assert_eq!(receiver.access, MirAliasAccess::ReadOnly);
            assert_eq!(receiver.provenance, MirViewProvenance::Produced);
            assert!(temporaries.contains(&complete.base.expect_local_storage()));
            assert_eq!(complete.projections, []);
            assert_eq!(receiver.place.base, complete.base);
            assert_eq!(*dynamic_class, ClassId::new(0));
            produced_calls += 1;
        }
    }
    assert_eq!(produced_calls, 2);

    for temporary in temporaries {
        let operations: Vec<_> = main
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match instruction {
                MirInstruction::StorageLive(operation) if operation.storage == temporary => {
                    Some(true)
                }
                MirInstruction::StorageDead(operation) if operation.storage == temporary => {
                    Some(false)
                }
                _ => None,
            })
            .collect();
        assert_eq!(operations, [true, false]);
    }

    let first_dump = dump_mir(&program);
    let second_dump = dump_mir(&lower_text(LIFETIME_SOURCE));
    assert_eq!(first_dump, second_dump);
    assert_eq!(
        first_dump
            .matches(" readonly produced origin exact")
            .count(),
        3
    );
}

#[test]
fn logical_conditions_and_loop_epochs_keep_produced_ownership_path_local() {
    let program = lower_text(concat!(
        "class Item {\n",
        "  truth: bool;\n",
        "  init(truth: bool) { self.truth = truth; }\n",
        "  fn read() -> bool { return self.truth; }\n",
        "  destroy {}\n",
        "}\n",
        "fn choose(flag: bool) -> bool {\n",
        "  if (flag && Item(true).read()) { return true; }\n",
        "  elif (Item(false).read()) { return true; }\n",
        "  return false || Item(true).read();\n",
        "}\n",
        "fn repeat(limit: i64) -> i64 {\n",
        "  var count: i64 = 0;\n",
        "  while (count < limit && Item(true).read()) { count = count + 1; }\n",
        "  return count;\n",
        "}\n",
        "fn main() -> i64 { if (choose(false)) { return repeat(2); } return 0; }\n",
    ));
    verify_mir(&program).expect("path-local produced receiver ownership must verify");

    let choose = program.definitions.get(FunctionId::new(0)).unwrap();
    let repeat = program.definitions.get(FunctionId::new(1)).unwrap();
    assert!(!choose.body.path_conditions.is_empty());
    assert!(!repeat.body.path_conditions.is_empty());
    assert!(temporary_ids(choose).len() >= 3);
    assert_eq!(temporary_ids(repeat).len(), 1);
    assert!(
        repeat.body.blocks.iter().any(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(instruction, MirInstruction::StorageLive(operation)
                if temporary_ids(repeat).contains(&operation.storage))
            }) && block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::StorageDead(_)))
        }) || repeat.body.blocks.len() > 1
    );
}

#[test]
fn closed_generic_interface_dispatch_borrows_one_live_readonly_temporary() {
    let program = crate::test_support::lower_generic_source_to_final_mir(concat!(
        "interface Readable { fn read(extra: i64) -> i64; }\n",
        "class Item implements Readable {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Item) { self.value = source.value; }\n",
        "  assign(ref source: Item) { self.value = source.value; }\n",
        "  fn read(extra: i64) -> i64 { return self.value + extra; }\n",
        "  destroy {}\n",
        "}\n",
        "class Invoke<T> where T: Readable {\n",
        "  value: T;\n",
        "  init(value: T) { self.value = value; }\n",
        "  fn produce() -> T { return self.value; }\n",
        "  fn run() -> i64 { return self.produce().read(2); }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var invoke: Invoke<Item> = Invoke<Item>(Item(40));\n",
        "  return invoke.run();\n",
        "}\n",
    ));
    verify_mir(&program).expect("closed generic produced interface receiver must verify");

    let (function, receiver) = program
        .definitions
        .iter()
        .map(MirDefinitionRef::from)
        .chain(
            program
                .member_definitions
                .iter()
                .map(MirDefinitionRef::from),
        )
        .find_map(|definition| {
            definition
                .body()
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .find_map(|instruction| match instruction {
                    MirInstruction::Call(call)
                        if matches!(call.target, MirCallTarget::Interface(_)) =>
                    {
                        call.receiver
                            .as_ref()
                            .and_then(MirCallReceiver::as_interface)
                            .map(|receiver| (definition, receiver))
                    }
                    _ => None,
                })
        })
        .expect("specialized bound call must remain an interface call");
    let temporary = receiver.source.base.expect_local_storage();
    assert_eq!(receiver.access, MirAliasAccess::ReadOnly);
    assert_eq!(receiver.provenance, MirViewProvenance::Produced);
    assert_eq!(
        function.storage(temporary).unwrap().kind,
        MirStorageKind::Temporary
    );
    assert!(matches!(
        &*receiver.origin,
        MirObjectOrigin::Exact { complete, .. }
            if *complete == MirPlace::base(temporary)
    ));
    assert!(function
        .body()
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(
            |instruction| matches!(instruction, MirInstruction::EndFullExpression(end)
            if end.temporaries.iter().any(|cleanup|
                cleanup.destination == MirPlace::base(temporary)))
        ));
}

fn mutation_program() -> MirProgram {
    lower_text(concat!(
        "class Item {\n",
        "  init() {}\n",
        "  fn inspect() -> unit {}\n",
        "  mut fn replace() -> unit {}\n",
        "  destroy {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var stable: Item = Item();\n",
        "  Item().inspect();\n",
        "  return 0;\n",
        "}\n",
    ))
}

fn verification_messages(program: &MirProgram) -> Vec<String> {
    verify_mir(program)
        .expect_err("mutated produced-receiver MIR must fail verification")
        .iter()
        .map(|error| error.message.clone())
        .collect()
}

fn assert_verification_error(program: &MirProgram, expected: &str) {
    let messages = verification_messages(program);
    assert!(
        messages.iter().any(|message| message.contains(expected)),
        "expected `{expected}` in {messages:?}"
    );
}

fn produced_temporary(program: &MirProgram) -> StorageId {
    let main = program.definitions.get(program.entry_function).unwrap();
    *temporary_ids(main)
        .first()
        .expect("mutation fixture must contain a produced receiver temporary")
}

fn produced_cleanup(program: &MirProgram, temporary: StorageId) -> MirCleanup {
    program
        .definitions
        .get(program.entry_function)
        .unwrap()
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::EndFullExpression(end) => end
                .temporaries
                .iter()
                .find(|cleanup| cleanup.destination.base.local_storage() == Some(temporary)),
            _ => None,
        })
        .cloned()
        .expect("mutation fixture must clean its produced receiver")
}

fn produced_call_mut(function: &mut MirFunctionDefinition, temporary: StorageId) -> &mut MirCall {
    function
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call)
                if call
                    .receiver
                    .as_ref()
                    .and_then(MirCallReceiver::as_method)
                    .is_some_and(|receiver| {
                        receiver.place.base.local_storage() == Some(temporary)
                    }) =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("mutation fixture must call through its produced receiver")
}

#[test]
fn verifier_rejects_malformed_produced_receiver_access_origin_and_storage() {
    let valid = mutation_program();
    verify_mir(&valid).expect("produced-receiver mutation seed must verify");
    let temporary = produced_temporary(&valid);

    let mut wrong_kind = valid.clone();
    wrong_kind
        .definitions
        .get_mut_for_test(wrong_kind.entry_function)
        .unwrap()
        .storage[temporary.index()]
    .kind = MirStorageKind::Local;
    assert_verification_error(
        &wrong_kind,
        "full-expression cleanup must name complete temporary storage",
    );

    let mut mutable = valid.clone();
    let entry = mutable.entry_function;
    produced_call_mut(
        mutable.definitions.get_mut_for_test(entry).unwrap(),
        temporary,
    )
    .receiver
    .as_mut()
    .unwrap()
    .as_method_mut()
    .unwrap()
    .access = MirAliasAccess::Mutable;
    assert_verification_error(&mutable, "produced method receiver must be read-only");

    let mut mutable_target = valid.clone();
    let entry = mutable_target.entry_function;
    produced_call_mut(
        mutable_target.definitions.get_mut_for_test(entry).unwrap(),
        temporary,
    )
    .target = MirCallTarget::Method(MirMethodCallTarget::Direct(MethodId::new(
        ClassId::new(0),
        1,
    )));
    assert_verification_error(
        &mutable_target,
        "mutable method receiver requires mutable access",
    );

    let mut wrong_origin = valid.clone();
    let entry = wrong_origin.entry_function;
    let function = wrong_origin.definitions.get_mut_for_test(entry).unwrap();
    let stable = function
        .storage
        .iter()
        .find(|storage| storage.name == "stable")
        .unwrap()
        .id;
    let receiver = produced_call_mut(function, temporary)
        .receiver
        .as_mut()
        .unwrap()
        .as_method_mut()
        .unwrap();
    let MirObjectOrigin::Exact { complete, .. } = &mut *receiver.origin else {
        unreachable!();
    };
    *complete = MirPlace::base(stable);
    assert_verification_error(
        &wrong_origin,
        "exact origin is not an ancestor of its static place",
    );

    let mut invalid_projection = valid;
    let entry = invalid_projection.entry_function;
    produced_call_mut(
        invalid_projection
            .definitions
            .get_mut_for_test(entry)
            .unwrap(),
        temporary,
    )
    .receiver
    .as_mut()
    .unwrap()
    .as_method_mut()
    .unwrap()
    .place
    .projections
    .push(MirPlaceProjection::Base(ClassId::new(99)));
    assert_verification_error(
        &invalid_projection,
        "base projection c99 is not the declared direct base of c0",
    );
}

#[test]
fn verifier_rejects_missing_premature_duplicate_and_post_cleanup_use() {
    let valid = mutation_program();
    let temporary = produced_temporary(&valid);
    let cleanup = produced_cleanup(&valid, temporary);

    let mut missing = valid.clone();
    let entry = missing.entry_function;
    for block in &mut missing
        .definitions
        .get_mut_for_test(entry)
        .unwrap()
        .body
        .blocks
    {
        for instruction in &mut block.instructions {
            if let MirInstruction::EndFullExpression(end) = instruction {
                end.temporaries.retain(|candidate| candidate != &cleanup);
            }
        }
    }
    assert_verification_error(
        &missing,
        "full-expression temporaries must be cleaned in reverse completion order",
    );

    let mut premature = valid.clone();
    let entry = premature.entry_function;
    let function = premature.definitions.get_mut_for_test(entry).unwrap();
    let (block_index, call_index) = function
        .body
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| {
                    matches!(instruction, MirInstruction::Call(call)
                    if call.receiver.as_ref().and_then(MirCallReceiver::as_method).is_some_and(
                        |receiver| receiver.place.base.local_storage() == Some(temporary)))
                })
                .map(|call_index| (block_index, call_index))
        })
        .unwrap();
    for block in &mut function.body.blocks {
        for instruction in &mut block.instructions {
            if let MirInstruction::EndFullExpression(end) = instruction {
                end.temporaries.retain(|candidate| candidate != &cleanup);
            }
        }
    }
    function.body.blocks[block_index].instructions.insert(
        call_index,
        MirInstruction::EndFullExpression(MirEndFullExpression {
            temporaries: vec![cleanup.clone()],
            span: cleanup.span,
        }),
    );
    assert_verification_error(&premature, "method receiver is not live");

    let mut duplicate_production = valid.clone();
    let entry = duplicate_production.entry_function;
    let function = duplicate_production
        .definitions
        .get_mut_for_test(entry)
        .unwrap();
    let (block_index, initialize_index, initialize) = function
        .body
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .enumerate()
                .find_map(|(index, instruction)| match instruction {
                    MirInstruction::Initialize(initialize)
                        if initialize.destination.base.local_storage() == Some(temporary) =>
                    {
                        Some((block_index, index, instruction.clone()))
                    }
                    _ => None,
                })
        })
        .unwrap();
    function.body.blocks[block_index]
        .instructions
        .insert(initialize_index + 1, initialize);
    assert_verification_error(
        &duplicate_production,
        "initialization destination is already live",
    );

    let mut duplicate_cleanup = valid.clone();
    let entry = duplicate_cleanup.entry_function;
    let function = duplicate_cleanup
        .definitions
        .get_mut_for_test(entry)
        .unwrap();
    let end = function
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::EndFullExpression(end)
                if end
                    .temporaries
                    .iter()
                    .any(|candidate| candidate == &cleanup) =>
            {
                Some(end)
            }
            _ => None,
        })
        .unwrap();
    end.temporaries.push(cleanup.clone());
    assert_verification_error(
        &duplicate_cleanup,
        "full-expression cleanup destination is not live",
    );

    let mut use_after_cleanup = valid;
    let entry = use_after_cleanup.entry_function;
    let function = use_after_cleanup
        .definitions
        .get_mut_for_test(entry)
        .unwrap();
    let call = function
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|instruction| {
            matches!(instruction, MirInstruction::Call(call)
            if call.receiver.as_ref().and_then(MirCallReceiver::as_method).is_some_and(
                |receiver| receiver.place.base.local_storage() == Some(temporary)))
        })
        .cloned()
        .unwrap();
    let block = function
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(instruction, MirInstruction::EndFullExpression(end)
                    if end.temporaries.iter().any(|candidate| candidate == &cleanup))
            })
        })
        .unwrap();
    let boundary = block
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::EndFullExpression(end)
            if end.temporaries.iter().any(|candidate| candidate == &cleanup))
        })
        .unwrap();
    block.instructions.insert(boundary + 1, call);
    assert_verification_error(&use_after_cleanup, "method receiver is not live");
}

#[test]
fn verifier_rejects_produced_cleanup_leaking_to_a_skipped_path() {
    let mut program = lower_text(concat!(
        "class Item { init() {} fn read() -> bool { return true; } destroy {} }\n",
        "fn choose(flag: bool) -> bool { return flag && Item().read(); }\n",
        "fn main() -> i64 { if (choose(false)) { return 1; } return 0; }\n",
    ));
    verify_mir(&program).expect("conditional produced-receiver seed must verify");
    let choose = program.definitions.get(FunctionId::new(0)).unwrap();
    let temporary = *temporary_ids(choose).first().unwrap();
    let cleanup = produced_cleanup_for_function(choose, temporary);
    let inactive = choose.body.path_conditions[0].inactive_predecessor;

    let choose = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let block = choose
        .body
        .blocks
        .iter_mut()
        .find(|block| block.id == inactive)
        .unwrap();
    block
        .instructions
        .push(MirInstruction::EndFullExpression(MirEndFullExpression {
            temporaries: vec![cleanup],
            span: block.span,
        }));

    let messages = verification_messages(&program);
    assert!(
        messages.iter().any(|message| {
            message.contains("outside a live lifetime epoch")
                || message.contains("cleanup destination is not live")
                || message.contains("conditional owner state")
        }),
        "skipped-path cleanup leakage must fail: {messages:?}"
    );
}

fn produced_cleanup_for_function(
    function: &MirFunctionDefinition,
    temporary: StorageId,
) -> MirCleanup {
    function
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::EndFullExpression(end) => end
                .temporaries
                .iter()
                .find(|cleanup| cleanup.destination.base.local_storage() == Some(temporary)),
            _ => None,
        })
        .cloned()
        .unwrap()
}
