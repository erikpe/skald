use super::*;
use crate::{
    backend::{emit_assembly, Target},
    passes::run_mir_pipeline,
};

fn exact_owner_program() -> MirProgram {
    lower_text(concat!(
        "class Widget { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn main() -> i64 {\n",
        "  var value: shared Widget = new Widget(7);\n",
        "  return 0;\n",
        "}\n",
    ))
}

fn shared_cast_program() -> MirProgram {
    lower_text(concat!(
        "interface Tagged { fn tag() -> i64; }\n",
        "class Root { init() {} virtual fn tag() -> i64 { return 1; } }\n",
        "class Leaf extends Root implements Tagged {\n",
        "  init() { super(); }\n",
        "  override fn tag() -> i64 { return 2; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var erased: shared Obj = new Leaf();\n",
        "  var leaf: shared Leaf = (shared Leaf) erased;\n",
        "  var tagged: shared Tagged = (shared Tagged) leaf;\n",
        "  var root: shared Root = (shared Root) new Leaf();\n",
        "  return leaf.tag() + tagged.tag() + root.tag();\n",
        "}\n",
    ))
}

fn main_instructions(program: &MirProgram) -> &[MirInstruction] {
    &program
        .definitions
        .get(program.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .instructions
}

fn main_instructions_mut(program: &mut MirProgram) -> &mut Vec<MirInstruction> {
    &mut program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .instructions
}

fn has_error(program: &MirProgram, needle: &str) -> bool {
    verify_mir(program)
        .unwrap_err()
        .iter()
        .any(|error| error.message.contains(needle))
}

#[test]
fn lowers_and_verifies_the_first_exact_shared_owner_lifetime() {
    let program = exact_owner_program();
    verify_mir(&program).expect("lowered shared ownership MIR must verify");
    let instructions = main_instructions(&program);
    let allocation = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedAllocate(_)))
        .unwrap();
    assert!(allocation > 0);
    assert!(instructions[..allocation]
        .iter()
        .any(|instruction| matches!(instruction, MirInstruction::Assign(_))));
    assert!(matches!(
        &instructions[allocation..allocation + 5],
        [
            MirInstruction::SharedAllocate(_),
            MirInstruction::SharedInitialize(_),
            MirInstruction::SharedPublish(_),
            MirInstruction::SharedAdopt(_),
            MirInstruction::EndFullExpression(_),
        ]
    ));
    assert!(instructions
        .iter()
        .skip(allocation + 5)
        .any(|instruction| matches!(instruction, MirInstruction::SharedRelease(_))));
    run_mir_pipeline(program.clone()).expect("shared MIR must survive target-independent passes");
    let assembly = emit_assembly(Target::X86_64SysV, &program)
        .expect("the exact shared lifetime must reach the native backend");
    assert!(assembly.contains("call ska_rt_alloc"));
    assert!(assembly.contains("call ska_rt_free"));
}

#[test]
fn shared_lifetime_dump_is_exact_and_deterministic() {
    let dump = dump_mir(&exact_owner_program());
    assert_eq!(dump, dump_mir(&exact_owner_program()));
    assert!(dump.contains("shared class c0"));
    assert!(dump.contains("shared-allocation"));
    assert!(dump.contains("shared-allocate"));
    assert!(dump.contains("shared-initialize"));
    assert!(dump.contains("shared-publish"));
    assert!(dump.contains("shared-adopt"));
    assert!(dump.contains("end-full-expression"));
    assert!(dump.contains("shared-release"));
}

#[test]
fn lowers_static_and_runtime_shared_casts_without_allocating_for_the_cast() {
    let program = shared_cast_program();
    verify_mir(&program).expect("shared casts must produce verified owner control flow");
    let dump = dump_mir(&program);
    assert!(dump.contains("shared-cast-runtime"));
    assert!(dump.contains("shared-cast-static"));
    assert!(dump.contains("copy"));
    assert!(dump.contains("adopt"));

    let assembly =
        emit_assembly(Target::X86_64SysV, &program).expect("shared casts must reach the backend");
    assert_eq!(assembly.matches("call ska_rt_alloc").count(), 2);
    assert!(assembly.contains("_cast_"));
}

#[test]
fn rejects_corrupt_shared_cast_provenance_target_and_failure_flow() {
    let program = shared_cast_program();

    let mut wrong_transfer = program.clone();
    let runtime = wrong_transfer
        .definitions
        .get_mut_for_test(wrong_transfer.entry_function)
        .unwrap()
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match block.terminator.as_mut() {
            Some(MirTerminator::SharedCast { cast, .. }) => Some(cast),
            _ => None,
        })
        .unwrap();
    runtime.transfer = MirSharedCastTransfer::Adopt;
    assert!(has_error(
        &wrong_transfer,
        "source provenance or copy/adopt operation is invalid"
    ));

    let mut forged_exact = program.clone();
    let runtime = forged_exact
        .definitions
        .get_mut_for_test(forged_exact.entry_function)
        .unwrap()
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match block.terminator.as_mut() {
            Some(MirTerminator::SharedCast { cast, .. }) => Some(cast),
            _ => None,
        })
        .unwrap();
    runtime.exact_dynamic_class = Some(ClassId::new(0));
    assert!(has_error(
        &forged_exact,
        "source provenance or copy/adopt operation is invalid"
    ));
    assert!(has_error(
        &forged_exact,
        "exact dynamic provenance does not match its allocation"
    ));

    let mut wrong_target = program.clone();
    let runtime = wrong_target
        .definitions
        .get_mut_for_test(wrong_target.entry_function)
        .unwrap()
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match block.terminator.as_mut() {
            Some(MirTerminator::SharedCast { cast, .. }) => Some(cast),
            _ => None,
        })
        .unwrap();
    runtime.target = MirSharedTarget::Obj;
    assert!(has_error(
        &wrong_target,
        "matching fresh owner destination storage"
    ));
    assert!(has_error(
        &wrong_target,
        "does not require a runtime metadata check"
    ));

    let mut wrong_failure = program;
    let function = wrong_failure
        .definitions
        .get_mut_for_test(wrong_failure.entry_function)
        .unwrap();
    let Some(MirTerminator::SharedCast {
        success_target,
        failure_target,
        ..
    }) = function
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match block.terminator.as_mut() {
            Some(terminator @ MirTerminator::SharedCast { .. }) => Some(terminator),
            _ => None,
        })
    else {
        panic!("expected runtime shared cast");
    };
    *failure_target = *success_target;
    assert!(has_error(
        &wrong_failure,
        "shared cast success and failure edges must differ"
    ));
}

#[test]
fn shared_field_cast_checks_before_copying_the_field_owner() {
    let program = lower_text(concat!(
        "class Root { init() {} }\n",
        "class Leaf extends Root { init() { super(); } }\n",
        "class Holder {\n",
        "  value: shared Obj;\n",
        "  init(value: shared Obj) { self.value = value; }\n",
        "  fn leaf() -> shared Leaf { return (shared Leaf) self.value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_mir(&program).expect("shared field casts must verify");
    let holder_leaf = program
        .member_definitions
        .get(MethodId::new(ClassId::new(2), 0).into())
        .expect("holder leaf method");
    let cast = holder_leaf
        .body
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            Some(MirTerminator::SharedCast { cast, .. }) => Some(cast),
            _ => None,
        })
        .expect("field downcast must require runtime metadata");
    assert!(matches!(cast.source, MirSharedCastSource::Field { .. }));
    assert_eq!(cast.transfer, MirSharedCastTransfer::Copy);
}

#[test]
fn lowers_local_copy_and_secure_release_move_assignment_explicitly() {
    let program = lower_text(concat!(
        "class Item { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var source: shared Item = new Item();\n",
        "  var destination: shared Item = source;\n",
        "  destination = destination;\n",
        "  destination = new Item();\n",
        "  return 0;\n",
        "}\n",
    ));
    verify_mir(&program).expect("local owner operations must verify");
    let instructions = main_instructions(&program);
    assert!(instructions.windows(2).any(|window| matches!(
        window,
        [
            MirInstruction::SharedCopy(_),
            MirInstruction::EndFullExpression(_)
        ]
    )));
    assert!(instructions.windows(4).any(|window| matches!(
        window,
        [
            MirInstruction::SharedCopy(_),
            MirInstruction::SharedRelease(_),
            MirInstruction::SharedMove(_),
            MirInstruction::EndFullExpression(_),
        ]
    )));
    assert!(instructions.windows(7).any(|window| matches!(
        window,
        [
            MirInstruction::SharedAllocate(_),
            MirInstruction::SharedInitialize(_),
            MirInstruction::SharedPublish(_),
            MirInstruction::SharedAdopt(_),
            MirInstruction::SharedRelease(_),
            MirInstruction::SharedMove(_),
            MirInstruction::EndFullExpression(_),
        ]
    )));

    let dump = dump_mir(&program);
    assert!(dump.contains("temporary <temporary>"));
    assert!(dump.contains(": shared class c0"));
    assert!(dump.contains("shared-copy"));
    assert!(dump.contains("shared-release"));
    assert!(dump.contains("shared-move"));
}

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
fn shared_upviews_retain_header_provenance_for_members_dispatch_and_type_tests() {
    let program = lower_text(concat!(
        "interface Readable { fn read() -> i64; }\n",
        "class Root implements Readable {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  virtual fn read() -> i64 { return self.value; }\n",
        "}\n",
        "class Middle extends Root { init(value: i64) { super(value); } }\n",
        "class Leaf extends Middle {\n",
        "  extra: i64;\n",
        "  init(value: i64, extra: i64) { super(value); self.extra = extra; }\n",
        "  override fn read() -> i64 { return self.value + self.extra; }\n",
        "  mut fn bump() -> i64 { self.value = self.value + 1; return self.value; }\n",
        "}\n",
        "fn classify(value: shared Obj) -> i64 {\n",
        "  if (value is Leaf) { return 1; } else { return 0; }\n",
        "}\n",
        "fn relay(value: shared Root) -> i64 { return value.read(); }\n",
        "fn bump(value: shared Leaf) -> i64 { return value.bump(); }\n",
        "fn main() -> i64 {\n",
        "  var leaf: shared Leaf = new Leaf(10, 5);\n",
        "  var root: shared Root = leaf;\n",
        "  var readable: shared Readable = leaf;\n",
        "  var erased: shared Obj = leaf;\n",
        "  var bumped: i64 = bump(leaf);\n",
        "  return bumped + root.read() + readable.read() + relay(leaf) + classify(erased);\n",
        "}\n",
    ));
    verify_mir(&program).expect("shared polymorphic views must verify");
    let dump = dump_mir(&program);
    assert!(dump.contains("shared-pointee("));
    assert!(dump.contains("origin shared("));
    assert!(dump.contains("virtual "));
    assert!(dump.contains("interface "));
    assert!(dump.contains("type-test view(shared-pointee("));

    let assembly = emit_assembly(Target::X86_64SysV, &program)
        .expect("shared polymorphic views must reach the backend");
    assert!(assembly.contains(" + 16]"));
    assert!(assembly.contains(" + 8]"));
}

#[test]
fn rejects_corrupt_shared_pointee_origin_and_dead_owner_use() {
    let program = lower_text(concat!(
        "class Root {\n",
        "  init() {}\n",
        "  virtual fn read() -> i64 { return 1; }\n",
        "}\n",
        "class Leaf extends Root {\n",
        "  init() { super(); }\n",
        "  override fn read() -> i64 { return 2; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var value: shared Root = new Leaf();\n",
        "  return value.read();\n",
        "}\n",
    ));

    let mut wrong_origin = program.clone();
    let call = main_instructions_mut(&mut wrong_origin)
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if call.receiver.is_some() => Some(call),
            _ => None,
        })
        .unwrap();
    let receiver = call.receiver.as_mut().unwrap().as_method_mut().unwrap();
    let MirObjectOrigin::Shared { static_target, .. } = receiver.origin.as_mut() else {
        panic!("shared receiver must retain a shared origin");
    };
    *static_target = MirViewTarget::Obj;
    assert!(has_error(
        &wrong_origin,
        "shared origin requires a stable owner with the declared static target"
    ));

    let mut dead_owner = program;
    let instructions = main_instructions_mut(&mut dead_owner);
    let call_index = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Call(call) if call.receiver.is_some()))
        .unwrap();
    let owner = match &instructions[call_index] {
        MirInstruction::Call(call) => {
            let receiver = call.receiver.as_ref().unwrap().as_method().unwrap();
            let MirObjectOrigin::Shared { owner, .. } = receiver.origin.as_ref() else {
                panic!("shared receiver must retain a shared owner");
            };
            *owner
        }
        _ => unreachable!(),
    };
    instructions.insert(
        call_index,
        MirInstruction::SharedRelease(MirSharedRelease {
            owner,
            span: instructions[call_index].span(),
        }),
    );
    assert!(has_error(
        &dead_owner,
        "shared pointee is used without a live owner"
    ));
    assert!(has_error(
        &dead_owner,
        "shared object origin is used without a live owner"
    ));
}

#[test]
fn lowers_shared_fields_as_owner_edges_in_lifecycle_order() {
    let program = lower_text(concat!(
        "class Item { init() {} }\n",
        "class Inline {\n",
        "  edge: shared Item;\n",
        "  init(edge: shared Item) { self.edge = edge; }\n",
        "}\n",
        "class Holder {\n",
        "  left: shared Item;\n",
        "  middle: Inline;\n",
        "  right: shared Item;\n",
        "  init(value: shared Item) {\n",
        "    self.left = value;\n",
        "    self.middle = Inline(new Item());\n",
        "    self.right = new Item();\n",
        "  }\n",
        "  mut fn replace() -> unit {\n",
        "    self.middle.edge = self.right;\n",
        "    self.left = self.right;\n",
        "  }\n",
        "  fn snapshot() -> shared Item { return self.left; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_mir(&program).expect("shared field lifecycle MIR must verify");

    let holder = program.class(ClassId::new(2)).unwrap();
    let MirCopyCapability::Synthesized(construction) = &holder.copy_constructor else {
        panic!("shared fields must retain synthesized copy construction");
    };
    assert!(matches!(
        construction.fields.as_slice(),
        [
            MirSynthesizedFieldCopy::Shared { .. },
            MirSynthesizedFieldCopy::Class { .. },
            MirSynthesizedFieldCopy::Shared { .. },
        ]
    ));
    assert!(matches!(
        holder.destruction.steps.as_slice(),
        [
            MirDestructionStep::SharedField(_),
            MirDestructionStep::Field(_),
            MirDestructionStep::SharedField(_),
        ]
    ));

    let dump = dump_mir(&program);
    assert!(dump.contains("shared-field-initialize"));
    assert!(dump.contains("shared-field-replace"));
    assert!(dump.contains("shared-field-copy"));
    assert!(dump.contains("Shared c2:field0"));
    assert!(dump.contains("SharedField c2:field2"));

    let assembly =
        emit_assembly(Target::X86_64SysV, &program).expect("verified shared fields must execute");
    assert!(assembly.contains("ownership_field_replace"));
    assert!(assembly.contains("field_2_2_release"));
}

#[test]
fn rejects_corrupt_shared_field_initialization_and_lifecycle_metadata() {
    let source = concat!(
        "class Item { init() {} }\n",
        "class Holder {\n",
        "  first: shared Item;\n",
        "  second: shared Item;\n",
        "  init() { self.first = new Item(); self.second = new Item(); }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let program = lower_text(source);
    let initializer = InitializerId::new(ClassId::new(1), 0);

    let mut missing = program.clone();
    let body = &mut missing
        .member_definitions
        .get_mut_for_test(initializer.into())
        .unwrap()
        .body;
    let remove = body.blocks[0]
        .instructions
        .iter()
        .rposition(|instruction| matches!(instruction, MirInstruction::SharedFieldInitialize(_)))
        .unwrap();
    body.blocks[0].instructions.remove(remove);
    assert!(has_error(
        &missing,
        "shared receiver fields are not initialized exactly once"
    ));

    let mut duplicate = program.clone();
    let body = &mut duplicate
        .member_definitions
        .get_mut_for_test(initializer.into())
        .unwrap()
        .body;
    let initialize = body.blocks[0]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::SharedFieldInitialize(initialize) => Some(initialize.clone()),
            _ => None,
        })
        .unwrap();
    let boundary = body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndFullExpression(_)))
        .unwrap();
    body.blocks[0]
        .instructions
        .insert(boundary, MirInstruction::SharedFieldInitialize(initialize));
    assert!(has_error(
        &duplicate,
        "shared field transfer source is not a live owner"
    ));
    assert!(has_error(
        &duplicate,
        "shared field is initialized more than once"
    ));

    let mut wrong_plan = program;
    let holder = &mut wrong_plan.classes.entries_mut_for_test()[1];
    holder.destruction.steps[0] = MirDestructionStep::Field(FieldId::new(holder.id, 1));
    assert!(verify_mir(&wrong_plan)
        .unwrap_err()
        .iter()
        .any(|error| error
            .message
            .contains("owning fields in reverse declaration order")));
}

#[test]
fn preserves_user_shared_field_lifecycle_across_inheritance() {
    let program = lower_text(concat!(
        "class Item { init() {} }\n",
        "class Base {\n",
        "  edge: shared Item;\n",
        "  init(edge: shared Item) { self.edge = edge; }\n",
        "  copy(ref source: Base) { self.edge = source.edge; }\n",
        "  assign(ref source: Base) { self.edge = source.edge; }\n",
        "}\n",
        "class Derived extends Base {\n",
        "  extra: shared Item;\n",
        "  init(edge: shared Item, extra: shared Item) {\n",
        "    super(edge); self.extra = extra;\n",
        "  }\n",
        "  copy(ref source: Derived) { self.extra = source.extra; }\n",
        "  assign(ref source: Derived) { self.extra = source.extra; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_mir(&program).expect("inherited shared lifecycle must verify");

    let derived = program.class(ClassId::new(2)).unwrap();
    assert!(matches!(
        derived.copy_constructor,
        MirCopyCapability::User(_)
    ));
    assert!(matches!(
        derived.copy_assignment,
        MirCopyCapability::User(_)
    ));
    assert!(matches!(
        derived.destruction.steps.as_slice(),
        [
            MirDestructionStep::SharedField(_),
            MirDestructionStep::Base(_)
        ]
    ));
    let dump = dump_mir(&program);
    assert!(dump.contains("shared-field-initialize"));
    assert!(dump.contains("shared-field-replace"));
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

#[test]
fn rejects_move_before_release_and_live_full_expression_temporaries() {
    let program = lower_text(concat!(
        "class Item { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var source: shared Item = new Item();\n",
        "  var destination: shared Item = source;\n",
        "  destination = source;\n",
        "  return 0;\n",
        "}\n",
    ));

    let mut early_move = program.clone();
    let instructions = main_instructions_mut(&mut early_move);
    let release = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
        .unwrap();
    let transfer = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedMove(_)))
        .unwrap();
    instructions.swap(release, transfer);
    assert!(has_error(&early_move, "destination is still live"));

    let mut live_temporary = program;
    main_instructions_mut(&mut live_temporary)
        .retain(|instruction| !matches!(instruction, MirInstruction::SharedMove(_)));
    assert!(has_error(
        &live_temporary,
        "temporary remains live at full-expression boundary"
    ));
}

#[test]
fn fully_released_branch_local_owner_does_not_escape_to_the_join() {
    let program = lower_text(concat!(
        "class Widget { init() {} }\n",
        "fn main() -> i64 {\n",
        "  if (true) { var value: shared Widget = new Widget(); }\n",
        "  return 0;\n",
        "}\n",
    ));
    verify_mir(&program).expect("a completed branch-local lifetime must join cleanly");
}

#[test]
fn rejects_duplicate_adoption_and_release_before_publication() {
    let mut duplicate = exact_owner_program();
    let adopt = main_instructions(&duplicate)
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::SharedAdopt(adopt) => Some(adopt.clone()),
            _ => None,
        })
        .unwrap();
    let index = main_instructions(&duplicate)
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndFullExpression(_)))
        .unwrap();
    main_instructions_mut(&mut duplicate).insert(index, MirInstruction::SharedAdopt(adopt));
    assert!(has_error(
        &duplicate,
        "requires one published produced owner"
    ));

    let mut early = exact_owner_program();
    let instructions = main_instructions_mut(&mut early);
    let publish = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedPublish(_)))
        .unwrap();
    let adopt = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedAdopt(_)))
        .unwrap();
    instructions.swap(publish, adopt);
    assert!(has_error(
        &early,
        "adoption requires one published produced owner"
    ));
}

#[test]
fn rejects_missing_and_duplicate_release() {
    let mut missing = exact_owner_program();
    main_instructions_mut(&mut missing)
        .retain(|instruction| !matches!(instruction, MirInstruction::SharedRelease(_)));
    assert!(has_error(&missing, "remains live on normal return"));

    let mut duplicate = exact_owner_program();
    let release = main_instructions(&duplicate)
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::SharedRelease(release) => Some(release.clone()),
            _ => None,
        })
        .unwrap();
    let index = main_instructions(&duplicate)
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
        .unwrap();
    main_instructions_mut(&mut duplicate).insert(index + 1, MirInstruction::SharedRelease(release));
    assert!(has_error(&duplicate, "released without being live"));
}

#[test]
fn rejects_wrong_target_and_non_new_allocation() {
    let mut wrong_target = exact_owner_program();
    let owner = wrong_target
        .definitions
        .get_mut_for_test(wrong_target.entry_function)
        .unwrap()
        .storage
        .iter_mut()
        .find(|storage| matches!(storage.ty, MirType::Shared(_)))
        .unwrap();
    owner.ty = MirType::Shared(MirSharedTarget::Class(ClassId::new(99)));
    assert!(has_error(
        &wrong_target,
        "requires a compatible destination owner target"
    ));

    let mut non_new = exact_owner_program();
    let allocation = main_instructions_mut(&mut non_new)
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::SharedAllocate(allocation) => Some(allocation),
            _ => None,
        })
        .unwrap();
    allocation.origin = MirSharedAllocationOrigin::Unspecified;
    assert!(has_error(&non_new, "does not originate from `new`"));
}

#[test]
fn rejects_use_after_release_and_different_join_states() {
    let mut use_after_release = exact_owner_program();
    let owner = use_after_release
        .definitions
        .get(use_after_release.entry_function)
        .unwrap()
        .storage
        .iter()
        .find(|storage| matches!(storage.ty, MirType::Shared(_)))
        .unwrap()
        .id;
    let release_index = main_instructions(&use_after_release)
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
        .unwrap();
    let span = use_after_release.span;
    main_instructions_mut(&mut use_after_release).insert(
        release_index + 1,
        MirInstruction::SharedCopy(MirSharedCopy {
            destination: owner,
            source: owner,
            span,
        }),
    );
    assert!(has_error(
        &use_after_release,
        "copy source is not a live owner"
    ));

    let mut join = exact_owner_program();
    let function = join
        .definitions
        .get_mut_for_test(join.entry_function)
        .unwrap();
    let span = function.span;
    let original = function.body.blocks.pop().unwrap();
    let split = original
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
        .unwrap();
    let mut before_release = original.instructions;
    let after_release = before_release.split_off(split);
    let condition = ValueId::new(function.function, function.values.len());
    function
        .values
        .push(fixture_value(condition, MirType::Bool, span));
    let entry = BlockId::new(function.function, 0);
    let released = BlockId::new(function.function, 1);
    let live = BlockId::new(function.function, 2);
    let exit = BlockId::new(function.function, 3);
    before_release.push(fixture_assign(
        condition,
        MirRvalueKind::ConstantBool(true),
        MirType::Bool,
        span,
    ));
    function.body.blocks = vec![
        fixture_block(
            entry,
            before_release,
            Some(MirTerminator::Branch {
                condition,
                true_target: released,
                false_target: live,
                span,
            }),
            span,
        ),
        fixture_block(
            released,
            after_release,
            Some(MirTerminator::Goto { target: exit, span }),
            span,
        ),
        fixture_block(
            live,
            vec![],
            Some(MirTerminator::Goto { target: exit, span }),
            span,
        ),
        fixture_block(exit, vec![], original.terminator, span),
    ];
    assert!(has_error(&join, "state differs across control-flow paths"));
}
