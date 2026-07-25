use super::*;

fn shared_anchor_program() -> MirProgram {
    lower_text(concat!(
        "class Leaf { init() {} fn read() -> i64 { return 7; } }\n",
        "class Holder { edge: shared Leaf; init() { self.edge = new Leaf(); } }\n",
        "fn main() -> i64 {\n",
        "  var holder: Holder = Holder();\n",
        "  return holder.edge.read();\n",
        "}\n",
    ))
}

fn shared_checked_place_program() -> MirProgram {
    lower_text(concat!(
        "class Leaf { init() {} fn read() -> i64 { return 7; } }\n",
        "class Holder { edge: shared Obj; init() { self.edge = new Leaf(); } }\n",
        "fn main() -> i64 {\n",
        "  var holder: Holder = Holder();\n",
        "  return ((Leaf) holder.edge).read();\n",
        "}\n",
    ))
}

#[test]
fn verifier_rejects_forged_or_ended_shared_call_anchors() {
    let mut forged = shared_anchor_program();
    let definition = forged
        .definitions
        .get_mut_for_test(forged.entry_function)
        .unwrap();
    let anchor = definition
        .storage
        .iter_mut()
        .find(|storage| storage.kind == MirStorageKind::SharedAnchor)
        .unwrap();
    anchor.kind = MirStorageKind::Temporary;
    assert!(has_error(
        &forged,
        "shared origin requires a stable or call-anchor owner"
    ));

    let mut ended = shared_anchor_program();
    let instructions = main_instructions_mut(&mut ended);
    let release = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::SharedRelease(release)
                    if release.owner
                        == instructions.iter().find_map(|candidate| match candidate {
                            MirInstruction::SharedFieldCopy(copy) => Some(copy.destination),
                            _ => None,
                        }).unwrap()
            )
        })
        .unwrap();
    let release = instructions.remove(release);
    let call = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Call(_)))
        .unwrap();
    instructions.insert(call, release);
    assert!(has_error(
        &ended,
        "shared pointee is used without a live owner"
    ));
}

#[test]
fn verifier_rejects_releasing_a_shared_anchor_before_its_checked_view_ends() {
    let mut program = shared_checked_place_program();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let block = definition
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::EndCheckedView(_)))
                && block.instructions.iter().any(|instruction| {
                    matches!(
                        instruction,
                        MirInstruction::SharedRelease(release)
                            if definition.storage[release.owner.index()].kind
                                == MirStorageKind::SharedAnchor
                    )
                })
        })
        .expect("cast success block must end its checked view and anchor");
    let end = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndCheckedView(_)))
        .unwrap();
    let release = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
        .unwrap();
    block.instructions.swap(end, release);

    assert!(has_error(
        &program,
        "shared owner is released before its checked view ends"
    ));

    let mut forged_exact = lower_text(concat!(
        "class Leaf { init() {} fn read() -> i64 { return 7; } }\n",
        "class Other { init() {} }\n",
        "fn main() -> i64 { return ((Leaf) new Leaf()).read(); }\n",
    ));
    let definition = forged_exact
        .definitions
        .get_mut_for_test(forged_exact.entry_function)
        .unwrap();
    let origin = definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::BindCheckedView(binding) => Some(binding.view.origin.as_mut()),
            _ => None,
        })
        .expect("static shared-backed cast must bind a checked view");
    let MirObjectOrigin::Shared {
        exact_dynamic_class,
        ..
    } = origin
    else {
        panic!("produced shared allocation must retain shared origin");
    };
    *exact_dynamic_class = Some(ClassId::new(1));
    assert!(has_error(
        &forged_exact,
        "shared origin has incompatible exact dynamic provenance"
    ));
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
        "shared origin requires a stable or call-anchor owner with the declared static target"
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
