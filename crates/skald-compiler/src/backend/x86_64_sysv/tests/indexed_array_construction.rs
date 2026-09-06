use super::*;

#[test]
fn executes_every_primitive_indexed_array_in_inline_and_shared_outer_storage() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  var empty: i64[] = i64[](0u; index => index + 100);\n",
        "  var signed: i64[] = i64[](3u; index => index + 1);\n",
        "  var unsigned: u64[] = u64[](3u; index => (u64) index + 2u);\n",
        "  var bytes: u8[] = u8[](3u; index => (u8) index + 2u8);\n",
        "  var floats: f64[] = f64[](3u; index => (f64) index + 0.5);\n",
        "  var flags: bool[] = bool[](3u; index => index == 1);\n",
        "  var shared: shared i64[] = new i64[](3u; index => index + 10);\n",
        "  if (empty.len() != 0u || !flags[1]) { return 99; }\n",
        "  return signed[2] + (i64) unsigned[2] + (i64) bytes[2]\n",
        "    + (i64) floats[2] + shared->[2];\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(25), "{output}");
}

#[test]
fn primitive_indexed_assembly_is_deterministic_and_uses_existing_runtime_symbols() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[](3u; index => index * index);\n",
        "  return values[2];\n",
        "}\n",
    );
    let first = assembly(source);
    let second = assembly(source);

    assert_eq!(first, second);
    assert!(first.contains("call ska_rt_abi_v9"), "{first}");
    assert!(first.contains("call ska_rt_alloc"), "{first}");
    assert!(first.contains("call ska_rt_free"), "{first}");
    assert!(
        !first.contains("indexed"),
        "source protocol must erase before assembly"
    );
}

#[test]
fn zero_length_skips_the_element_expression() {
    let source = concat!(
        "fn explode(index: i64) -> i64 { return 1 / index; }\n",
        "fn main() -> i64 {\n",
        "  var empty: i64[] = i64[](0u; index => explode(index));\n",
        "  return 7 + (i64) empty.len();\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(7), "{output}");
}

#[test]
fn exact_class_indexed_arrays_preserve_placement_copy_and_cleanup() {
    let source = concat!(
        "class Item {\n",
        "  static constructed: i64;\n",
        "  static copied: i64;\n",
        "  static destroyed: i64;\n",
        "  tag: u8;\n",
        "  value: f64;\n",
        "  static fn construct_value(value: i64) -> f64 {\n",
        "    Item.constructed = Item.constructed + 1;\n",
        "    return (f64) value;\n",
        "  }\n",
        "  static fn copy_value(value: f64) -> f64 {\n",
        "    Item.copied = Item.copied + 1;\n",
        "    return value;\n",
        "  }\n",
        "  init(value: i64) {\n",
        "    self.tag = (u8) value;\n",
        "    self.value = Item.construct_value(value);\n",
        "  }\n",
        "  copy(ref source: Item) {\n",
        "    self.tag = source.tag;\n",
        "    self.value = Item.copy_value(source.value);\n",
        "  }\n",
        "  fn read() -> i64 { return (i64) self.value + (i64) self.tag; }\n",
        "  destroy { Item.destroyed = Item.destroyed + 1; }\n",
        "}\n",
        "fn make(index: i64) -> Item { return Item(index + 6); }\n",
        "fn choose(index: i64) -> Item {\n",
        "  if (index == 0) { return Item(11); }\n",
        "  return Item(12);\n",
        "}\n",
        "fn exercise() -> i64 {\n",
        "  var seed: Item = Item(7);\n",
        "  var fresh: Item[] = Item[](3u; index => Item(index + 1));\n",
        "  var copied: Item[] = Item[](2u; index => seed);\n",
        "  var explicit: Item[] = Item[](2u; index => Item(copy seed));\n",
        "  var grouped: Item[] = Item[](2u; index => (Item(index + 4)));\n",
        "  var returned: Item[] = Item[](2u; index => make(index));\n",
        "  var conditional: Item[] = Item[](2u; index => choose(index));\n",
        "  var shared: shared Item[] = new Item[](2u; index => Item(index + 8));\n",
        "  if (Item.constructed != 12 || Item.copied != 6 || Item.destroyed != 2) {\n",
        "    return 1;\n",
        "  }\n",
        "  if (fresh[2].read() != 6 || copied[1].read() != 14 || explicit[1].read() != 14) { return 2; }\n",
        "  if (grouped[1].read() != 10 || returned[1].read() != 14) { return 3; }\n",
        "  if (conditional[1].read() != 24 || shared->[1].read() != 18) { return 4; }\n",
        "  return 0;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  if (exercise() != 0) { return 90; }\n",
        "  if (Item.destroyed != 18) { return 91; }\n",
        "  return 42;\n",
        "}\n",
    );
    let mut output = assembly(source);
    assert_eq!(output, assembly(source));
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
    assert!(
        !output.contains("memcpy"),
        "class slots must not use aggregate copying"
    );
}

#[test]
fn exact_class_indexed_copy_reuses_ancestor_slices_and_checked_sources() {
    let source = concat!(
        "class Base {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Base) { self.value = other.value + 10; }\n",
        "}\n",
        "class Leaf extends Base {\n",
        "  extra: i64;\n",
        "  init(value: i64, extra: i64) { super(value); self.extra = extra; }\n",
        "}\n",
        "fn ancestor(ref source: Leaf) -> i64 {\n",
        "  var values: Base[] = Base[](2u; index => source);\n",
        "  return values[1].value;\n",
        "}\n",
        "fn checked(ref source: Base) -> i64 {\n",
        "  var values: Leaf[] = Leaf[](2u; index => (Leaf) source);\n",
        "  return values[1].value + values[1].extra;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var leaf: Leaf = Leaf(2, 30);\n",
        "  return ancestor(leaf) + checked(leaf);\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(54), "{output}");
}

#[test]
fn class_local_indexed_construction_may_use_a_private_initializer() {
    let source = concat!(
        "class Secret {\n",
        "  value: i64;\n",
        "  private init(value: i64) { self.value = value; }\n",
        "  static fn build() -> Secret[] {\n",
        "    return Secret[](2u; index => Secret(index + 20));\n",
        "  }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "}\n",
        "fn main() -> i64 { return Secret.build()[1].read(); }\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(21), "{output}");
}
