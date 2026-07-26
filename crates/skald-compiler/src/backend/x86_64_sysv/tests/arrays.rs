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
    let item = layouts
        .array(array(MirType::Class(ClassId::new(0))))
        .unwrap();
    let optional = layouts
        .array(array(MirType::OptionalClass(ClassId::new(0))))
        .unwrap();
    let primitive = array(MirType::I64);
    let nested = layouts.array(array(MirType::Array(primitive))).unwrap();
    let node = layouts.class(ClassId::new(1)).unwrap();

    assert_eq!(
        item.stride(),
        layouts.class(ClassId::new(0)).unwrap().ty().size()
    );
    assert!(optional.stride() > item.stride());
    assert_eq!(nested.stride(), 8);
    assert_eq!(node.ty().size(), 8);
}

#[test]
fn primitive_inline_array_helpers_are_deterministic_and_layout_specialized() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  var wide: i64[] = i64[](3u);\n",
        "  var bytes: u8[] = u8[](3u);\n",
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
    assert!(first.contains("call ska_rt_abi_v5"));
    assert!(first.contains("call ska_rt_alloc"));
    assert!(first.contains("call ska_rt_free"));
    assert!(!first.contains("ska_rt_array"));
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

    assert!(output.contains(".Lska_class_0_copy_complete:"));
    assert!(output.contains(".Lska_array_0_destroy_element:"));
    assert!(output.contains(".Lska_array_3_clone:"));
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(1));
}

#[test]
fn deferred_array_profiles_remain_structured_backend_errors() {
    for source in [
        concat!(
            "fn length(ref values: i64[]) -> u64 { return values.len(); }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        concat!(
            "fn main() -> i64 {\n",
            "  var values: shared i64[] = new i64[](1u);\n",
            "  return 0;\n",
            "}\n",
        ),
    ] {
        let program = lower_text(source);
        let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
        assert!(
            error.to_string().contains("not yet supported")
                || error
                    .to_string()
                    .contains("outside the non-shared inline-array"),
            "{error}"
        );
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
