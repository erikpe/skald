use crate::{
    identity::{ClassId, FieldId, FunctionId, InitializerId, MethodId},
    mir::{
        verify_mir, MirAliasAccess, MirCall, MirCallTarget, MirInstruction, MirMethodCallTarget,
        MirMethodKind, MirMethodReceiver, MirPlace, MirReceiverAccess, MirType, MirValue, ValueId,
    },
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
fn direct_call_target_and_arity_contracts_retain_exact_diagnostics() {
    let source = concat!(
        "fn one(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { return one(1); }\n",
    );
    let mut wrong_arity = lower_source_to_mir(source);
    call_mut(&mut wrong_arity).arguments.clear();
    assert!(messages(&wrong_arity)
        .iter()
        .any(|message| message == "call has 0 arguments but requires 1"));

    let mut unknown_target = lower_source_to_mir(source);
    call_mut(&mut unknown_target).target = MirCallTarget::Direct(FunctionId::new(99));
    assert!(messages(&unknown_target)
        .iter()
        .any(|message| message == "call target f99 is not declared"));
}

#[test]
fn scalar_result_presence_contracts_retain_exact_diagnostics() {
    let mut missing = lower_source_to_mir(concat!(
        "fn value() -> i64 { return 1; }\n",
        "fn main() -> i64 { return value(); }\n",
    ));
    call_mut(&mut missing).result = None;
    assert!(messages(&missing)
        .iter()
        .any(|message| message == "value-returning call has no result"));

    let mut unexpected = lower_source_to_mir(concat!(
        "fn notify() -> unit {}\n",
        "fn main() -> i64 { notify(); return 0; }\n",
    ));
    let function = unexpected
        .definitions
        .get_mut_for_test(unexpected.entry_function)
        .unwrap();
    let result = ValueId::new(function.function, function.values.len());
    function.values.push(MirValue {
        id: result,
        ty: MirType::I64,
        span: function.span,
    });
    let call = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    call.result = Some(result);
    assert!(messages(&unexpected)
        .iter()
        .any(|message| message == "unit-returning call must not have a result"));
}

#[test]
fn initializer_and_method_receiver_contracts_retain_exact_diagnostics() {
    let source = concat!(
        "class Box { value: i64; init(value: i64) { self.value = value; } ",
        "fn get() -> i64 { return self.value; } }\n",
        "fn main() -> i64 { var value: Box = Box(1); return value.get(); }\n",
    );
    let mut initializer = lower_source_to_mir(source);
    let function = initializer
        .definitions
        .get_mut_for_test(initializer.entry_function)
        .unwrap();
    let initialize = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Initialize(initialize) => Some(initialize),
            _ => None,
        })
        .unwrap();
    initialize.arguments.clear();
    assert!(messages(&initializer)
        .iter()
        .any(|message| message == "initializer has 0 arguments but requires 1"));

    let mut method = lower_source_to_mir(source);
    let function = method
        .definitions
        .get_mut_for_test(method.entry_function)
        .unwrap();
    let call = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    call.receiver = None;
    assert!(messages(&method)
        .iter()
        .any(|message| message == "method call requires a receiver"));

    let mut wrong_receiver = lower_source_to_mir(source);
    let function = wrong_receiver
        .definitions
        .get_mut_for_test(wrong_receiver.entry_function)
        .unwrap();
    let storage = function.storage[0].id;
    let call = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    call.receiver = Some(
        MirMethodReceiver::exact(
            MirPlace::base(storage).project_field(FieldId::new(ClassId::new(0), 0)),
            ClassId::new(0),
            MirAliasAccess::Mutable,
        )
        .into(),
    );
    assert!(messages(&wrong_receiver)
        .iter()
        .any(|message| message == "method receiver has the wrong class type"));
}

#[test]
fn static_and_instance_call_kinds_require_matching_targets_and_receivers() {
    let source = concat!(
        "class Math {\n",
        "  init() {}\n",
        "  static fn answer(value: i64) -> i64 { return value + 1; }\n",
        "}\n",
        "fn main() -> i64 { var math: Math = Math(); return Math.answer(41); }\n",
    );
    let method = MethodId::new(ClassId::new(0), 0);

    let mut receiver = lower_source_to_mir(source);
    let storage = receiver
        .definitions
        .get(receiver.entry_function)
        .unwrap()
        .storage[0]
        .id;
    call_mut(&mut receiver).receiver = Some(
        MirMethodReceiver::exact(
            MirPlace::base(storage),
            ClassId::new(0),
            MirAliasAccess::Mutable,
        )
        .into(),
    );
    assert!(messages(&receiver)
        .iter()
        .any(|message| message == "static method call must not have a receiver"));

    let mut wrong_static_target = lower_source_to_mir(source);
    wrong_static_target.classes.entries_mut_for_test()[0].methods[0].kind =
        MirMethodKind::instance(MirReceiverAccess::ReadOnly);
    assert!(messages(&wrong_static_target)
        .iter()
        .any(|message| message == "static call target is an instance method"));

    let mut wrong_method_target = lower_source_to_mir(source);
    call_mut(&mut wrong_method_target).target =
        MirCallTarget::Method(MirMethodCallTarget::Direct(method));
    assert!(messages(&wrong_method_target)
        .iter()
        .any(|message| message == "receiver-bearing method call target is static"));
}

#[test]
fn initializer_target_identity_and_selected_signature_are_verified() {
    let source = concat!(
        "class Box {\n",
        "  init(value: i64) {}\n",
        "  init(value: bool) {}\n",
        "}\n",
        "fn main() -> i64 { var value: Box = Box(1); return 0; }\n",
    );

    let mut wrong_selection = lower_source_to_mir(source);
    let function = wrong_selection
        .definitions
        .get_mut_for_test(wrong_selection.entry_function)
        .unwrap();
    let initialize = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Initialize(initialize) => Some(initialize),
            _ => None,
        })
        .unwrap();
    initialize.target = InitializerId::new(ClassId::new(0), 1);
    assert!(messages(&wrong_selection)
        .iter()
        .any(|message| message == "initializer argument 0 type mismatch"));

    let mut unknown_target = lower_source_to_mir(source);
    let function = unknown_target
        .definitions
        .get_mut_for_test(unknown_target.entry_function)
        .unwrap();
    let initialize = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Initialize(initialize) => Some(initialize),
            _ => None,
        })
        .unwrap();
    initialize.target = InitializerId::new(ClassId::new(0), 99);
    assert!(messages(&unknown_target)
        .iter()
        .any(|message| message == "initializer target c0:init99 is not declared"));
}
