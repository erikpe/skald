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
fn lifecycle_payloads_independent_copy_and_polymorphic_owner_views_are_native() {
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
    let output = emit_assembly(Target::X86_64SysV, &polymorphic).unwrap();
    assert!(
        output.contains(".Lska_optional_box_0_metadata:"),
        "{output}"
    );
}

#[test]
fn polymorphic_optional_boxes_dispatch_cast_and_unwrap_natively() {
    let source = concat!(
        "interface Marker { fn mark() -> i64; }\n",
        "class Base {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  virtual fn mark() -> i64 { return self.value; }\n",
        "}\n",
        "class Derived extends Base implements Marker {\n",
        "  init() { super(1); }\n",
        "  override fn mark() -> i64 { return self.value + 6; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var exact: shared Derived?? = new Derived??(some(Derived()));\n",
        "  var base: shared Base?? = exact;\n",
        "  var marker: shared Marker?? = exact;\n",
        "  var object: shared Obj?? = exact;\n",
        "  var absent_exact: shared Derived?? = new Derived??();\n",
        "  var absent_marker: shared Marker?? = absent_exact;\n",
        "  if ((*marker) is none) { return 88; }\n",
        "  if ((*object) is none) { return 89; }\n",
        "  if ((*absent_marker) is some) { return 91; }\n",
        "  if (!(((*object)!)! is Derived)) { return 90; }\n",
        "  var down: shared Derived?? = (shared Derived) base;\n",
        "  ((*base)!)!.value = 2;\n",
        "  return ((*base)!)!.mark() + ((*marker)!)!.mark() + ((*down)!)!.mark();\n",
        "}\n",
    );
    let mut output = assembly(source);
    assert!(
        output.contains(".Lska_optional_box_0_metadata:"),
        "{output}"
    );
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(24), "{output}");
}

#[test]
fn absent_polymorphic_box_unwrap_reports_the_canonical_native_failure() {
    let mut output = assembly(concat!(
        "interface Marker { fn mark() -> i64; }\n",
        "class Value implements Marker {\n",
        "  init() {}\n",
        "  fn mark() -> i64 { return 1; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var exact: shared Value? = new Value?();\n",
        "  var marker: shared Marker? = exact;\n",
        "  return (*marker)!.mark();\n",
        "}\n",
    ));
    output.push_str(native_allocator());
    output.push_str(native_panic_reporter());
    let result = run_native_assembly_output(&output);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(result.stderr, b"panic: optional value is absent\n");
}

#[test]
fn failed_polymorphic_optional_box_downcast_reports_the_canonical_failure() {
    let mut output = assembly(concat!(
        "class Base { init() {} }\n",
        "class Left extends Base { init() { super(); } }\n",
        "class Right extends Base { init() { super(); } }\n",
        "fn main() -> i64 {\n",
        "  var box: shared Base? = new Left?(Left());\n",
        "  var down: shared Right? = (shared Right) box;\n",
        "  if ((*down)! is Right) { return 1; }\n",
        "  return 0;\n",
        "}\n",
    ));
    output.push_str(native_allocator());
    output.push_str(native_panic_reporter());
    let result = run_native_assembly_output(&output);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(result.stderr, b"panic: checked object cast failed\n");
}

#[test]
fn polymorphic_optional_box_dispatch_survives_recursion_and_stack_pressure() {
    let source = concat!(
        "class Base {\n",
        "  init() {}\n",
        "  virtual fn total(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64, h: i64) -> i64 {\n",
        "    return a + b + c + d + e + f + g + h;\n",
        "  }\n",
        "}\n",
        "class Derived extends Base {\n",
        "  init() { super(); }\n",
        "  override fn total(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64, h: i64) -> i64 {\n",
        "    return a + b + c + d + e + f + g + h + 2;\n",
        "  }\n",
        "}\n",
        "fn recurse(depth: i64) -> i64 {\n",
        "  var box: shared Base? = new Derived?(Derived());\n",
        "  var value: i64 = (*box)!.total(1, 1, 1, 1, 1, 1, 1, 1);\n",
        "  if (depth == 0) { return value; }\n",
        "  return value + recurse(depth - 1);\n",
        "}\n",
        "fn main() -> i64 { return recurse(2); }\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(30), "{output}");
}

#[test]
fn polymorphic_box_copy_to_exact_optional_slices_deliberately() {
    let source = concat!(
        "class Base {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  virtual fn mark() -> i64 { return self.value; }\n",
        "}\n",
        "class Derived extends Base {\n",
        "  extra: i64;\n",
        "  init(value: i64) { super(value); self.extra = 100; }\n",
        "  override fn mark() -> i64 { return self.value + self.extra; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var exact: shared Derived? = new Derived?(Derived(2));\n",
        "  var view: shared Base? = exact;\n",
        "  var sliced: shared Base? = new Base?(*view);\n",
        "  return (*view)!.mark() + (*sliced)!.mark();\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(104), "{output}");
}

#[test]
fn exact_box_observers_and_contained_mutation_execute_natively() {
    let source = concat!(
        "class Value {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  mut fn set(value: i64) -> unit { self.value = value; }\n",
        "}\n",
        "fn inspect(ref value: i64?) -> bool { return value is some; }\n",
        "fn main() -> i64 {\n",
        "  var number: shared i64? = new i64?(20);\n",
        "  if ((*number) is none) { return 1; }\n",
        "  if (!inspect(*number)) { return 2; }\n",
        "  var copied: i64? = *number;\n",
        "  var produced: i64 = (*(new i64?(4)))!;\n",
        "  var maybe: shared? i64? = new i64?(5);\n",
        "  var outer: i64 = (*(maybe!))!;\n",
        "  var nested: shared i64?? = new i64??(some(some(3)));\n",
        "  var inner: i64? = (*nested)!;\n",
        "  var values: shared i64[]? = new i64[]?(i64[]{2});\n",
        "  var array: i64[] = (*values)!;\n",
        "  var object: shared Value? = new Value?(Value(1));\n",
        "  (*object)!.set(5);\n",
        "  var shared_value: shared Value = new Value(6);\n",
        "  var owner_box: shared (shared Value)? = new (shared Value)?(shared_value);\n",
        "  var extracted: shared Value = (*owner_box)!;\n",
        "  return copied! + produced + outer + inner! + array[0] + (*object)!.value + extracted->value;\n",
        "}\n",
    );
    let mut output = assembly(source);
    assert!(output.contains("lea rdi, [rdi + 16]"), "{output}");
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(45), "{output}");
}

#[test]
fn absent_exact_box_unwrap_reports_the_canonical_native_failure() {
    let mut output = assembly(concat!(
        "fn main() -> i64 {\n",
        "  var box: shared i64? = new i64?();\n",
        "  return (*box)!;\n",
        "}\n",
    ));
    output.push_str(native_allocator());
    output.push_str(native_panic_reporter());
    let result = run_native_assembly_output(&output);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(result.stderr, b"panic: optional value is absent\n");
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
