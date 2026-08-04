use super::*;

fn function_id(program: &MirProgram, name: &str) -> FunctionId {
    program
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .unwrap_or_else(|| panic!("fixture function `{name}` must be declared"))
        .id
}

fn verifier_errors(program: &MirProgram) -> String {
    verify_mir(program)
        .expect_err("mutated produced-alias MIR must fail verification")
        .to_string()
}

fn produced_alias_fixture() -> MirProgram {
    lower_text(concat!(
        "class Base { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Value extends Base { init(value: i64) { super(value); } }\n",
        "fn make(value: i64) -> Value { return Value(value); }\n",
        "fn later() -> i64 { return 10; }\n",
        "fn observe(ref first: Base, marker: i64, ref second: Obj) -> i64 {\n",
        "  return first.value + marker;\n",
        "}\n",
        "fn relay(ref value: Base) -> i64 { return observe(value, later(), Value(4)); }\n",
        "fn main() -> i64 {\n",
        "  var first: i64 = observe(Value(1), later(), make(2));\n",
        "  return first + relay(Value(3));\n",
        "}\n",
    ))
}

#[test]
fn produced_aliases_lower_once_in_source_order_and_cleanup_in_reverse() {
    let program = produced_alias_fixture();
    verify_mir(&program).unwrap();

    let make = function_id(&program, "make");
    let later = function_id(&program, "later");
    let observe = function_id(&program, "observe");
    let relay = function_id(&program, "relay");
    let main = program.definitions.get(program.entry_function).unwrap();
    let temporaries: Vec<_> = main
        .storage
        .iter()
        .filter(|storage| storage.kind == MirStorageKind::Temporary)
        .map(|storage| storage.id)
        .collect();
    assert_eq!(temporaries.len(), 3);

    let instructions = &main.body.blocks[0].instructions;
    let first_initialize = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Initialize(initialize)
                if initialize.destination == MirPlace::base(temporaries[0]))
        })
        .unwrap();
    let later_call = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Call(call)
                if call.target == MirCallTarget::Direct(later))
        })
        .unwrap();
    let make_call = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Call(call)
                if call.target == MirCallTarget::Direct(make))
        })
        .unwrap();
    let observe_call = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Call(call)
                if call.target == MirCallTarget::Direct(observe))
        })
        .unwrap();
    assert!(first_initialize < later_call && later_call < make_call && make_call < observe_call);

    let MirInstruction::Call(call) = &instructions[observe_call] else {
        unreachable!();
    };
    for (argument, temporary) in [
        (&call.arguments[0], temporaries[0]),
        (&call.arguments[2], temporaries[1]),
    ] {
        let MirArgument::View(view) = argument else {
            panic!("produced alias must lower as an object view");
        };
        assert_eq!(view.source.base.local_storage(), Some(temporary));
        assert_eq!(view.access, MirAliasAccess::ReadOnly);
        let MirObjectOrigin::Exact { complete, .. } = view.origin.as_ref() else {
            panic!("produced alias must retain exact complete-object provenance");
        };
        assert_eq!(complete, &MirPlace::base(temporary));
    }

    let first_boundary = instructions[observe_call + 1..]
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::EndFullExpression(end) => Some(end),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        first_boundary
            .temporaries
            .iter()
            .map(|cleanup| cleanup.destination.base.expect_local_storage())
            .collect::<Vec<_>>(),
        [temporaries[1], temporaries[0]]
    );

    for temporary in &temporaries {
        let lifetime: Vec<_> = main
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match instruction {
                MirInstruction::StorageLive(operation) if operation.storage == *temporary => {
                    Some(true)
                }
                MirInstruction::StorageDead(operation) if operation.storage == *temporary => {
                    Some(false)
                }
                _ => None,
            })
            .collect();
        assert_eq!(lifetime, [true, false]);
    }
    assert!(!main.body.blocks.iter().flat_map(|block| &block.instructions).any(
        |instruction| matches!(instruction, MirInstruction::CopyConstruct(copy)
            if copy.destination.base.local_storage().is_some_and(|storage| temporaries.contains(&storage)))
    ));

    let relay_definition = program.definitions.get(relay).unwrap();
    let forwarded_call = relay_definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if call.target == MirCallTarget::Direct(observe) => {
                Some(call)
            }
            _ => None,
        })
        .unwrap();
    let MirArgument::View(forwarded) = &forwarded_call.arguments[0] else {
        panic!("relay must forward its alias without taking ownership");
    };
    assert!(matches!(
        forwarded.source.base,
        MirPlaceBase::AliasParameter(_)
    ));

    let dump = dump_mir(&program);
    assert_eq!(dump, dump_mir(&program));
    assert!(dump.contains("end-full-expression cleanup"), "{dump}");
}

#[test]
fn static_produced_cast_projects_the_temporary_without_a_checked_carrier() {
    let program = lower_text(concat!(
        "class Base { init() {} }\n",
        "class Value extends Base { init() { super(); } }\n",
        "fn inspect(ref value: Base) -> i64 { return 1; }\n",
        "fn main() -> i64 { return inspect((Base) Value()); }\n",
    ));
    verify_mir(&program).unwrap();

    let main = program.definitions.get(program.entry_function).unwrap();
    assert!(main
        .storage
        .iter()
        .all(|storage| !matches!(storage.kind, MirStorageKind::CheckedView(_))));
    let temporary = main
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::Temporary)
        .unwrap()
        .id;
    let view = main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => match &call.arguments[0] {
                MirArgument::View(view) => Some(view),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    assert_eq!(view.source.base.local_storage(), Some(temporary));
    assert_eq!(view.source.projections.len(), 1);
    let MirObjectOrigin::Exact { complete, .. } = view.origin.as_ref() else {
        panic!("static produced cast must keep exact provenance");
    };
    assert_eq!(complete, &MirPlace::base(temporary));
}

#[test]
fn checked_carriers_end_before_produced_temporary_cleanup() {
    let source = concat!(
        "class Value { init() {} }\n",
        "fn inspect(ref value: Value, ref other: Value) -> i64 { return 1; }\n",
        "fn checked(ref value: Obj) -> i64 { return inspect((Value) value, Value()); }\n",
        "fn main() -> i64 { var value: Value = Value(); return checked(value); }\n",
    );
    let program = lower_text(source);
    verify_mir(&program).unwrap();

    let checked = program
        .definitions
        .get(function_id(&program, "checked"))
        .unwrap();
    let instructions: Vec<_> = checked
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .collect();
    let carrier_end = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndCheckedView(_)))
        .unwrap();
    let cleanup = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::EndFullExpression(end)
                if !end.temporaries.is_empty())
        })
        .unwrap();
    assert!(carrier_end < cleanup);

    let mut malformed = lower_text(source);
    let checked = malformed
        .definitions
        .get_mut_for_test(function_id(&program, "checked"))
        .unwrap();
    let block = checked
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::EndCheckedView(_)))
                && block.instructions.iter().any(|instruction| {
                    matches!(instruction, MirInstruction::EndFullExpression(end)
                        if !end.temporaries.is_empty())
                })
        })
        .unwrap();
    let carrier_end = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndCheckedView(_)))
        .unwrap();
    let cleanup = block
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::EndFullExpression(end)
                if !end.temporaries.is_empty())
        })
        .unwrap();
    block.instructions.swap(carrier_end, cleanup);
    assert!(verifier_errors(&malformed)
        .contains("checked-view carriers must end before owning temporary cleanup"));
}

#[test]
fn selected_produced_alias_has_only_path_selected_lifetime_effects() {
    let program = lower_text(concat!(
        "class Value { init() {} }\n",
        "fn inspect(ref value: Value) -> bool { return true; }\n",
        "fn main() -> i64 { if (false && inspect(Value())) { return 1; } return 0; }\n",
    ));
    verify_mir(&program).unwrap();

    let main = program.definitions.get(program.entry_function).unwrap();
    assert_eq!(main.body.path_conditions.len(), 1);
    let temporary = main
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::Temporary)
        .unwrap()
        .id;
    let producer_block = main
        .body
        .blocks
        .iter()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(instruction, MirInstruction::Initialize(initialize)
                    if initialize.destination == MirPlace::base(temporary))
            })
        })
        .unwrap();
    assert_ne!(producer_block.id, main.body.entry);
    let operations: Vec<_> = main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::StorageLive(operation) if operation.storage == temporary => {
                Some("live")
            }
            MirInstruction::Initialize(initialize)
                if initialize.destination == MirPlace::base(temporary) =>
            {
                Some("initialize")
            }
            MirInstruction::EndFullExpression(end)
                if end
                    .temporaries
                    .iter()
                    .any(|cleanup| cleanup.destination == MirPlace::base(temporary)) =>
            {
                Some("cleanup")
            }
            MirInstruction::StorageDead(operation) if operation.storage == temporary => {
                Some("dead")
            }
            _ => None,
        })
        .collect();
    assert_eq!(operations, ["live", "initialize", "cleanup", "dead"]);
}

fn simple_mutation_fixture() -> MirProgram {
    lower_text(concat!(
        "class Value { field: i64; init(field: i64) { self.field = field; } }\n",
        "fn inspect(ref value: Value) -> i64 { return value.field; }\n",
        "fn main() -> i64 { return inspect(Value(7)); }\n",
    ))
}

#[test]
fn verifier_rejects_malformed_produced_alias_lifetimes_and_views() {
    let mut premature = simple_mutation_fixture();
    let function = premature
        .definitions
        .get_mut_for_test(premature.entry_function)
        .unwrap();
    let instructions = &mut function.body.blocks[0].instructions;
    let call = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Call(_)))
        .unwrap();
    let boundary = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndFullExpression(_)))
        .unwrap();
    let cleanup = instructions.remove(boundary);
    instructions.insert(call, cleanup);
    assert!(verifier_errors(&premature).contains("object view source is not live"));

    let mut missing = simple_mutation_fixture();
    let function = missing
        .definitions
        .get_mut_for_test(missing.entry_function)
        .unwrap();
    let end = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::EndFullExpression(end) => Some(end),
            _ => None,
        })
        .unwrap();
    end.temporaries.clear();
    assert!(verifier_errors(&missing).contains("full-expression temporaries must be cleaned"));

    let mut mutable = simple_mutation_fixture();
    let function = mutable
        .definitions
        .get_mut_for_test(mutable.entry_function)
        .unwrap();
    let view = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => match &mut call.arguments[0] {
                MirArgument::View(view) => Some(view),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    view.access = MirAliasAccess::Mutable;
    assert!(
        verifier_errors(&mutable).contains("cannot grant mutable access to a produced temporary")
    );

    let mut wrong_order = simple_mutation_fixture();
    let function = wrong_order
        .definitions
        .get_mut_for_test(wrong_order.entry_function)
        .unwrap();
    let instructions = &mut function.body.blocks[0].instructions;
    let initialize = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Initialize(_)))
        .unwrap();
    let call = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Call(_)))
        .unwrap();
    instructions.swap(initialize, call);
    assert!(verifier_errors(&wrong_order).contains("object view source is not live"));

    let mut invalid_origin = simple_mutation_fixture();
    let function = invalid_origin
        .definitions
        .get_mut_for_test(invalid_origin.entry_function)
        .unwrap();
    let field = FieldId::new(ClassId::new(0), 0);
    let view = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => match &mut call.arguments[0] {
                MirArgument::View(view) => Some(view),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    let MirObjectOrigin::Exact { complete, .. } = view.origin.as_mut() else {
        unreachable!();
    };
    *complete = complete.clone().project_field(field);
    assert!(verifier_errors(&invalid_origin).contains("exact origin"));

    let mut duplicate = simple_mutation_fixture();
    let function = duplicate
        .definitions
        .get_mut_for_test(duplicate.entry_function)
        .unwrap();
    let end = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::EndFullExpression(end) => Some(end),
            _ => None,
        })
        .unwrap();
    end.temporaries.push(end.temporaries[0].clone());
    assert!(verifier_errors(&duplicate).contains("full-expression cleanup destination is not live"));
}

#[test]
fn verifier_requires_one_lifetime_epoch_per_temporary_storage() {
    let mut program = simple_mutation_fixture();
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let temporary = function
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::Temporary)
        .unwrap()
        .id;
    let instructions = &mut function.body.blocks[0].instructions;
    let live = instructions
        .iter()
        .find(|instruction| {
            matches!(instruction, MirInstruction::StorageLive(operation)
                if operation.storage == temporary)
        })
        .unwrap()
        .clone();
    let dead = instructions
        .iter()
        .rposition(|instruction| {
            matches!(instruction, MirInstruction::StorageDead(operation)
                if operation.storage == temporary)
        })
        .unwrap();
    instructions.insert(dead + 1, live);

    assert!(verifier_errors(&program).contains("must have one non-reused lifetime epoch"));
}
