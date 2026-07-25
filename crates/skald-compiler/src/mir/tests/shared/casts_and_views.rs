use super::*;

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
        "  return leaf->tag() + tagged->tag() + root->tag();\n",
        "}\n",
    ))
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
        "  if (*value is Leaf) { return 1; } else { return 0; }\n",
        "}\n",
        "fn relay(value: shared Root) -> i64 { return value->read(); }\n",
        "fn bump(value: shared Leaf) -> i64 { return value->bump(); }\n",
        "fn main() -> i64 {\n",
        "  var leaf: shared Leaf = new Leaf(10, 5);\n",
        "  var root: shared Root = leaf;\n",
        "  var readable: shared Readable = leaf;\n",
        "  var erased: shared Obj = leaf;\n",
        "  var bumped: i64 = bump(leaf);\n",
        "  return bumped + root->read() + readable->read() + relay(leaf) + classify(erased);\n",
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
