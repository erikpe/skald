use super::*;

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
    assert!(first.contains("mov qword ptr [rdi + rsi*8 + 16], rax"));
    assert!(first.contains(".Lska_array_1_initialize_element:"));
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
fn indexing_and_nonlocal_array_ownership_remain_structured_backend_errors() {
    for source in [
        concat!(
            "fn main() -> i64 {\n",
            "  var values: i64[] = i64[](1u);\n",
            "  return values[0];\n",
            "}\n",
        ),
        concat!(
            "fn make() -> i64[] { return i64[](1u); }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
    ] {
        let program = lower_text(source);
        let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
        assert!(
            error.to_string().contains("not yet supported")
                || error
                    .to_string()
                    .contains("outside the primitive inline-array"),
            "{error}"
        );
    }
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
