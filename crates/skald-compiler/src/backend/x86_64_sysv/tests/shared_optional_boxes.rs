use super::*;

const ALL_PRIMITIVE_BOXES: &str = concat!(
    "fn main() -> i64 {\n",
    "  var signed: shared i64? = new i64?(1);\n",
    "  var unsigned: shared u64? = new u64?(2u);\n",
    "  var byte: shared u8? = new u8?(3u8);\n",
    "  var floating: shared f64? = new f64?(4.0);\n",
    "  var truth: shared bool? = new bool?(true);\n",
    "  var absent: shared i64? = new i64?();\n",
    "  return 0;\n",
    "}\n",
);

#[test]
fn primitive_boxes_have_checked_layout_and_distinct_exact_descriptors() {
    let first = assembly(ALL_PRIMITIVE_BOXES);
    let second = assembly(ALL_PRIMITIVE_BOXES);

    assert_eq!(first, second, "optional-box assembly must be deterministic");
    assert_eq!(
        first.matches("mov rdi, 32\n    call ska_rt_alloc").count(),
        6
    );
    for index in 0..5 {
        let descriptor = format!(".Lska_optional_box_{index}_metadata:");
        let finalizer = format!(".Lska_optional_box_{index}_finalize");
        assert_eq!(first.matches(&descriptor).count(), 1, "{first}");
        assert_eq!(first.matches(&format!(".quad {finalizer}")).count(), 1);
        let body = function_assembly(&first, &finalizer);
        assert!(body.contains("    ret"), "{body}");
        assert!(
            !body.contains("call "),
            "primitive finalizer must be a no-op: {body}"
        );
    }
    for runtime_call in first
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("call ska_rt_"))
    {
        assert!(
            matches!(
                runtime_call,
                "call ska_rt_abi_v9"
                    | "call ska_rt_alloc"
                    | "call ska_rt_free"
                    | "call ska_rt_panic"
            ),
            "primitive boxes must not add a runtime ABI entry: {runtime_call}"
        );
    }
    let runtime_header = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../runtime/include/skald_runtime.h"),
    )
    .unwrap();
    assert!(runtime_header.contains("#define SKALD_RUNTIME_ABI_VERSION UINT64_C(9)"));
    assert!(runtime_header.contains("#define SKALD_RUNTIME_ABI_MARKER ska_rt_abi_v9"));
    assert!(!runtime_header.contains("optional_box"));
    assert_system_assembler_accepts(&first);
}

#[test]
fn owner_replacement_keeps_the_old_box_alive_and_frees_exact_bases_once() {
    let mut output = assembly(concat!(
        "extern fn checkpoint(allocations: i64, frees: i64) -> unit;\n",
        "fn main() -> i64 {\n",
        "  {\n",
        "    var first: shared i64? = new i64?(1);\n",
        "    {\n",
        "      var alias: shared i64? = first;\n",
        "      first = new i64?(2);\n",
        "      checkpoint(2, 0);\n",
        "    }\n",
        "    checkpoint(2, 1);\n",
        "  }\n",
        "  checkpoint(2, 2);\n",
        "  return 0;\n",
        "}\n",
    ));
    output.push_str(exact_base_allocator_probe());

    let result = run_native_assembly_output(&output);
    assert_eq!(
        result.status.code(),
        Some(0),
        "box lifetime probe failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
fn allocation_failure_uses_the_common_runtime_trace_boundary() {
    let fixture = crate::test_support::lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "extern fn ska_test_fail_next_allocation() -> unit;\n",
            "fn main() -> i64 {\n",
            "  ska_test_fail_next_allocation();\n",
            "  var value: shared bool? = new bool?(true);\n",
            "  return 0;\n",
            "}\n",
        ),
    );
    let output = fixture
        .emit_assembly(
            Target::X86_64SysV,
            crate::backend::RuntimeTracePolicy::Enabled,
        )
        .unwrap();

    let result = crate::test_support::run_native_assembly_with_runtime_trace_probe(&output);
    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("memory allocation failed"), "{stderr}");
    assert!(stderr.contains("app/main.ska:4:"), "{stderr}");
}

#[test]
fn lifecycle_payloads_and_independent_copy_are_native_while_views_remain_gated() {
    let aggregate = lower_source_to_final_mir(concat!(
        "class Value { init() {} copy(ref source: Value) {} }\n",
        "fn main() -> i64 {\n",
        "  var value: shared Value? = new Value?(Value());\n",
        "  var copied: shared Value? = new Value?(*value);\n",
        "  return 0;\n",
        "}\n",
    ));
    let output = emit_assembly(Target::X86_64SysV, &aggregate).unwrap();
    assert_eq!(output.matches("call ska_rt_alloc").count(), 2, "{output}");
    assert!(
        output.contains(".Lska_optional_box_0_finalize:"),
        "{output}"
    );

    let polymorphic = lower_source_to_final_mir(concat!(
        "class Base { init() {} }\n",
        "class Derived extends Base { init() { super(); } }\n",
        "fn main() -> i64 {\n",
        "  var exact: shared Derived? = new Derived?();\n",
        "  var view: shared Base? = exact;\n",
        "  return 0;\n",
        "}\n",
    ));
    let error = emit_assembly(Target::X86_64SysV, &polymorphic).unwrap_err();
    assert!(error
        .message()
        .contains("polymorphic shared optional-box views are not yet supported"));
}

#[test]
fn exact_class_box_copy_uses_independent_storage_and_finalizes_each_payload() {
    let source = concat!(
        "class Tracked {\n",
        "  private static destroyed_count: i64;\n",
        "  init() {}\n",
        "  copy(ref source: Tracked) {}\n",
        "  destroy { Tracked.destroyed_count = Tracked.destroyed_count + 1; }\n",
        "  static fn destroyed() -> i64 { return Tracked.destroyed_count; }\n",
        "}\n",
        "fn build() -> unit {\n",
        "  var source: shared Tracked? = new Tracked?(Tracked());\n",
        "  var copied: shared Tracked? = new Tracked?(*source);\n",
        "  return;\n",
        "}\n",
        "fn main() -> i64 { build(); return Tracked.destroyed(); }\n",
    );
    let mut output = assembly(source);
    assert_eq!(output.matches("call ska_rt_alloc").count(), 2, "{output}");
    let finalizer = function_assembly(&output, ".Lska_optional_box_0_finalize");
    assert!(finalizer.contains(".destroy."), "{finalizer}");
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(2), "{output}");
}

#[test]
fn nested_box_finalizers_cover_every_presence_shape_at_depth_five() {
    let source = concat!(
        "class Tracked {\n",
        "  private static destroyed_count: i64;\n",
        "  init() {}\n",
        "  destroy { Tracked.destroyed_count = Tracked.destroyed_count + 1; }\n",
        "  static fn destroyed() -> i64 { return Tracked.destroyed_count; }\n",
        "}\n",
        "fn build() -> unit {\n",
        "  var absent0: shared Tracked????? = new Tracked?????();\n",
        "  var absent1: shared Tracked????? = new Tracked?????(some(none));\n",
        "  var absent2: shared Tracked????? = new Tracked?????(some(some(none)));\n",
        "  var absent3: shared Tracked????? = new Tracked?????(some(some(some(none))));\n",
        "  var absent4: shared Tracked????? = new Tracked?????(some(some(some(some(none)))));\n",
        "  var present: shared Tracked????? = new Tracked?????(some(some(some(some(some(Tracked()))))));\n",
        "  return;\n",
        "}\n",
        "fn main() -> i64 { build(); return Tracked.destroyed(); }\n",
    );
    let mut output = assembly(source);
    let finalizer = function_assembly(&output, ".Lska_optional_box_0_finalize");
    assert!(finalizer.matches("finalize_nested_optional").count() >= 4);
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(1), "{output}");
}

#[test]
fn optional_array_and_inner_owner_boxes_release_their_nested_resources() {
    let source = concat!(
        "class Tracked {\n",
        "  private static destroyed_count: i64;\n",
        "  init() {}\n",
        "  destroy { Tracked.destroyed_count = Tracked.destroyed_count + 1; }\n",
        "  static fn destroyed() -> i64 { return Tracked.destroyed_count; }\n",
        "}\n",
        "fn build() -> unit {\n",
        "  var absent_array: shared i64[]? = new i64[]?();\n",
        "  var empty_array: shared i64[]? = new i64[]?(i64[]{});\n",
        "  var values: shared i64[]? = new i64[]?(i64[]{1, 2});\n",
        "  var inner: shared Tracked = new Tracked();\n",
        "  var owner_box: shared (shared Tracked)? = new (shared Tracked)?(inner);\n",
        "  return;\n",
        "}\n",
        "fn main() -> i64 { build(); return Tracked.destroyed(); }\n",
    );
    let mut output = assembly(source);
    assert!(
        output.contains("call .Lska_array_0_release"),
        "optional-array finalizer must use the canonical array release helper: {output}"
    );
    assert!(output.contains("nested_owner_release"), "{output}");
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(1), "{output}");
}

fn exact_base_allocator_probe() -> &'static str {
    concat!(
        "\n.bss\n",
        ".p2align 4\n",
        ".Lbox_block_0: .zero 32\n",
        ".Lbox_block_1: .zero 32\n",
        "\n.data\n",
        ".p2align 3\n",
        ".Lbox_allocations: .quad 0\n",
        ".Lbox_frees: .quad 0\n",
        ".Lbox_freed_0: .quad 0\n",
        ".Lbox_freed_1: .quad 0\n",
        "\n.text\n",
        ".globl ska_rt_alloc\n",
        ".type ska_rt_alloc, @function\n",
        "ska_rt_alloc:\n",
        "    cmp rdi, 32\n",
        "    jne .Lbox_failure\n",
        "    mov rax, qword ptr [rip + .Lbox_allocations]\n",
        "    cmp rax, 0\n",
        "    je .Lbox_allocate_0\n",
        "    cmp rax, 1\n",
        "    jne .Lbox_failure\n",
        "    lea rax, [rip + .Lbox_block_1]\n",
        "    jmp .Lbox_allocation_ready\n",
        ".Lbox_allocate_0:\n",
        "    lea rax, [rip + .Lbox_block_0]\n",
        ".Lbox_allocation_ready:\n",
        "    inc qword ptr [rip + .Lbox_allocations]\n",
        "    ret\n",
        ".size ska_rt_alloc, .-ska_rt_alloc\n",
        ".globl ska_rt_free\n",
        ".type ska_rt_free, @function\n",
        "ska_rt_free:\n",
        "    lea rax, [rip + .Lbox_block_0]\n",
        "    cmp rdi, rax\n",
        "    je .Lbox_free_0\n",
        "    lea rax, [rip + .Lbox_block_1]\n",
        "    cmp rdi, rax\n",
        "    jne .Lbox_failure\n",
        "    cmp qword ptr [rip + .Lbox_freed_1], 0\n",
        "    jne .Lbox_failure\n",
        "    inc qword ptr [rip + .Lbox_freed_1]\n",
        "    jmp .Lbox_free_ready\n",
        ".Lbox_free_0:\n",
        "    cmp qword ptr [rip + .Lbox_freed_0], 0\n",
        "    jne .Lbox_failure\n",
        "    inc qword ptr [rip + .Lbox_freed_0]\n",
        ".Lbox_free_ready:\n",
        "    inc qword ptr [rip + .Lbox_frees]\n",
        "    ret\n",
        ".size ska_rt_free, .-ska_rt_free\n",
        ".globl checkpoint\n",
        ".type checkpoint, @function\n",
        "checkpoint:\n",
        "    cmp qword ptr [rip + .Lbox_allocations], rdi\n",
        "    jne .Lbox_failure\n",
        "    cmp qword ptr [rip + .Lbox_frees], rsi\n",
        "    jne .Lbox_failure\n",
        "    ret\n",
        ".size checkpoint, .-checkpoint\n",
        ".Lbox_failure:\n",
        "    mov rax, 60\n",
        "    mov rdi, 97\n",
        "    syscall\n",
    )
}
