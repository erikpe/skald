use super::*;

#[test]
fn primitive_element_lists_execute_for_every_scalar_and_outer_ownership() {
    for (ty, first, second) in [
        ("i64", "-7", "42"),
        ("u64", "1u", "9u"),
        ("u8", "2u8", "8u8"),
        ("f64", "1.25", "2.5"),
        ("bool", "false", "true"),
    ] {
        let source = format!(
            concat!(
                "fn main() -> i64 {{\n",
                "  var inline: {ty}[] = {ty}[]{{{first}, {second}}};\n",
                "  var shared: shared {ty}[] = new {ty}[]{{{first}, {second}}};\n",
                "  if (inline[0] != {first}) {{ return 1; }}\n",
                "  if (inline[1] != {second}) {{ return 2; }}\n",
                "  if (shared->[0] != {first}) {{ return 3; }}\n",
                "  if (shared->[1] != {second}) {{ return 4; }}\n",
                "  return 42;\n",
                "}}\n",
            ),
            ty = ty,
            first = first,
            second = second,
        );
        let mut output = assembly(&source);
        assert!(output.contains("call ska_rt_abi_v8"));
        assert!(!output.contains("ska_rt_array_element_list"));
        output.push_str(native_allocator());
        assert_eq!(
            run_native_assembly(&output).code(),
            Some(42),
            "{ty} element lists failed\n{output}"
        );
    }
}

#[test]
fn primitive_element_lists_execute_in_nested_expression_contexts() {
    let source = concat!(
        "class Holder {\n",
        "  values: i64[];\n",
        "  init(values: i64[]) { self.values = values; }\n",
        "  mut fn replace() -> unit { self.values = i64[]{30, 12}; }\n",
        "  fn first() -> i64 { return self.values[0]; }\n",
        "}\n",
        "fn returned() -> i64[] { return i64[]{40, 2}; }\n",
        "fn first(values: i64[]) -> i64 { return values[0]; }\n",
        "fn main() -> i64 {\n",
        "  var empty: i64[] = i64[]{};\n",
        "  if (empty.len() != 0u) { return 1; }\n",
        "  if (first(i64[]{42}) != 42) { return 2; }\n",
        "  if (i64[]{42}[0] != 42) { return 3; }\n",
        "  if (returned()[0] != 40) { return 4; }\n",
        "  var holder: Holder = Holder(i64[]{10, 20});\n",
        "  holder.replace();\n",
        "  if (holder.first() != 30) { return 5; }\n",
        "  return 42;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

#[test]
fn primitive_element_lists_preserve_control_effects_between_positions() {
    let source = concat!(
        "class Trace { static count: i64; init() {} }\n",
        "fn mark(value: bool) -> bool { Trace.count = Trace.count + 1; return value; }\n",
        "fn main() -> i64 {\n",
        "  var values: bool[] = bool[]{false && mark(true), true || mark(false), mark(true) && mark(true)};\n",
        "  if (values[0] || !values[1] || !values[2]) { return 1; }\n",
        "  if (Trace.count != 2) { return 2; }\n",
        "  return 42;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

#[test]
fn primitive_element_list_expressions_run_once_in_left_to_right_order() {
    let source = concat!(
        "class Trace { static order: i64; init() {} }\n",
        "fn record(value: i64) -> i64 { Trace.order = Trace.order * 10 + value; return value; }\n",
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[]{record(1), record(2), record(3)};\n",
        "  if (values[0] != 1 || values[1] != 2 || values[2] != 3) { return 1; }\n",
        "  if (Trace.order != 123) { return 2; }\n",
        "  return 42;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

#[test]
fn exact_class_element_lists_execute_all_source_shapes_and_outer_ownership() {
    let source = concat!(
        "class Item {\n",
        "  tag: u8; value: i64; enabled: bool;\n",
        "  init(value: i64) { self.tag = 7u8; self.value = value; self.enabled = true; }\n",
        "  copy(ref other: Item) { self.tag = other.tag; self.value = other.value + 10; self.enabled = other.enabled; }\n",
        "}\n",
        "class Holder {\n",
        "  values: Item[];\n",
        "  init(values: Item[]) { self.values = values; }\n",
        "  mut fn replace() -> unit { self.values = Item[]{Item(8)}; }\n",
        "  fn first() -> i64 { return self.values[0].value; }\n",
        "}\n",
        "fn make(value: i64) -> Item { return Item(value); }\n",
        "fn first(values: Item[]) -> i64 { return values[0].value; }\n",
        "fn returned() -> Item[] { return Item[]{make(6)}; }\n",
        "fn main() -> i64 {\n",
        "  var source: Item = Item(3);\n",
        "  var inline: Item[] = Item[]{Item(1), make(2), source, (Item(4))};\n",
        "  var shared: shared Item[] = new Item[]{Item(1), make(2), source, (Item(4))};\n",
        "  var empty: Item[] = Item[]{};\n",
        "  var shared_empty: shared Item[] = new Item[]{};\n",
        "  if (inline[0].value != 1 || inline[1].value != 2 || inline[2].value != 13 || inline[3].value != 14) { return 1; }\n",
        "  if (!inline[0].enabled || inline[0].tag != 7u8) { return 2; }\n",
        "  if (shared->[0].value != 1 || shared->[1].value != 2 || shared->[2].value != 13 || shared->[3].value != 14) { return 3; }\n",
        "  if (empty.len() != 0u || shared_empty->len() != 0u) { return 8; }\n",
        "  if (first(Item[]{Item(5)}) != 5) { return 4; }\n",
        "  if (returned()[0].value != 6) { return 5; }\n",
        "  var holder: Holder = Holder(Item[]{Item(7)});\n",
        "  if (holder.first() != 17) { return 6; }\n",
        "  holder.replace();\n",
        "  if (holder.first() != 8) { return 7; }\n",
        "  return 42;\n",
        "}\n",
    );
    let mut output = assembly(source);
    assert!(output.contains("call .Lska.class.main.Item.c0.init.i0"));
    assert!(output.contains("call .Lska.class.main.Item.c0.copy.k0"));
    assert!(!output.contains("memcpy"));
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

#[test]
fn exact_class_element_lists_preserve_lifecycle_order_and_reverse_destruction() {
    let source = concat!(
        "extern fn observe(value: i64) -> unit;\n",
        "extern fn validate() -> i64;\n",
        "class Trace {\n",
        "  marker: i64;\n",
        "  init(marker: i64) { self.marker = marker; }\n",
        "  copy(ref other: Trace) { self.marker = other.marker + 10; }\n",
        "  destroy { observe(self.marker + 100); }\n",
        "}\n",
        "fn mark(marker: i64) -> i64 { observe(marker); return marker; }\n",
        "fn make(marker: i64) -> Trace { return Trace(mark(marker)); }\n",
        "fn build() -> unit {\n",
        "  var source: Trace = Trace(mark(3));\n",
        "  var values: Trace[] = Trace[]{Trace(mark(1)), make(2), source, (Trace(mark(4)))};\n",
        "  return;\n",
        "}\n",
        "fn main() -> i64 { build(); return validate(); }\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());
    output.push_str(class_element_lifecycle_probe());

    assert_eq!(run_native_assembly(&output).code(), Some(0), "{output}");
}

#[test]
fn exact_class_element_lists_copy_ancestor_slices_and_checked_sources() {
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
        "  var values: Base[] = Base[]{source};\n",
        "  return values[0].value;\n",
        "}\n",
        "fn checked(ref source: Base) -> i64 {\n",
        "  var values: Leaf[] = Leaf[]{(Leaf) source};\n",
        "  return values[0].value + values[0].extra;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var leaf: Leaf = Leaf(2, 30);\n",
        "  return ancestor(leaf) + checked(leaf);\n",
        "}\n",
    );

    let mut output = assembly(source);
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(54));
}

fn class_element_lifecycle_probe() -> &'static str {
    concat!(
        "\n.section .rodata\n",
        ".align 8\n",
        ".Lclass_element_expected:\n",
        "    .quad 3, 1, 2, 4, 104, 114, 113, 102, 101, 103\n",
        ".bss\n",
        ".align 8\n",
        ".Lclass_element_index:\n",
        "    .zero 8\n",
        ".text\n",
        ".globl observe\n",
        ".type observe, @function\n",
        "observe:\n",
        "    mov rax, qword ptr [rip + .Lclass_element_index]\n",
        "    cmp rax, 10\n",
        "    jae .Lclass_element_bad\n",
        "    lea rcx, [rip + .Lclass_element_expected]\n",
        "    cmp rdi, qword ptr [rcx + rax * 8]\n",
        "    jne .Lclass_element_bad\n",
        "    inc rax\n",
        "    mov qword ptr [rip + .Lclass_element_index], rax\n",
        "    ret\n",
        ".globl validate\n",
        ".type validate, @function\n",
        "validate:\n",
        "    xor eax, eax\n",
        "    cmp qword ptr [rip + .Lclass_element_index], 10\n",
        "    je .Lclass_element_done\n",
        "    mov eax, 98\n",
        ".Lclass_element_done:\n",
        "    ret\n",
        ".Lclass_element_bad:\n",
        "    mov rax, 60\n",
        "    mov rdi, 99\n",
        "    syscall\n",
        ".size observe, .-observe\n",
        ".size validate, .-validate\n",
    )
}
