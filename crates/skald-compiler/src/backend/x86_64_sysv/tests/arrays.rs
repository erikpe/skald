use super::*;

#[test]
fn nontrivial_nested_and_recursive_array_layouts_are_finite_and_aligned() {
    let program = lower_text(concat!(
        "class Item { value: i64; init() { self.value = 0; } }\n",
        "class Node { children: Node[]; init() { self.children = Node[](); } }\n",
        "fn main() -> i64 {\n",
        "  var items: Item[] = Item[]();\n",
        "  var optional: Item?[] = Item?[]();\n",
        "  var nested: i64[][] = i64[][]();\n",
        "  var owners: (shared Item)[] = (shared Item)[]();\n",
        "  var maybe_owners: (shared? Item)[] = (shared? Item)[]();\n",
        "  var array_owners: (shared i64[])[] = (shared i64[])[]();\n",
        "  var maybe_array_owners: (shared? i64[])[] = (shared? i64[])[]();\n",
        "  return 0;\n",
        "}\n",
    ));
    let layouts = super::super::layout::DataLayout::compute(&program).unwrap();

    let array = |element| {
        program
            .array_types
            .iter()
            .find(|array| array.element == element)
            .expect("source declares the requested array")
            .id
    };
    let optional = |payload| {
        MirType::Optional(
            program
                .optional_for_payload(payload)
                .expect("source declares the requested optional"),
        )
    };
    let item = layouts
        .array(array(MirType::Class(ClassId::new(0))))
        .unwrap();
    let optional_layout = layouts
        .array(array(optional(MirType::Class(ClassId::new(0)))))
        .unwrap();
    let primitive = array(MirType::I64);
    let nested = layouts.array(array(MirType::Array(primitive))).unwrap();
    let shared = layouts
        .array(array(MirType::Shared(MirSharedTarget::Class(
            ClassId::new(0),
        ))))
        .unwrap();
    let optional_shared = layouts
        .array(array(optional(MirType::Shared(MirSharedTarget::Class(
            ClassId::new(0),
        )))))
        .unwrap();
    let shared_array = layouts
        .array(array(MirType::Shared(MirSharedTarget::Array(primitive))))
        .unwrap();
    let optional_shared_array = layouts
        .array(array(optional(MirType::Shared(MirSharedTarget::Array(
            primitive,
        )))))
        .unwrap();
    let node = layouts.class(ClassId::new(1)).unwrap();

    assert_eq!(
        item.stride(),
        layouts.class(ClassId::new(0)).unwrap().ty().size()
    );
    assert!(optional_layout.stride() > item.stride());
    assert_eq!(nested.stride(), 8);
    assert_eq!(shared.stride(), 8);
    assert_eq!(optional_shared.stride(), 8);
    assert_eq!(shared_array.stride(), 8);
    assert_eq!(optional_shared_array.stride(), 8);
    assert_eq!(item.shared_element_offset(), 24);
    assert_eq!(nested.shared_element_offset(), 24);
    assert_eq!(node.ty().size(), 8);
}

#[test]
fn primitive_inline_array_helpers_are_deterministic_and_layout_specialized() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  var wide: i64[] = i64[]{1, 2, 3};\n",
        "  var bytes: u8[] = u8[]{1u8, 2u8, 3u8};\n",
        "  return 0;\n",
        "}\n",
    );
    let first = assembly(source);
    let second = assembly(source);

    assert_eq!(first, second);
    assert!(first.contains(".Lska_array_0_initialize_element:"));
    assert!(first.contains(".Lska_array_0_copy_element:"));
    assert!(first.contains(".Lska_array_0_clone:"));
    assert!(first.contains("mov qword ptr [rdi + rsi*8 + 16], rax"));
    assert!(first.contains(".Lska_array_1_initialize_element:"));
    assert!(first.contains(".Lska_array_1_copy_element:"));
    assert!(first.contains(".Lska_array_1_clone:"));
    assert!(first.contains("mov byte ptr [rdi + rsi*1 + 16], al"));
    assert!(first.contains(".Lska_array_0_release:"));
    assert!(first.contains("call ska_rt_abi_v9"));
    assert!(first.contains("call ska_rt_alloc"));
    assert!(first.contains("call ska_rt_free"));
    assert!(!first.contains("ska_rt_array"));
    assert_system_assembler_accepts(&first);

    let mut labels = std::collections::HashSet::new();
    for label in first
        .lines()
        .filter_map(|line| line.trim().strip_suffix(':'))
        .filter(|label| label.starts_with(".Lska_array_"))
    {
        assert!(
            labels.insert(label),
            "generated array helper label `{label}` collided"
        );
    }
}

#[test]
fn empty_and_nonempty_primitive_arrays_initialize_report_length_and_free_once() {
    for (element, length, byte_elements) in [
        ("i64", 3_u64, false),
        ("u64", 3, false),
        ("f64", 3, false),
        ("u8", 3, true),
        ("bool", 3, true),
        ("i64", 0, false),
    ] {
        let construction = if length == 0 {
            format!("{element}[]()")
        } else {
            format!("{element}[]({length}u)")
        };
        let source = format!(
            concat!(
                "extern fn validate_live(length: u64) -> unit;\n",
                "extern fn validate_counts() -> i64;\n",
                "fn build() -> unit {{\n",
                "  var values: {0}[] = {1};\n",
                "  validate_live(values.len());\n",
                "  return;\n",
                "}}\n",
                "fn main() -> i64 {{ build(); return validate_counts(); }}\n",
            ),
            element, construction
        );
        let mut output = assembly(&source);
        output.push_str(&array_runtime_probe(length, byte_elements));

        let result = run_native_assembly_output(&output);
        assert!(
            result.status.success(),
            "{element}[{length}] failed with {:?}: {}\n{output}",
            result.status,
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn dynamic_length_overflow_terminates_before_allocation() {
    let source = concat!(
        "fn too_large() -> u64 { return 9223372036854775807u; }\n",
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[](too_large());\n",
        "  return 0;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(concat!(
        "\n.text\n",
        ".globl ska_rt_alloc\n",
        ".type ska_rt_alloc, @function\n",
        "ska_rt_alloc:\n",
        "    ud2\n",
        ".size ska_rt_alloc, .-ska_rt_alloc\n",
        ".globl ska_rt_free\n",
        ".type ska_rt_free, @function\n",
        "ska_rt_free:\n",
        "    ud2\n",
        ".size ska_rt_free, .-ska_rt_free\n",
    ));

    assert!(!run_native_assembly(&output).success());
}

#[test]
fn array_owner_count_overflow_reports_exact_message_natively() {
    for (source, overflow_label, saturated_count) in [
        (
            concat!(
                "fn main() -> i64 {\n",
                "  var first: shared i64[] = new i64[](1u);\n",
                "  var second: shared i64[] = first;\n",
                "  return second->[0];\n",
                "}\n",
            ),
            "ownership_retain_overflow",
            "0xfffffffffffffffe",
        ),
        (
            concat!(
                "class Item { value: i64; init() { self.value = 0; } }\n",
                "fn observe(ref item: Item) -> i64 { return item.value; }\n",
                "fn main() -> i64 {\n",
                "  var items: Item[] = Item[](1u);\n",
                "  return observe(items[0]);\n",
                "}\n",
            ),
            "anchor_retain_overflow",
            "0xffffffffffffffff",
        ),
    ] {
        let mut output = assembly(source);
        let mut overflows = output.match_indices(overflow_label).map(|(index, _)| index);
        let first_overflow = overflows
            .next()
            .expect("the checked retain must expose its overflow edge");
        let overflow = overflows.next().unwrap_or(first_overflow);
        let count_load = "    mov rcx, qword ptr [rax]\n";
        let load = output[..overflow]
            .rfind(count_load)
            .expect("the checked retain must load its owner count");
        output.replace_range(
            load..load + count_load.len(),
            &format!("    mov rcx, {saturated_count}\n"),
        );
        output.push_str(native_allocator());
        output.push_str(native_panic_reporter());

        let result = run_native_assembly_output(&output);
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        assert_eq!(result.stderr, b"panic: ownership count overflow\n");
    }
}

#[test]
fn primitive_element_access_uses_checked_specialized_addressing() {
    let source = concat!(
        "extern fn index_once() -> i64;\n",
        "fn main() -> i64 {\n",
        "  var wide: i64[] = i64[](2u);\n",
        "  var bytes: u8[] = u8[](2u);\n",
        "  var flags: bool[] = bool[](2u);\n",
        "  var floats: f64[] = f64[](2u);\n",
        "  wide[index_once()] = 7;\n",
        "  bytes[0] = 8u8;\n",
        "  flags[-1] = true;\n",
        "  floats[0] = 1.5;\n",
        "  var observed: f64 = floats[0];\n",
        "  return wide[-1];\n",
        "}\n",
    );
    let output = assembly(source);

    assert_eq!(output.matches("call index_once").count(), 1);
    assert!(output.contains("jns "));
    assert!(output.contains("mov r11, 0xffffffffffffffff"));
    assert!(output.contains("mov qword ptr [r11 + r10*8 + 16], rax"));
    assert!(output.contains("mov byte ptr [r11 + r10*1 + 16], al"));
    assert!(output.contains("movsd qword ptr [r11 + r10*8 + 16], xmm14"));
    assert!(output.contains("movsd xmm14, qword ptr [r11 + r10*8 + 16]"));
}

#[test]
fn positive_and_negative_element_boundaries_execute_natively() {
    for (index, expected) in [("-3", 12), ("0", 12), ("-1", 14), ("2", 14)] {
        let source = format!(
            concat!(
                "fn main() -> i64 {{\n",
                "  var values: i64[] = i64[](3u);\n",
                "  values[-3] = 11;\n",
                "  values[0] = 12;\n",
                "  values[-1] = 13;\n",
                "  values[2] = 14;\n",
                "  return values[{index}];\n",
                "}}\n",
            ),
            index = index,
        );
        let mut output = assembly(&source);
        output.push_str(native_allocator());
        assert_eq!(
            run_native_assembly(&output).code(),
            Some(expected),
            "index {index} selected the wrong element"
        );
    }
}

#[test]
fn invalid_element_boundaries_terminate_before_addressing() {
    for (length, index) in [
        ("0u", "0"),
        ("0u", "-1"),
        ("3u", "-4"),
        ("3u", "3"),
        ("3u", "-9223372036854775808"),
    ] {
        let source = format!(
            concat!(
                "fn main() -> i64 {{\n",
                "  var values: i64[] = i64[]({length});\n",
                "  return values[{index}];\n",
                "}}\n",
            ),
            length = length,
            index = index,
        );
        let mut output = assembly(&source);
        output.push_str(native_allocator());
        assert!(
            !run_native_assembly(&output).success(),
            "i64[{length}][{index}] unexpectedly succeeded"
        );
    }
}

#[test]
fn primitive_array_ownership_crosses_calls_results_and_replacement_without_extra_adoption_copy() {
    let source = concat!(
        "extern fn validate_counts() -> i64;\n",
        "fn consume(values: i64[]) -> unit { return; }\n",
        "fn make() -> i64[] { return i64[](2u); }\n",
        "fn exercise() -> unit {\n",
        "  var source: i64[] = i64[](2u);\n",
        "  var named: i64[] = source;\n",
        "  var produced: i64[] = i64[](2u);\n",
        "  named = source;\n",
        "  produced = i64[](3u);\n",
        "  consume(source);\n",
        "  consume(i64[](1u));\n",
        "  var result: i64[] = make();\n",
        "  return;\n",
        "}\n",
        "fn main() -> i64 { exercise(); return validate_counts(); }\n",
    );
    let mut output = assembly(source);
    output.push_str(&ownership_counter_probe(8));

    let result = run_native_assembly_output(&output);
    assert!(
        result.status.success(),
        "array ownership accounting failed with {:?}: {}\n{output}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn nontrivial_and_nested_inline_array_lifecycle_reaches_native_lowering() {
    let source = concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init() { self.value = 1; }\n",
        "}\n",
        "class Node {\n",
        "  children: Node[];\n",
        "  init() { self.children = Node[](); }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var items: Item[] = Item[](2u);\n",
        "  var optional: Item?[] = Item?[](2u);\n",
        "  optional[0] = Item();\n",
        "  var nested: i64[][] = i64[][](2u);\n",
        "  nested[0] = i64[](3u);\n",
        "  var nodes: Node[] = Node[](1u);\n",
        "  var copied: Node[] = nodes;\n",
        "  return items[0].value;\n",
        "}\n",
    );
    let mut output = assembly(source);

    assert!(output.contains(".Lska.class.main.Item.c0.copy_complete:"));
    assert!(output.contains(".Lska_array_0_destroy_element:"));
    assert!(output.contains(".Lska_array_3_clone:"));
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(1));
}

#[test]
fn readonly_and_mutable_whole_array_aliases_execute_natively() {
    let source = concat!(
        "fn read(ref values: i64[]) -> i64 { return values[0] + values[-1]; }\n",
        "fn mutate(mut ref values: i64[]) -> unit {\n",
        "  values[0] = 7;\n",
        "  values[-1] = 9;\n",
        "  return;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[](2u);\n",
        "  mutate(values);\n",
        "  return read(values);\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(16));
}

#[test]
fn class_and_nested_array_element_aliases_preserve_exact_element_identity() {
    let source = concat!(
        "class Item { value: i64; init() { self.value = 0; } }\n",
        "fn touch(mut ref item: Item) -> unit { item.value = 11; return; }\n",
        "fn write(mut ref values: i64[]) -> unit { values[0] = 13; return; }\n",
        "fn overlap(mut ref left: Item, mut ref right: Item) -> unit {\n",
        "  left.value = 17;\n",
        "  right.value = 19;\n",
        "  return;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var items: Item[] = Item[](1u);\n",
        "  var rows: i64[][] = i64[][](1u);\n",
        "  rows[0] = i64[](1u);\n",
        "  touch(items[0]);\n",
        "  write(rows[0]);\n",
        "  overlap(items[0], items[0]);\n",
        "  return items[0].value + rows[0][0];\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(32));
}

#[test]
fn shared_and_optional_shared_array_aliases_reuse_secured_owner_anchors() {
    let source = concat!(
        "class Item { value: i64; init() { self.value = 0; } }\n",
        "class Holder {\n",
        "  values: shared i64[];\n",
        "  init(values: shared i64[]) { self.values = values; }\n",
        "}\n",
        "fn write(mut ref values: i64[]) -> unit { values[0] = 7; return; }\n",
        "fn read(ref values: i64[]) -> i64 { return values[0]; }\n",
        "fn forward(ref values: i64[]) -> i64 { return read(values); }\n",
        "fn make() -> shared i64[] { return new i64[](1u); }\n",
        "fn touch(mut ref item: Item) -> unit { item.value = 11; return; }\n",
        "fn main() -> i64 {\n",
        "  var values: shared i64[] = new i64[](1u);\n",
        "  var maybe: shared? i64[] = values;\n",
        "  write(*values);\n",
        "  write(*maybe!);\n",
        "  var holder: Holder = Holder(values);\n",
        "  var items: shared Item[] = new Item[](1u);\n",
        "  touch(items->[0]);\n",
        "  return read(*values) + read(*maybe!) + read(*holder.values)\n",
        "    + forward(*values)\n",
        "    + read(*make()) + items->[0].value;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(39));
}

#[test]
fn detached_element_alias_keeps_old_backing_while_descriptor_alias_observes_replacement() {
    let source = concat!(
        "extern fn validate_counts() -> i64;\n",
        "class Item {\n",
        "  value: i64;\n",
        "  init() { self.value = 0; }\n",
        "  fn observe(ignored: i64) -> i64 { return self.value; }\n",
        "  destroy { self.value = 100; }\n",
        "}\n",
        "class Holder {\n",
        "  items: Item[];\n",
        "  init(value: i64) {\n",
        "    self.items = Item[](1u);\n",
        "    self.items[0].value = value;\n",
        "  }\n",
        "  mut fn replace(value: i64) -> i64 {\n",
        "    self.items = Item[](1u);\n",
        "    self.items[0].value = value;\n",
        "    return 0;\n",
        "  }\n",
        "}\n",
        "fn observe_element(ref item: Item, ignored: i64) -> i64 {\n",
        "  return item.value;\n",
        "}\n",
        "fn observe_array(ref items: Item[], ignored: i64) -> i64 {\n",
        "  return items[0].value;\n",
        "}\n",
        "fn replace_during_call(ref item: Item, mut ref holder: Holder) -> i64 {\n",
        "  holder.items = Item[](1u);\n",
        "  holder.items[0].value = 50;\n",
        "  return item.value;\n",
        "}\n",
        "fn build() -> i64 {\n",
        "  var element_holder: Holder = Holder(7);\n",
        "  var descriptor_holder: Holder = Holder(8);\n",
        "  var receiver_holder: Holder = Holder(9);\n",
        "  var call_holder: Holder = Holder(12);\n",
        "  var old_element: i64 = observe_element(\n",
        "    element_holder.items[0], element_holder.replace(20));\n",
        "  var new_element: i64 = observe_array(\n",
        "    descriptor_holder.items, descriptor_holder.replace(30));\n",
        "  var old_receiver: i64 = receiver_holder.items[0].observe(\n",
        "    receiver_holder.replace(40));\n",
        "  var during_call: i64 = replace_during_call(\n",
        "    call_holder.items[0], call_holder);\n",
        "  return old_element + new_element + old_receiver + during_call;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var result: i64 = build();\n",
        "  return result + validate_counts();\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(&ownership_counter_probe(8));

    assert_eq!(run_native_assembly(&output).code(), Some(58));
}

#[test]
fn shared_array_owners_share_one_non_null_backing_across_calls_and_optionals() {
    let source = concat!(
        "class Holder {\n",
        "  values: shared i64[];\n",
        "  init(values: shared i64[]) { self.values = values; }\n",
        "}\n",
        "fn mutate(values: shared i64[]) -> shared i64[] {\n",
        "  values->[0] = 40;\n",
        "  return values;\n",
        "}\n",
        "fn from_holder(holder: Holder) -> shared i64[] { return holder.values; }\n",
        "fn main() -> i64 {\n",
        "  var empty: shared i64[] = new i64[]();\n",
        "  var original: shared i64[] = new i64[](2u);\n",
        "  var alias: shared i64[] = original;\n",
        "  var returned: shared i64[] = mutate(alias);\n",
        "  alias = new i64[](1u);\n",
        "  alias->[0] = 9;\n",
        "  original = original;\n",
        "  var holder: Holder = Holder(returned);\n",
        "  var maybe: shared? i64[] = from_holder(holder);\n",
        "  maybe!->[1] = 2;\n",
        "  var clone: shared i64[] = new i64[](copy *original);\n",
        "  clone->[0] = 1;\n",
        "  return original->[0] + (*original)[1];\n",
        "}\n",
    );
    let mut output = assembly(source);

    assert!(output.contains(".Lska_array_0_shared_metadata:"));
    assert!(output.contains(".Lska_array_0_finalize_shared:"));
    assert!(output.contains("mov qword ptr [rdx + 16], rax"));
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(42));
}

#[test]
fn empty_and_nonempty_shared_arrays_each_use_one_outer_allocation() {
    for construction in ["new i64[]()", "new i64[](3u)"] {
        let source = format!(
            concat!(
                "extern fn validate_counts() -> i64;\n",
                "fn build() -> unit {{\n",
                "  var absent: shared? i64[] = none;\n",
                "  var values: shared i64[] = {construction};\n",
                "  return;\n",
                "}}\n",
                "fn main() -> i64 {{ build(); return validate_counts(); }}\n",
            ),
            construction = construction,
        );
        let mut output = assembly(&source);
        output.push_str(&ownership_counter_probe(1));
        assert_eq!(
            run_native_assembly(&output).code(),
            Some(0),
            "{construction}"
        );
    }
}

#[test]
fn shared_array_last_owner_finalizes_exact_class_elements_in_reverse_order() {
    let source = concat!(
        "extern fn observe(value: i64) -> unit;\n",
        "extern fn validate() -> i64;\n",
        "class Item {\n",
        "  value: i64;\n",
        "  init() { self.value = 0; }\n",
        "  destroy { observe(self.value); }\n",
        "}\n",
        "fn build() -> unit {\n",
        "  var values: shared Item[] = new Item[](3u);\n",
        "  values->[0].value = 1;\n",
        "  values->[1].value = 2;\n",
        "  values->[2].value = 3;\n",
        "  return;\n",
        "}\n",
        "fn main() -> i64 { build(); return validate(); }\n",
    );
    let mut output = assembly(source);
    output.push_str(shared_array_trace_probe());

    assert_eq!(run_native_assembly(&output).code(), Some(0));
}

#[test]
fn shared_owner_elements_execute_in_inline_and_shared_outer_arrays() {
    let source = concat!(
        "class Item {\n",
        "  marker: i64;\n",
        "  init() { self.marker = 1; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var owners: (shared Item)[] = (shared Item)[](2u);\n",
        "  var maybe: (shared? Item)[] = (shared? Item)[](2u);\n",
        "  var shared_owners: shared (shared Item)[] = new (shared Item)[](2u);\n",
        "  var shared_maybe: shared (shared? Item)[] = new (shared? Item)[](2u);\n",
        "  var copied: (shared Item)[] = owners;\n",
        "  var shared_copy: shared (shared Item)[] = new (shared Item)[](copy *shared_owners);\n",
        "  var replacement: shared Item = new Item();\n",
        "  owners[0] = replacement;\n",
        "  owners[0] = owners[0];\n",
        "  maybe[0] = replacement;\n",
        "  maybe[0] = maybe[0];\n",
        "  maybe[0] = none;\n",
        "  var owner_zero: shared Item = owners[0];\n",
        "  var copied_zero: shared Item = copied[0];\n",
        "  var shared_zero: shared Item = shared_owners->[0];\n",
        "  copied_zero->marker = 7;\n",
        "  return owner_zero->marker + copied_zero->marker + shared_zero->marker;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(9));
}

#[test]
fn copied_slices_and_checked_slice_assignment_execute_with_snapshot_semantics() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[](5u);\n",
        "  values[0] = 1;\n",
        "  values[1] = 2;\n",
        "  values[2] = 3;\n",
        "  values[3] = 4;\n",
        "  values[4] = 5;\n",
        "  var middle: i64[] = values[1:-1];\n",
        "  values[1:4] = values[0:3];\n",
        "  var full: i64[] = i64[](5u);\n",
        "  full[:] = values;\n",
        "  return middle[0] + middle[-1] + values[1] + values[3] + full[4];\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(15));
}

#[test]
fn full_slice_assignment_preserves_backing_while_whole_assignment_replaces_it() {
    let source = concat!(
        "extern fn validate_counts() -> i64;\n",
        "fn build() -> unit {\n",
        "  var destination: i64[] = i64[](2u);\n",
        "  var source: i64[] = i64[](2u);\n",
        "  destination[:] = source;\n",
        "  destination = source;\n",
        "  return;\n",
        "}\n",
        "fn main() -> i64 { build(); return validate_counts(); }\n",
    );
    let mut output = assembly(source);
    output.push_str(&ownership_counter_probe(3));

    assert!(run_native_assembly(&output).success());
}

#[test]
fn slices_read_inline_shared_and_optional_shared_receivers_at_every_bound_shape() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  var shared_values: shared i64[] = new i64[](4u);\n",
        "  shared_values->[0] = 10;\n",
        "  shared_values->[1] = 20;\n",
        "  shared_values->[2] = 30;\n",
        "  shared_values->[3] = 40;\n",
        "  var optional_values: shared? i64[] = shared_values;\n",
        "  var leading: i64[] = shared_values->[:2];\n",
        "  var trailing: i64[] = optional_values!->[-2:];\n",
        "  var empty_left: i64[] = leading[:0];\n",
        "  var empty_right: i64[] = trailing[2:];\n",
        "  return leading[1] + trailing[0];\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(50));
}

#[test]
fn slice_lifecycle_uses_class_nested_and_shared_element_operations() {
    let source = concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init() { self.value = 0; }\n",
        "  copy(ref other: Item) { self.value = other.value + 10; }\n",
        "  assign(ref other: Item) { self.value = other.value + 100; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var items: Item[] = Item[](3u);\n",
        "  items[0].value = 1;\n",
        "  items[1].value = 2;\n",
        "  items[2].value = 3;\n",
        "  var assigned: Item[] = Item[](3u);\n",
        "  assigned[:] = items[:];\n",
        "  var rows: i64[][] = i64[][](1u);\n",
        "  rows[0] = i64[](1u);\n",
        "  rows[0][0] = 7;\n",
        "  var copied_rows: i64[][] = rows[:];\n",
        "  var owners: (shared Item)[] = (shared Item)[](1u);\n",
        "  var owner: shared Item = owners[0];\n",
        "  owner->value = 9;\n",
        "  var copied_owners: (shared Item)[] = owners[:];\n",
        "  var copied_owner: shared Item = copied_owners[0];\n",
        "  return assigned[0].value + assigned[1].value + assigned[2].value\n",
        "    + copied_rows[0][0] + copied_owner->value;\n",
        "}\n",
    );
    let mut output = assembly(source);
    assert!(output.contains(".Lska_shared_handle_retain_overflow"));
    assert!(output.contains(".Lska_shared_handle_retain_invalid"));
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(96));
}

#[test]
fn slice_lifecycle_conditionally_assigns_optional_element_categories() {
    let source = concat!(
        "class Item { value: i64; init() { self.value = 6; } }\n",
        "fn main() -> i64 {\n",
        "  var primitive: i64?[] = i64?[](1u);\n",
        "  primitive[0] = 5;\n",
        "  var primitive_copy: i64?[] = primitive[:];\n",
        "  var primitive_destination: i64?[] = i64?[](1u);\n",
        "  primitive_destination[:] = primitive_copy;\n",
        "  var inline: Item?[] = Item?[](1u);\n",
        "  inline[0] = Item();\n",
        "  var inline_copy: Item?[] = inline[:];\n",
        "  var inline_destination: Item?[] = Item?[](1u);\n",
        "  inline_destination[:] = inline_copy;\n",
        "  var owner: shared Item = new Item();\n",
        "  var optional_owner: (shared? Item)[] = (shared? Item)[](1u);\n",
        "  optional_owner[0] = owner;\n",
        "  var owner_copy: (shared? Item)[] = optional_owner[:];\n",
        "  var owner_destination: (shared? Item)[] = (shared? Item)[](1u);\n",
        "  owner_destination[:] = owner_copy;\n",
        "  var inline_value: Item? = inline_destination[0];\n",
        "  var copied_owner: shared? Item = owner_destination[0];\n",
        "  return primitive_destination[0]! + inline_value!.value\n",
        "    + copied_owner!->value;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(17));
}

#[test]
fn invalid_slice_bounds_and_length_mismatch_terminate_before_writes() {
    for body in [
        "var copied: i64[] = values[2:1];",
        "var copied: i64[] = values[-4:];",
        "var copied: i64[] = values[-9223372036854775808:];",
        concat!("var source: i64[] = i64[](2u);\n", "  values[:] = source;",),
    ] {
        let source = format!(
            concat!(
                "fn main() -> i64 {{\n",
                "  var values: i64[] = i64[](3u);\n",
                "  values[0] = 7;\n",
                "  {body}\n",
                "  return values[0];\n",
                "}}\n",
            ),
            body = body,
        );
        let mut output = assembly(&source);
        output.push_str(native_allocator());

        assert!(!run_native_assembly(&output).success(), "{body}");
    }
}

#[test]
fn copied_slice_allocation_failure_terminates_without_publishing_a_result() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[](2u);\n",
        "  var copied: i64[] = values[:];\n",
        "  return copied[0];\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(second_allocation_traps());

    assert!(!run_native_assembly(&output).success());
}

#[test]
fn shared_element_defaults_copy_and_optional_absence_have_exact_allocation_counts() {
    let source = concat!(
        "extern fn validate_counts() -> i64;\n",
        "class Item { marker: i64; init() { self.marker = 1; } }\n",
        "fn build() -> unit {\n",
        "  var inline: (shared Item)[] = (shared Item)[](3u);\n",
        "  var inline_optional: (shared? Item)[] = (shared? Item)[](3u);\n",
        "  var outer: shared (shared Item)[] = new (shared Item)[](2u);\n",
        "  var outer_optional: shared (shared? Item)[] = new (shared? Item)[](2u);\n",
        "  var inline_copy: (shared Item)[] = inline;\n",
        "  var outer_copy: shared (shared Item)[] = new (shared Item)[](copy *outer);\n",
        "  return;\n",
        "}\n",
        "fn main() -> i64 { build(); return validate_counts(); }\n",
    );
    let mut output = assembly(source);
    // Four outer backings, five default Item pointees, and two copied
    // backings. Optional shared defaults contribute no pointee allocations.
    output.push_str(&ownership_counter_probe(11));

    assert_eq!(run_native_assembly(&output).code(), Some(0));
}

#[test]
fn shared_element_default_construction_and_release_follow_index_order() {
    for (ty, construction) in [
        ("(shared Item)[]", "(shared Item)[](3u)"),
        ("shared (shared Item)[]", "new (shared Item)[](3u)"),
    ] {
        let source = format!(
            concat!(
                "extern fn next_marker() -> i64;\n",
                "extern fn observe(value: i64) -> unit;\n",
                "extern fn validate() -> i64;\n",
                "class Item {{\n",
                "  marker: i64;\n",
                "  init() {{ self.marker = next_marker(); }}\n",
                "  destroy {{ observe(self.marker); }}\n",
                "}}\n",
                "fn build() -> unit {{\n",
                "  var values: {ty} = {construction};\n",
                "  return;\n",
                "}}\n",
                "fn main() -> i64 {{ build(); return validate(); }}\n",
            ),
            ty = ty,
            construction = construction,
        );
        let mut output = assembly(&source);
        output.push_str(shared_element_trace_probe());

        assert_eq!(
            run_native_assembly(&output).code(),
            Some(0),
            "{construction}"
        );
    }
}

#[test]
fn nested_shared_array_elements_keep_outer_and_inner_ownership_independent() {
    for construction in [
        concat!(
            "var rows: (shared i64[])[] = (shared i64[])[](2u);\n",
            "  var copied: (shared i64[])[] = rows;\n",
        ),
        concat!(
            "var rows: shared (shared i64[])[] = new (shared i64[])[](2u);\n",
            "  var copied: shared (shared i64[])[] = new (shared i64[])[](copy *rows);\n",
        ),
    ] {
        let source = format!(
            concat!(
                "extern fn validate_counts() -> i64;\n",
                "fn build() -> unit {{\n",
                "  {construction}",
                "  return;\n",
                "}}\n",
                "fn main() -> i64 {{ build(); return validate_counts(); }}\n",
            ),
            construction = construction,
        );
        let mut output = assembly(&source);
        // One original outer backing, two distinct empty shared inner arrays,
        // and one copied outer backing. Copying retains inner owners.
        output.push_str(&ownership_counter_probe(4));

        assert_eq!(run_native_assembly(&output).code(), Some(0));
    }
}

fn native_allocator() -> &'static str {
    concat!(
        "\n.text\n",
        ".globl ska_rt_alloc\n",
        ".type ska_rt_alloc, @function\n",
        "ska_rt_alloc:\n",
        "    jmp malloc@PLT\n",
        ".size ska_rt_alloc, .-ska_rt_alloc\n",
        ".globl ska_rt_free\n",
        ".type ska_rt_free, @function\n",
        "ska_rt_free:\n",
        "    jmp free@PLT\n",
        ".size ska_rt_free, .-ska_rt_free\n",
    )
}

fn second_allocation_traps() -> &'static str {
    concat!(
        "\n.bss\n",
        ".p2align 3\n",
        ".Lslice_allocation_count: .quad 0\n",
        "\n.text\n",
        ".globl ska_rt_alloc\n",
        ".type ska_rt_alloc, @function\n",
        "ska_rt_alloc:\n",
        "    add qword ptr [rip + .Lslice_allocation_count], 1\n",
        "    cmp qword ptr [rip + .Lslice_allocation_count], 2\n",
        "    je .Lslice_allocation_failure\n",
        "    jmp malloc@PLT\n",
        ".Lslice_allocation_failure:\n",
        "    ud2\n",
        ".size ska_rt_alloc, .-ska_rt_alloc\n",
        ".globl ska_rt_free\n",
        ".type ska_rt_free, @function\n",
        "ska_rt_free:\n",
        "    jmp free@PLT\n",
        ".size ska_rt_free, .-ska_rt_free\n",
    )
}

fn shared_element_trace_probe() -> &'static str {
    concat!(
        "\n.bss\n",
        ".p2align 3\n",
        ".Lshared_element_next: .quad 0\n",
        ".Lshared_element_trace: .quad 0\n",
        ".Lshared_element_allocations: .quad 0\n",
        ".Lshared_element_frees: .quad 0\n",
        "\n.text\n",
        ".globl next_marker\n",
        ".type next_marker, @function\n",
        "next_marker:\n",
        "    add qword ptr [rip + .Lshared_element_next], 1\n",
        "    mov rax, qword ptr [rip + .Lshared_element_next]\n",
        "    ret\n",
        ".size next_marker, .-next_marker\n",
        ".globl observe\n",
        ".type observe, @function\n",
        "observe:\n",
        "    imul rax, qword ptr [rip + .Lshared_element_trace], 10\n",
        "    add rax, rdi\n",
        "    mov qword ptr [rip + .Lshared_element_trace], rax\n",
        "    ret\n",
        ".size observe, .-observe\n",
        ".globl ska_rt_alloc\n",
        ".type ska_rt_alloc, @function\n",
        "ska_rt_alloc:\n",
        "    push rbp\n",
        "    mov rbp, rsp\n",
        "    add qword ptr [rip + .Lshared_element_allocations], 1\n",
        "    call malloc@PLT\n",
        "    leave\n",
        "    ret\n",
        ".size ska_rt_alloc, .-ska_rt_alloc\n",
        ".globl ska_rt_free\n",
        ".type ska_rt_free, @function\n",
        "ska_rt_free:\n",
        "    add qword ptr [rip + .Lshared_element_frees], 1\n",
        "    jmp free@PLT\n",
        ".size ska_rt_free, .-ska_rt_free\n",
        ".globl validate\n",
        ".type validate, @function\n",
        "validate:\n",
        "    cmp qword ptr [rip + .Lshared_element_next], 3\n",
        "    jne .Lshared_element_failure\n",
        "    cmp qword ptr [rip + .Lshared_element_trace], 321\n",
        "    jne .Lshared_element_failure\n",
        "    cmp qword ptr [rip + .Lshared_element_allocations], 4\n",
        "    jne .Lshared_element_failure\n",
        "    cmp qword ptr [rip + .Lshared_element_frees], 4\n",
        "    jne .Lshared_element_failure\n",
        "    mov rax, 0\n",
        "    ret\n",
        ".Lshared_element_failure:\n",
        "    mov rax, 1\n",
        "    ret\n",
        ".size validate, .-validate\n",
    )
}

fn ownership_counter_probe(expected: u64) -> String {
    format!(
        concat!(
            "\n.bss\n",
            ".p2align 3\n",
            ".Lownership_allocations: .quad 0\n",
            ".Lownership_frees: .quad 0\n",
            "\n.text\n",
            ".globl ska_rt_alloc\n",
            ".type ska_rt_alloc, @function\n",
            "ska_rt_alloc:\n",
            "    push rbp\n",
            "    mov rbp, rsp\n",
            "    add qword ptr [rip + .Lownership_allocations], 1\n",
            "    call malloc@PLT\n",
            "    leave\n",
            "    ret\n",
            ".size ska_rt_alloc, .-ska_rt_alloc\n",
            ".globl ska_rt_free\n",
            ".type ska_rt_free, @function\n",
            "ska_rt_free:\n",
            "    add qword ptr [rip + .Lownership_frees], 1\n",
            "    jmp free@PLT\n",
            ".size ska_rt_free, .-ska_rt_free\n",
            ".globl validate_counts\n",
            ".type validate_counts, @function\n",
            "validate_counts:\n",
            "    cmp qword ptr [rip + .Lownership_allocations], {expected}\n",
            "    jne .Lownership_failure\n",
            "    cmp qword ptr [rip + .Lownership_frees], {expected}\n",
            "    jne .Lownership_failure\n",
            "    mov rax, 0\n",
            "    ret\n",
            ".Lownership_failure:\n",
            "    mov rax, 1\n",
            "    ret\n",
            ".size validate_counts, .-validate_counts\n",
        ),
        expected = expected,
    )
}

fn shared_array_trace_probe() -> &'static str {
    concat!(
        "\n.bss\n",
        ".p2align 3\n",
        ".Lshared_trace: .quad 0\n",
        ".Lshared_allocations: .quad 0\n",
        ".Lshared_frees: .quad 0\n",
        "\n.text\n",
        ".globl observe\n",
        ".type observe, @function\n",
        "observe:\n",
        "    imul rax, qword ptr [rip + .Lshared_trace], 10\n",
        "    add rax, rdi\n",
        "    mov qword ptr [rip + .Lshared_trace], rax\n",
        "    ret\n",
        ".size observe, .-observe\n",
        ".globl ska_rt_alloc\n",
        ".type ska_rt_alloc, @function\n",
        "ska_rt_alloc:\n",
        "    push rbp\n",
        "    mov rbp, rsp\n",
        "    add qword ptr [rip + .Lshared_allocations], 1\n",
        "    call malloc@PLT\n",
        "    leave\n",
        "    ret\n",
        ".size ska_rt_alloc, .-ska_rt_alloc\n",
        ".globl ska_rt_free\n",
        ".type ska_rt_free, @function\n",
        "ska_rt_free:\n",
        "    add qword ptr [rip + .Lshared_frees], 1\n",
        "    jmp free@PLT\n",
        ".size ska_rt_free, .-ska_rt_free\n",
        ".globl validate\n",
        ".type validate, @function\n",
        "validate:\n",
        "    cmp qword ptr [rip + .Lshared_trace], 321\n",
        "    jne .Lshared_failure\n",
        "    cmp qword ptr [rip + .Lshared_allocations], 1\n",
        "    jne .Lshared_failure\n",
        "    cmp qword ptr [rip + .Lshared_frees], 1\n",
        "    jne .Lshared_failure\n",
        "    mov rax, 0\n",
        "    ret\n",
        ".Lshared_failure:\n",
        "    mov rax, 1\n",
        "    ret\n",
        ".size validate, .-validate\n",
    )
}

fn array_runtime_probe(length: u64, byte_elements: bool) -> String {
    let expected_allocations = u64::from(length != 0);
    let element_checks = if length == 0 {
        String::new()
    } else if byte_elements {
        (0..length)
            .map(|index| {
                format!(
                    "    cmp byte ptr [rdx + {}], 0\n    jne .Lprobe_fail\n",
                    16 + index
                )
            })
            .collect()
    } else {
        (0..length)
            .map(|index| {
                format!(
                    "    cmp qword ptr [rdx + {}], 0\n    jne .Lprobe_fail\n",
                    16 + index * 8
                )
            })
            .collect()
    };
    let live_pointer_check = if length == 0 {
        "    cmp qword ptr [rip + .Lprobe_allocation], 0\n    jne .Lprobe_fail\n".to_owned()
    } else {
        format!(
            concat!(
                "    mov rdx, qword ptr [rip + .Lprobe_allocation]\n",
                "    test rdx, rdx\n",
                "    je .Lprobe_fail\n",
                "    cmp qword ptr [rdx], 1\n",
                "    jne .Lprobe_fail\n",
                "    cmp qword ptr [rdx + 8], {length}\n",
                "    jne .Lprobe_fail\n",
                "{element_checks}",
            ),
            length = length,
            element_checks = element_checks
        )
    };
    format!(
        concat!(
            "\n.bss\n",
            ".p2align 3\n",
            ".Lprobe_allocation: .quad 0\n",
            ".Lprobe_allocations: .quad 0\n",
            ".Lprobe_frees: .quad 0\n",
            ".Lprobe_failed: .quad 0\n",
            "\n.text\n",
            ".globl ska_rt_alloc\n",
            ".type ska_rt_alloc, @function\n",
            "ska_rt_alloc:\n",
            "    push rbp\n",
            "    mov rbp, rsp\n",
            "    add qword ptr [rip + .Lprobe_allocations], 1\n",
            "    call malloc@PLT\n",
            "    mov qword ptr [rip + .Lprobe_allocation], rax\n",
            "    leave\n",
            "    ret\n",
            ".size ska_rt_alloc, .-ska_rt_alloc\n",
            ".globl ska_rt_free\n",
            ".type ska_rt_free, @function\n",
            "ska_rt_free:\n",
            "    add qword ptr [rip + .Lprobe_frees], 1\n",
            "    jmp free@PLT\n",
            ".size ska_rt_free, .-ska_rt_free\n",
            ".globl validate_live\n",
            ".type validate_live, @function\n",
            "validate_live:\n",
            "    cmp rdi, {length}\n",
            "    jne .Lprobe_fail\n",
            "{live_pointer_check}",
            "    ret\n",
            ".Lprobe_fail:\n",
            "    mov qword ptr [rip + .Lprobe_failed], 1\n",
            "    ret\n",
            ".size validate_live, .-validate_live\n",
            ".globl validate_counts\n",
            ".type validate_counts, @function\n",
            "validate_counts:\n",
            "    mov rax, qword ptr [rip + .Lprobe_allocations]\n",
            "    cmp rax, {expected_allocations}\n",
            "    jne .Lprobe_counts_fail\n",
            "    mov rax, qword ptr [rip + .Lprobe_frees]\n",
            "    cmp rax, {expected_allocations}\n",
            "    jne .Lprobe_counts_fail\n",
            "    mov rax, qword ptr [rip + .Lprobe_failed]\n",
            "    ret\n",
            ".Lprobe_counts_fail:\n",
            "    mov rax, 1\n",
            "    ret\n",
            ".size validate_counts, .-validate_counts\n",
        ),
        length = length,
        live_pointer_check = live_pointer_check,
        expected_allocations = expected_allocations,
    )
}
