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

#[test]
fn optional_indexed_arrays_preserve_presence_payload_copy_and_cleanup() {
    let source = concat!(
        "class Item {\n",
        "  static constructed: i64;\n",
        "  static copied: i64;\n",
        "  static destroyed: i64;\n",
        "  value: i64;\n",
        "  static fn construct_value(value: i64) -> i64 {\n",
        "    Item.constructed = Item.constructed + 1; return value;\n",
        "  }\n",
        "  static fn copy_value(value: i64) -> i64 {\n",
        "    Item.copied = Item.copied + 1; return value + 10;\n",
        "  }\n",
        "  init(value: i64) { self.value = Item.construct_value(value); }\n",
        "  copy(ref source: Item) { self.value = Item.copy_value(source.value); }\n",
        "  destroy { Item.destroyed = Item.destroyed + 1; }\n",
        "}\n",
        "fn maybe_number(index: i64) -> i64? {\n",
        "  if (index == 1) { return none; }\n",
        "  return index + 20;\n",
        "}\n",
        "fn maybe_item(index: i64) -> Item? {\n",
        "  if (index == 0) { return none; }\n",
        "  return Item(index + 4);\n",
        "}\n",
        "fn exercise() -> i64 {\n",
        "  var seed: Item? = Item(7);\n",
        "  var numbers: i64?[] = i64?[](3u; index => maybe_number(index));\n",
        "  var shared_numbers: shared i64?[] = new i64?[](2u; index => index);\n",
        "  var absent: Item?[] = Item?[](2u; index => none);\n",
        "  var fresh: Item?[] = Item?[](2u; index => Item(index + 1));\n",
        "  var copied: Item?[] = Item?[](2u; index => seed);\n",
        "  var grouped: Item?[] = Item?[](2u; index => (Item(index + 3)));\n",
        "  var conditional: Item?[] = Item?[](2u; index => maybe_item(index));\n",
        "  if (numbers[0]! != 20 || numbers[1] is some || numbers[2]! != 22) { return 1; }\n",
        "  if (shared_numbers->[1]! != 1 || absent[0] is some) { return 2; }\n",
        "  if (fresh[1]!.value != 2 || copied[1]!.value != 17) { return 3; }\n",
        "  if (grouped[1]!.value != 14 || conditional[0] is some || conditional[1]!.value != 15) { return 4; }\n",
        "  if (Item.constructed != 6 || Item.copied != 5 || Item.destroyed != 3) { return 5; }\n",
        "  return 0;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var result: i64 = exercise();\n",
        "  if (result != 0) { return result; }\n",
        "  if (Item.destroyed != 11) { return 91; }\n",
        "  return 42;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

#[test]
fn nested_indexed_arrays_copy_adopt_and_keep_prefixes_independent() {
    let source = concat!(
        "fn make_row(row: i64) -> i64[] {\n",
        "  return i64[]((u64) (row + 1); column => row * 10 + column);\n",
        "}\n",
        "fn maybe_row(row: i64) -> i64[]? {\n",
        "  if (row == 0) { return none; }\n",
        "  return make_row(row);\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var named: i64[] = i64[]{7, 8};\n",
        "  var copies: i64[][] = i64[][](2u; row => named);\n",
        "  var adopted: i64[][] = i64[][](3u; row => make_row(row));\n",
        "  var nested: i64[][] = i64[][](3u; row =>\n",
        "    i64[]((u64) (row + 1); column => row * 10 + column));\n",
        "  var optional: i64[]?[] = i64[]?[](2u; row => maybe_row(row));\n",
        "  var shared: shared i64[][] = new i64[][](2u; row => make_row(row + 3));\n",
        "  if (copies[1].len() != 2u || copies[1][0] != 7) { return 1; }\n",
        "  if (adopted[0].len() != 1u || adopted[2].len() != 3u || adopted[2][2] != 22) { return 2; }\n",
        "  if (nested[1].len() != 2u || nested[1][1] != 11) { return 3; }\n",
        "  if (optional[0] is some || optional[1]![1] != 11) { return 4; }\n",
        "  if (shared->[0].len() != 4u || shared->[1][4] != 44) { return 5; }\n",
        "  return 42;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

#[test]
fn shared_owner_indexed_arrays_preserve_polymorphism_transfers_and_outer_ownership() {
    let source = concat!(
        "interface Value { fn read() -> i64; }\n",
        "class Base implements Value {\n",
        "  static destroyed: i64;\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  virtual fn read() -> i64 { return self.value; }\n",
        "  destroy { Base.destroyed = Base.destroyed + 1; }\n",
        "}\n",
        "class Leaf extends Base {\n",
        "  init(value: i64) { super(value); }\n",
        "  override fn read() -> i64 { return self.value + 1; }\n",
        "}\n",
        "class Holder {\n",
        "  edge: shared Leaf;\n",
        "  init(edge: shared Leaf) { self.edge = edge; }\n",
        "}\n",
        "fn make(value: i64) -> shared Leaf { return new Leaf(value); }\n",
        "fn make_obj(value: i64) -> shared Obj { return new Leaf(value); }\n",
        "fn make_box(value: i64) -> shared Leaf? { return new Leaf?(Leaf(value)); }\n",
        "fn maybe(index: i64) -> shared? Leaf {\n",
        "  if (index == 1) { return none; }\n",
        "  return make(index + 30);\n",
        "}\n",
        "fn maybe_box(index: i64) -> shared? Leaf? {\n",
        "  if (index == 0) { return none; }\n",
        "  return make_box(index + 90);\n",
        "}\n",
        "fn exercise() -> i64 {\n",
        "  var named: shared Leaf = new Leaf(10);\n",
        "  var holder: shared Holder = new Holder(named);\n",
        "  {\n",
        "    var empty: (shared Leaf)[] = (shared Leaf)[](0u; index => make(index));\n",
        "    var retained: (shared Leaf)[] = (shared Leaf)[](3u; index => named);\n",
        "    var produced: (shared Leaf)[] = (shared Leaf)[](2u; index => make(index + 20));\n",
        "    var bases: (shared Base)[] = (shared Base)[](2u; index => make(index + 40));\n",
        "    var views: (shared Value)[] = (shared Value)[](2u; index => make(index + 50));\n",
        "    var objects: (shared Obj)[] = (shared Obj)[](2u; index => make_obj(index + 60));\n",
        "    var optional: (shared? Leaf)[] = (shared? Leaf)[](3u; index => maybe(index));\n",
        "    var boxes: (shared Leaf?)[] = (shared Leaf?)[](2u; index => make_box(index + 80));\n",
        "    var maybe_boxes: (shared? Leaf?)[] = (shared? Leaf?)[](2u; index => maybe_box(index));\n",
        "    var anchored: (shared Leaf)[] = (shared Leaf)[](2u; index => holder->edge);\n",
        "    var rows: (shared i64[])[] = (shared i64[])[](2u; row =>\n",
        "      new i64[]((u64) (row + 1); column => row * 10 + column));\n",
        "    var shared_outer: shared (shared Value)[] =\n",
        "      new (shared Value)[](2u; index => make(index + 70));\n",
        "    if (empty.len() != 0u || retained[2]->read() != 11) { return 1; }\n",
        "    if (produced[1]->read() != 22 || bases[1]->read() != 42) { return 2; }\n",
        "    if (views[1]->read() != 52 || !(*objects[1] is Leaf)) { return 3; }\n",
        "    if (optional[0]!->read() != 31 || optional[1] is some || optional[2]!->read() != 33) { return 4; }\n",
        "    var box: shared Leaf? = boxes[1];\n",
        "    var maybe_box_owner: shared Leaf? = maybe_boxes[1]!;\n",
        "    if ((*box)!.read() != 82 || (*maybe_box_owner)!.read() != 92) { return 7; }\n",
        "    if (anchored[1]->read() != 11 || rows[1]->[1] != 11 || shared_outer->[1]->read() != 72) { return 5; }\n",
        "  }\n",
        "  if (Base.destroyed != 15 || named->read() != 11 || holder->edge->read() != 11) { return 6; }\n",
        "  return 42;\n",
        "}\n",
        "fn main() -> i64 { return exercise(); }\n",
    );
    let mut output = assembly(source);
    assert_eq!(output, assembly(source));
    assert!(output.contains("ownership_retain_overflow"), "{output}");
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

#[test]
fn shared_owner_indexed_arrays_release_every_allocation_after_normal_cleanup() {
    let source = concat!(
        "extern fn validate_allocations() -> i64;\n",
        "class Item { init(value: i64) {} }\n",
        "fn make(value: i64) -> shared Item { return new Item(value); }\n",
        "fn make_box(value: i64) -> shared Item? { return new Item?(Item(value)); }\n",
        "fn maybe(index: i64) -> shared? Item {\n",
        "  if (index == 0) { return none; }\n",
        "  return make(index);\n",
        "}\n",
        "fn build() -> unit {\n",
        "  var named: shared Item = new Item(1);\n",
        "  var retained: (shared Item)[] = (shared Item)[](3u; index => named);\n",
        "  var produced: shared (shared Item)[] = new (shared Item)[](2u; index => make(index));\n",
        "  var optional: (shared? Item)[] = (shared? Item)[](3u; index => maybe(index));\n",
        "  var boxes: (shared Item?)[] = (shared Item?)[](2u; index => make_box(index));\n",
        "  var maybe_boxes: (shared? Item?)[] = (shared? Item?)[](2u; index => make_box(index));\n",
        "  var rows: (shared i64[])[] = (shared i64[])[](2u; row =>\n",
        "    new i64[]((u64) (row + 1); column => row + column));\n",
        "  return;\n",
        "}\n",
        "fn main() -> i64 { build(); return validate_allocations(); }\n",
    );
    let mut output = assembly(source);
    output.push_str(indexed_owner_allocation_probe());

    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

fn indexed_owner_allocation_probe() -> &'static str {
    concat!(
        "\n.bss\n",
        ".p2align 3\n",
        ".Lindexed_owner_allocations: .quad 0\n",
        ".Lindexed_owner_frees: .quad 0\n",
        "\n.text\n",
        ".globl ska_rt_alloc\n",
        ".type ska_rt_alloc, @function\n",
        "ska_rt_alloc:\n",
        "    add qword ptr [rip + .Lindexed_owner_allocations], 1\n",
        "    jmp malloc@PLT\n",
        ".size ska_rt_alloc, .-ska_rt_alloc\n",
        ".globl ska_rt_free\n",
        ".type ska_rt_free, @function\n",
        "ska_rt_free:\n",
        "    add qword ptr [rip + .Lindexed_owner_frees], 1\n",
        "    jmp free@PLT\n",
        ".size ska_rt_free, .-ska_rt_free\n",
        ".globl validate_allocations\n",
        ".type validate_allocations, @function\n",
        "validate_allocations:\n",
        "    mov rax, qword ptr [rip + .Lindexed_owner_allocations]\n",
        "    test rax, rax\n",
        "    je .Lindexed_owner_allocation_failure\n",
        "    cmp rax, qword ptr [rip + .Lindexed_owner_frees]\n",
        "    jne .Lindexed_owner_allocation_failure\n",
        "    mov rax, 42\n",
        "    ret\n",
        ".Lindexed_owner_allocation_failure:\n",
        "    mov rax, 1\n",
        "    ret\n",
        ".size validate_allocations, .-validate_allocations\n",
    )
}
