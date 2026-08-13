use super::*;

#[test]
fn closed_specializations_execute_with_substituted_calling_conventions_and_cleanup() {
    let source = concat!(
        "class Tracked {\n",
        "  static destroyed: i64;\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Tracked) { self.value = source.value; }\n",
        "  assign(ref source: Tracked) { self.value = source.value; }\n",
        "  destroy { Tracked.destroyed = Tracked.destroyed + 1; }\n",
        "}\n",
        "class Box<T> {\n",
        "  value: T;\n",
        "  init(value: T) { self.value = value; }\n",
        "  fn seventh(a: T, b: T, c: T, d: T, e: T, f: T, g: T) -> T { return g; }\n",
        "  fn get() -> T { return self.value; }\n",
        "}\n",
        "fn exercise_cleanup() -> unit {\n",
        "  var tracked: Tracked = Tracked(5);\n",
        "  var box: Box<Tracked> = Box<Tracked>(tracked);\n",
        "  var result: Tracked = box.seventh(tracked, tracked, tracked, tracked, tracked, tracked, tracked);\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var integers: Box<i64> = Box<i64>(1);\n",
        "  var integer: i64 = integers.seventh(1, 2, 3, 4, 5, 6, 37);\n",
        "  var floats: Box<f64> = Box<f64>(1.0);\n",
        "  var floating: f64 = floats.seventh(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 2.5);\n",
        "  exercise_cleanup();\n",
        "  if (floating == 2.5) {\n",
        "    if (Tracked.destroyed > 0) { return integer + 5; }\n",
        "  }\n",
        "  return 1;\n",
        "}\n",
    );
    let mut assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    assembly.push_str(native_allocator());

    assert_system_assembler_accepts(&assembly);
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
    assert!(!assembly.contains("memcpy"));
}

#[test]
fn optional_array_shared_bound_inheritance_and_static_behavior_execute_natively() {
    let source = concat!(
        "interface Ranked { fn rank() -> i64; }\n",
        "class Item implements Ranked {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Item) { self.value = source.value; }\n",
        "  assign(ref source: Item) { self.value = source.value; }\n",
        "  fn rank() -> i64 { return self.value; }\n",
        "}\n",
        "class Store<T> {\n",
        "  static last: T?;\n",
        "  values: T?[];\n",
        "  init(length: u64) { self.values = T?[](length); }\n",
        "  mut fn put(index: i64, value: T?) -> unit { self.values[index] = value; Store<T>::last = value; }\n",
        "}\n",
        "class Reader<T> where T: Ranked {\n",
        "  value: T;\n",
        "  init(value: T) { self.value = value; }\n",
        "  virtual fn read() -> i64 { return self.value.rank(); }\n",
        "}\n",
        "class LoudReader<T> extends Reader<T> where T: Ranked {\n",
        "  init(value: T) { super(value); }\n",
        "  override fn read() -> i64 { return 41; }\n",
        "}\n",
        "fn through_base(ref reader: Reader<Item>) -> i64 { return reader.read(); }\n",
        "fn main() -> i64 {\n",
        "  var owner: shared Item = new Item(40);\n",
        "  var store: Store<shared Item> = Store<shared Item>(2u);\n",
        "  store.put(0, owner);\n",
        "  var missing: (shared Item)? = store.values[1];\n",
        "  if (missing is some) { return 1; }\n",
        "  if (Store<shared Item>::last is none) { return 2; }\n",
        "  var item: Item = Item(40);\n",
        "  var reader: LoudReader<Item> = LoudReader<Item>(item);\n",
        "  return through_base(reader) + Store<shared Item>::last!->rank() - 39;\n",
        "}\n",
    );
    let mut assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    assembly.push_str(native_allocator());

    assert_system_assembler_accepts(&assembly);
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn equal_layout_specializations_emit_distinct_deterministic_private_artifacts() {
    let source = concat!(
        "class Box<T> { value: T; init(value: T) { self.value = value; } fn get() -> T { return self.value; } }\n",
        "fn main() -> i64 {\n",
        "  var signed: Box<i64> = Box<i64>(20);\n",
        "  var unsigned: Box<u64> = Box<u64>(22u);\n",
        "  return signed.get() + (i64) unsigned.get();\n",
        "}\n",
    );
    let program = lower_source_to_final_mir(source);
    let classes = program
        .classes
        .iter()
        .filter(|class| class.name.starts_with("Box<"))
        .map(|class| class.id)
        .collect::<Vec<_>>();
    let layouts = super::super::layout::DataLayout::compute(&program).unwrap();
    assert_eq!(classes.len(), 2);
    assert_ne!(classes[0], classes[1]);
    assert_eq!(
        layouts.ty(MirType::Class(classes[0])).unwrap(),
        layouts.ty(MirType::Class(classes[1])).unwrap()
    );

    let first = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    let second = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();

    assert_eq!(first, second);
    assert!(first.contains("class.main.Box_x3c_i64_x3e_"), "{first}");
    assert!(first.contains("class.main.Box_x3c_u64_x3e_"), "{first}");
    assert_eq!(first.matches("call ska_rt_abi_v9").count(), 1);
    assert!(!first.contains("ClassTemplate"));
    assert_system_assembler_accepts(&first);
    assert_eq!(run_native_assembly(&first).code(), Some(42));
}

#[test]
fn nested_optional_array_elements_execute_through_generic_storage() {
    let source = concat!(
        "class Store<T> {\n",
        "  values: T?[];\n",
        "  init() { self.values = T?[](1u); }\n",
        "  mut fn put(value: T) -> unit { self.values[0] = value; }\n",
        "  fn get() -> T { return self.values[0]!; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var store: Store<i64[]?> = Store<i64[]?>();\n",
        "  var row: i64[]? = some(i64[]{40, 2});\n",
        "  store.put(row);\n",
        "  row = none;\n",
        "  var recovered_optional: i64[]? = store.get();\n",
        "  var recovered: i64[] = recovered_optional!;\n",
        "  return recovered[0] + recovered[1];\n",
        "}\n",
    );
    let mut assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    assembly.push_str(native_allocator());

    assert_system_assembler_accepts(&assembly);
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}
