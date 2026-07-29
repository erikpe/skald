use super::*;
use crate::identity::VirtualSlotId;

#[test]
fn emits_deterministic_tables_and_indirect_calls() {
    let output = assembly(concat!(
        "class Root {\n",
        "  init() {}\n",
        "  virtual fn value() -> i64 { return 1; }\n",
        "}\n",
        "class Middle extends Root {\n",
        "  init() { super(); }\n",
        "  override fn value() -> i64 { return 2; }\n",
        "}\n",
        "class Leaf extends Middle { init() { super(); } }\n",
        "fn read(ref value: Root) -> i64 { return value.value(); }\n",
        "fn main() -> i64 { var value: Leaf = Leaf(); return read(value); }\n",
    ));

    assert!(output.contains(concat!(
        ".Lska.class.main.Root.c0.dispatch:\n",
        "    .quad .Lska.class.main.Root.c0.method.value.m0\n",
    )));
    assert!(output.contains(concat!(
        ".Lska.class.main.Middle.c1.dispatch:\n",
        "    .quad .Lska.class.main.Middle.c1.method.value.m0\n",
    )));
    assert!(output.contains(concat!(
        ".Lska.class.main.Leaf.c2.dispatch:\n",
        "    .quad .Lska.class.main.Middle.c1.method.value.m0\n",
    )));
    assert!(output.contains("call r11"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn rejects_corrupt_virtual_metadata_before_instruction_selection() {
    let mut program = lower_text(concat!(
        "class Root { init() {} virtual fn value() -> i64 { return 1; } }\n",
        "class Leaf extends Root {\n",
        "  init() { super(); }\n",
        "  override fn value() -> i64 { return 2; }\n",
        "}\n",
        "fn read(ref value: Root) -> i64 { return value.value(); }\n",
        "fn main() -> i64 { var value: Leaf = Leaf(); return read(value); }\n",
    ));
    program.virtual_families.entries_mut_for_test()[0].slot = VirtualSlotId::new(1);

    let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error.message().contains("non-canonical slot"));
}

#[test]
fn rejects_a_virtual_table_selection_without_an_executable_body() {
    let mut program = lower_text(concat!(
        "class Root { init() {} virtual fn value() -> i64 { return 1; } }\n",
        "class Leaf extends Root {\n",
        "  init() { super(); }\n",
        "  override fn value() -> i64 { return 2; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let missing = MethodId::new(ClassId::new(1), 0);
    program.member_definitions.remove_for_test(missing.into());
    verify_mir(&program).unwrap();

    let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
    assert!(error
        .message()
        .contains("virtual table for class c1 selects method c1:method0 without a MIR definition"));
}

#[test]
fn executes_deep_overrides_self_redispatch_and_inherited_selection() {
    let source = concat!(
        "class Root {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  virtual mut fn apply(amount: i64) -> i64 {\n",
        "    self.value = self.value + amount;\n",
        "    return self.value;\n",
        "  }\n",
        "  mut fn relay(amount: i64) -> i64 { return self.apply(amount); }\n",
        "}\n",
        "class Middle extends Root {\n",
        "  init(value: i64) { super(value); }\n",
        "  override mut fn apply(amount: i64) -> i64 {\n",
        "    self.value = self.value + amount + 10;\n",
        "    return self.value;\n",
        "  }\n",
        "}\n",
        "class Inherited extends Middle { init(value: i64) { super(value); } }\n",
        "class Leaf extends Inherited {\n",
        "  init(value: i64) { super(value); }\n",
        "  override mut fn apply(amount: i64) -> i64 {\n",
        "    self.value = self.value + amount + 20;\n",
        "    return self.value;\n",
        "  }\n",
        "}\n",
        "fn through_root(mut ref value: Root, amount: i64) -> i64 {\n",
        "  return value.relay(amount);\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var inherited: Inherited = Inherited(1);\n",
        "  var leaf: Leaf = Leaf(1);\n",
        "  return through_root(inherited, 1) + through_root(leaf, 1);\n",
        "}\n",
    );

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(34));
}

#[test]
fn preserves_metadata_through_recursion_mixed_arguments_and_stack_pressure() {
    let source = concat!(
        "class Root {\n",
        "  init() {}\n",
        "  virtual fn recurse(flag: bool, value: i64, ref a: Root, ref b: Root,\n",
        "      ref c: Root, ref d: Root, ref e: Root, x: f64, y: f64) -> i64 {\n",
        "    if (flag) { return self.recurse(false, value, a, b, c, d, e, x, y); }\n",
        "    else { return value; }\n",
        "  }\n",
        "}\n",
        "class Leaf extends Root {\n",
        "  init() { super(); }\n",
        "  override fn recurse(flag: bool, value: i64, ref a: Root, ref b: Root,\n",
        "      ref c: Root, ref d: Root, ref e: Root, x: f64, y: f64) -> i64 {\n",
        "    if (flag) { return self.recurse(false, value + 7, a, b, c, d, e, x, y); }\n",
        "    else { return value; }\n",
        "  }\n",
        "}\n",
        "fn invoke(ref value: Root, ref a: Root, ref b: Root, ref c: Root,\n",
        "    ref d: Root, ref e: Root) -> i64 {\n",
        "  return value.recurse(true, 30, a, b, c, d, e, 1.0, 2.0);\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var value: Leaf = Leaf();\n",
        "  return invoke(value, value, value, value, value, value);\n",
        "}\n",
    );

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(37));
}

#[test]
fn sliced_base_values_dispatch_as_exact_base_objects() {
    let source = concat!(
        "class Root {\n",
        "  init() {}\n",
        "  virtual fn value() -> i64 { return 7; }\n",
        "}\n",
        "class Leaf extends Root {\n",
        "  init() { super(); }\n",
        "  override fn value() -> i64 { return 41; }\n",
        "}\n",
        "fn read(ref value: Root) -> i64 { return value.value(); }\n",
        "fn main() -> i64 {\n",
        "  var leaf: Leaf = Leaf();\n",
        "  var sliced: Root = leaf;\n",
        "  return read(sliced);\n",
        "}\n",
    );

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(7));
}

#[test]
fn virtual_object_results_use_the_existing_hidden_destination_and_cleanup_path() {
    let source = concat!(
        "class Result {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "}\n",
        "class Root {\n",
        "  init() {}\n",
        "  virtual fn make(value: i64) -> Result { return Result(value); }\n",
        "}\n",
        "class Leaf extends Root {\n",
        "  init() { super(); }\n",
        "  override fn make(value: i64) -> Result { return Result(value + 9); }\n",
        "}\n",
        "fn invoke(ref source: Root) -> Result { return source.make(30); }\n",
        "fn main() -> i64 {\n",
        "  var source: Leaf = Leaf();\n",
        "  var result: Result = invoke(source);\n",
        "  return result.value;\n",
        "}\n",
    );

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(39));
}

#[test]
fn nested_copy_sources_start_a_new_exact_dynamic_origin() {
    let source = concat!(
        "class Root {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Root) { self.value = other.value; }\n",
        "  virtual fn read() -> i64 { return self.value; }\n",
        "}\n",
        "class Leaf extends Root {\n",
        "  observed: i64;\n",
        "  init(value: i64) { super(value); self.observed = 0; }\n",
        "  copy(ref other: Leaf) { self.observed = other.read(); }\n",
        "  override fn read() -> i64 { return self.value + 10; }\n",
        "}\n",
        "class Container {\n",
        "  child: Leaf;\n",
        "  init(value: i64) { self.child = Leaf(value); }\n",
        "  copy(ref other: Container) { self.child = other.child; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var source: Container = Container(5);\n",
        "  var copied: Container = source;\n",
        "  return copied.child.observed;\n",
        "}\n",
    );

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(15));
}
