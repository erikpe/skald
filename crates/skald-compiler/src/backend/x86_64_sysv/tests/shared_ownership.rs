use super::*;

#[test]
fn shared_fields_are_one_word_aligned_edges_after_the_inline_base_prefix() {
    let program = lower_text(concat!(
        "class Item { init() {} }\n",
        "class Root { marker: u8; init() { self.marker = 1u8; } }\n",
        "class Holder extends Root {\n",
        "  first: shared Item;\n",
        "  flag: bool;\n",
        "  second: shared Item;\n",
        "  init(first: shared Item, second: shared Item) {\n",
        "    super(); self.first = first; self.flag = true; self.second = second;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let layout = super::super::layout::DataLayout::compute(&program).unwrap();
    let root = layout.class(ClassId::new(1)).unwrap();
    let holder = layout.class(ClassId::new(2)).unwrap();

    assert_eq!(root.ty().size(), 1);
    assert_eq!(holder.base().unwrap().offset, 0);
    assert_eq!(
        holder
            .field(FieldId::new(ClassId::new(2), 0))
            .unwrap()
            .offset,
        8
    );
    assert_eq!(
        holder
            .field(FieldId::new(ClassId::new(2), 1))
            .unwrap()
            .offset,
        16
    );
    assert_eq!(
        holder
            .field(FieldId::new(ClassId::new(2), 2))
            .unwrap()
            .offset,
        24
    );
    assert_eq!(holder.ty().size(), 32);
    assert_eq!(holder.ty().alignment(), 8);
}

#[test]
fn synthesized_shared_field_copy_and_self_assignment_lower_to_balanced_owners() {
    let output = assembly(concat!(
        "class Item { init() {} }\n",
        "class Holder {\n",
        "  owner: shared Item;\n",
        "  init(owner: shared Item) { self.owner = owner; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var first: Holder = Holder(new Item());\n",
        "  var second: Holder = first;\n",
        "  second = second;\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(output.contains("field_copy_construct"));
    assert!(output.contains("field_copy_assign_retain"));
    assert!(output.contains("field_copy_assign_release"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn mixed_inline_shared_inheritance_graph_cascades_in_language_order() {
    let mut output = assembly(concat!(
        "extern fn observe(value: i64) -> unit;\n",
        "extern fn verify() -> unit;\n",
        "class Trace {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  destroy { observe(self.value); }\n",
        "}\n",
        "class Inner {\n",
        "  child: shared Trace;\n",
        "  init(child: shared Trace) { self.child = child; }\n",
        "  destroy { observe(30); }\n",
        "}\n",
        "class Root {\n",
        "  root: shared Trace;\n",
        "  init(root: shared Trace) { self.root = root; }\n",
        "  destroy { observe(40); }\n",
        "}\n",
        "class Holder extends Root {\n",
        "  inner: Inner;\n",
        "  leaf: shared Trace;\n",
        "  init(root: shared Trace, child: shared Trace, leaf: shared Trace) {\n",
        "    super(root); self.inner = Inner(child); self.leaf = leaf;\n",
        "  }\n",
        "  destroy { observe(50); }\n",
        "  mut fn replace(value: shared Trace) -> unit { self.leaf = value; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  {\n",
        "    var holder: Holder = Holder(new Trace(1), new Trace(2), new Trace(3));\n",
        "    holder.replace(holder.leaf);\n",
        "    holder.replace(new Trace(4));\n",
        "  }\n",
        "  verify();\n",
        "  return 0;\n",
        "}\n",
    ));
    output.push_str(native_graph_stubs());

    let result = run_native_assembly_output(&output);
    assert!(
        result.status.success(),
        "shared-field graph failed with {:?}: {}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

const DERIVED_SHARED_SOURCE: &str = concat!(
    "extern fn observe(value: i64) -> unit;\n",
    "class Root {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  destroy { observe(100 + self.value); }\n",
    "}\n",
    "class Leaf extends Root {\n",
    "  extra: i64;\n",
    "  init(value: i64, extra: i64) { super(value); self.extra = extra; }\n",
    "  destroy { observe(200 + self.extra); }\n",
    "}\n",
    "fn main() -> i64 {\n",
    "  var value: shared Leaf = new Leaf(5, 7);\n",
    "  return 0;\n",
    "}\n",
);

#[test]
fn lowers_the_frozen_handle_header_and_runtime_call_contract() {
    let output = assembly(DERIVED_SHARED_SOURCE);

    assert!(output.contains("mov rdi, 32\n    call ska_rt_alloc"));
    assert!(output.contains("mov qword ptr [r11 + 8], rax"));
    assert!(output.contains("mov qword ptr [r11], rax"));
    assert!(output.contains("lea rdi, [rax + 16]\n    call r11"));
    assert!(output.contains("call ska_rt_free"));
    assert!(output.contains(".Lska_class_1_dispatch:\n    .quad .Lska_class_1_finalize_complete"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn generated_dynamic_finalizer_executes_derived_then_base_and_frees_once() {
    let mut output = assembly(DERIVED_SHARED_SOURCE);
    output.push_str(native_ownership_stubs());

    let result = run_native_assembly_output(&output);
    assert!(
        result.status.success(),
        "shared lifetime failed with {:?}: {}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
fn verified_copy_fixture_contains_checked_count_overflow_termination() {
    let mut program = lower_text(concat!(
        "class Widget { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var source: shared Widget = new Widget();\n",
        "  return 0;\n",
        "}\n",
    ));
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let source = function
        .storage
        .iter()
        .find(|storage| matches!(storage.ty, MirType::Shared(_)))
        .unwrap()
        .id;
    let destination = StorageId::new(function.function, function.storage.len());
    function.storage.push(MirStorage {
        id: destination,
        source: Some(BindingId::Local(LocalId::new(function.function, 1))),
        name: "copy-fixture".to_owned(),
        kind: MirStorageKind::Local,
        ty: MirType::Shared(MirSharedTarget::Class(ClassId::new(0))),
        span: function.span,
    });
    let release = function.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
        .unwrap();
    function.body.blocks[0].instructions.splice(
        release..release,
        [
            MirInstruction::SharedCopy(MirSharedCopy {
                destination,
                source,
                span: function.span,
            }),
            MirInstruction::EndFullExpression(MirEndFullExpression {
                temporaries: vec![],
                span: function.span,
            }),
            MirInstruction::SharedRelease(MirSharedRelease {
                owner: destination,
                span: function.span,
            }),
        ],
    );
    verify_mir(&program).expect("the retain fixture must remain valid MIR");

    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert!(output.contains("mov r11, 0xffffffffffffffff"));
    assert!(output.contains("ownership_retain_invalid"));
    assert!(output.contains("ownership_retain_invalid_"));
    assert!(output.contains("    ud2"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn shared_parameters_and_results_use_integer_arguments_and_rax_without_hidden_destination() {
    let output = assembly(concat!(
        "class Widget { init() {} }\n",
        "fn forward(value: shared Widget) -> shared Widget { return value; }\n",
        "fn main() -> i64 {\n",
        "  var source: shared Widget = new Widget();\n",
        "  var result: shared Widget = forward(source);\n",
        "  return 0;\n",
        "}\n",
    ));

    let forward = output
        .split(".Lska_fn_0:")
        .nth(1)
        .and_then(|tail| tail.split(".size .Lska_fn_0").next())
        .expect("forward function assembly");
    assert!(forward.contains("mov qword ptr [rbp - 16], rdi"));
    assert!(forward.contains("mov rax, qword ptr [rbp - 8]"));
    assert!(!forward.contains("mov qword ptr [rbp - 8], rdi"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn shared_views_execute_deep_virtual_interface_and_mutable_member_access() {
    let mut output = assembly(concat!(
        "interface Readable { fn read() -> i64; }\n",
        "class Root implements Readable {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  virtual fn read() -> i64 { return self.value; }\n",
        "}\n",
        "class Middle extends Root { init(value: i64) { super(value); } }\n",
        "class Leaf extends Middle {\n",
        "  extra: i64;\n",
        "  init(value: i64, extra: i64) { super(value); self.extra = extra; }\n",
        "  override fn read() -> i64 { return self.value + self.extra; }\n",
        "  mut fn bump() -> i64 { self.value = self.value + 1; return self.value; }\n",
        "}\n",
        "fn bump(value: shared Leaf) -> i64 { return value.bump(); }\n",
        "fn main() -> i64 {\n",
        "  var leaf: shared Leaf = new Leaf(10, 5);\n",
        "  var root: shared Root = leaf;\n",
        "  var readable: shared Readable = leaf;\n",
        "  var erased: shared Obj = leaf;\n",
        "  var bumped: i64 = bump(leaf);\n",
        "  if (root is Leaf) {\n",
        "    if (readable is Leaf) {\n",
        "      if (erased is Leaf) { return bumped + root.read() + readable.read(); }\n",
        "    }\n",
        "  }\n",
        "  return 1;\n",
        "}\n",
    ));
    output.push_str(simple_ownership_stubs());

    let status = run_native_assembly(&output);
    assert_eq!(status.code(), Some(43));
}

#[test]
fn shared_casts_execute_named_retain_produced_transfer_and_cross_view_checks() {
    let mut output = assembly(concat!(
        "interface Left { fn value() -> i64; }\n",
        "interface Right { fn value() -> i64; }\n",
        "class Root { init() {} virtual fn value() -> i64 { return 1; } }\n",
        "class Leaf extends Root implements Left, Right {\n",
        "  init() { super(); }\n",
        "  override fn value() -> i64 { return 7; }\n",
        "}\n",
        "class LeftOnly implements Left {\n",
        "  init() {}\n",
        "  fn value() -> i64 { return 3; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var erased: shared Obj = new Leaf();\n",
        "  var leaf: shared Leaf = (shared Leaf) erased;\n",
        "  var left: shared Left = leaf;\n",
        "  var right: shared Right = (shared Right) left;\n",
        "  var root: shared Root = (shared Root) new Leaf();\n",
        "  return leaf.value() + right.value() + root.value();\n",
        "}\n",
    ));
    output.push_str(simple_ownership_stubs());
    assert_eq!(run_native_assembly(&output).code(), Some(21));
}

#[test]
fn failed_shared_cast_terminates_before_establishing_a_result_owner() {
    let mut output = assembly(concat!(
        "class Root { init() {} }\n",
        "class Leaf extends Root { init() { super(); } }\n",
        "class Other { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var erased: shared Obj = new Other();\n",
        "  var leaf: shared Leaf = (shared Leaf) erased;\n",
        "  return 0;\n",
        "}\n",
    ));
    output.push_str(simple_ownership_stubs());
    assert!(!run_native_assembly(&output).success());
}

#[test]
fn intentional_strong_cycle_leaks_while_acyclic_controls_finalize() {
    let mut output = assembly(concat!(
        "extern fn observe(value: i64) -> unit;\n",
        "extern fn report() -> i64;\n",
        "class Seed {\n",
        "  init() {}\n",
        "  destroy { observe(10); }\n",
        "}\n",
        "class Node {\n",
        "  edge: shared Obj;\n",
        "  init(edge: shared Obj) { self.edge = edge; }\n",
        "  destroy { observe(1); }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  {\n",
        "    var seed: shared Obj = new Seed();\n",
        "    var left: shared Node = new Node(seed);\n",
        "    var right: shared Node = new Node(left);\n",
        "    left.edge = right;\n",
        "    var control: shared Seed = new Seed();\n",
        "  }\n",
        "  return report();\n",
        "}\n",
    ));
    output.push_str(cycle_observer_stubs());

    let status = run_native_assembly(&output);
    assert_eq!(status.code(), Some(0));
}

fn simple_ownership_stubs() -> &'static str {
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

fn cycle_observer_stubs() -> &'static str {
    concat!(
        "\n.bss\n",
        ".align 8\n",
        ".Lcycle_observe_count: .zero 8\n",
        ".Lcycle_observe_sum: .zero 8\n",
        ".Lcycle_free_count: .zero 8\n",
        "\n.text\n",
        ".globl observe\n",
        ".type observe, @function\n",
        "observe:\n",
        "    inc qword ptr [rip + .Lcycle_observe_count]\n",
        "    add qword ptr [rip + .Lcycle_observe_sum], rdi\n",
        "    ret\n",
        ".size observe, .-observe\n",
        ".globl report\n",
        ".type report, @function\n",
        "report:\n",
        "    mov rax, 1\n",
        "    cmp qword ptr [rip + .Lcycle_observe_count], 2\n",
        "    jne .Lcycle_report_done\n",
        "    cmp qword ptr [rip + .Lcycle_observe_sum], 20\n",
        "    jne .Lcycle_report_done\n",
        "    cmp qword ptr [rip + .Lcycle_free_count], 2\n",
        "    jne .Lcycle_report_done\n",
        "    xor rax, rax\n",
        ".Lcycle_report_done:\n",
        "    ret\n",
        ".size report, .-report\n",
        ".globl ska_rt_alloc\n",
        ".type ska_rt_alloc, @function\n",
        "ska_rt_alloc:\n",
        "    jmp malloc@PLT\n",
        ".size ska_rt_alloc, .-ska_rt_alloc\n",
        ".globl ska_rt_free\n",
        ".type ska_rt_free, @function\n",
        "ska_rt_free:\n",
        "    inc qword ptr [rip + .Lcycle_free_count]\n",
        "    jmp free@PLT\n",
        ".size ska_rt_free, .-ska_rt_free\n",
    )
}

fn native_ownership_stubs() -> &'static str {
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
        "    cmp qword ptr [rip + .Lobserve_count], 2\n",
        "    jne .Lownership_failure\n",
        "    cmp qword ptr [rip + .Lfree_count], 0\n",
        "    jne .Lownership_failure\n",
        "    inc qword ptr [rip + .Lfree_count]\n",
        "    jmp free@PLT\n",
        ".size ska_rt_free, .-ska_rt_free\n",
        ".globl observe\n",
        ".type observe, @function\n",
        "observe:\n",
        "    mov rax, qword ptr [rip + .Lobserve_count]\n",
        "    cmp rax, 0\n",
        "    jne .Lobserve_second\n",
        "    cmp rdi, 207\n",
        "    jne .Lownership_failure\n",
        "    inc qword ptr [rip + .Lobserve_count]\n",
        "    ret\n",
        ".Lobserve_second:\n",
        "    cmp rax, 1\n",
        "    jne .Lownership_failure\n",
        "    cmp rdi, 105\n",
        "    jne .Lownership_failure\n",
        "    inc qword ptr [rip + .Lobserve_count]\n",
        "    ret\n",
        ".Lownership_failure:\n",
        "    mov rax, 60\n",
        "    mov rdi, 99\n",
        "    syscall\n",
        ".size observe, .-observe\n",
        ".section .data\n",
        ".p2align 3\n",
        ".Lobserve_count:\n",
        "    .quad 0\n",
        ".Lfree_count:\n",
        "    .quad 0\n",
    )
}

fn native_graph_stubs() -> &'static str {
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
        "    inc qword ptr [rip + .Lgraph_free_count]\n",
        "    jmp free@PLT\n",
        ".size ska_rt_free, .-ska_rt_free\n",
        ".globl observe\n",
        ".type observe, @function\n",
        "observe:\n",
        "    mov rax, qword ptr [rip + .Lgraph_observe_count]\n",
        "    cmp rax, 7\n",
        "    jae .Lgraph_failure\n",
        "    lea rcx, [rip + .Lgraph_expected]\n",
        "    cmp rdi, qword ptr [rcx + rax * 8]\n",
        "    jne .Lgraph_failure\n",
        "    inc rax\n",
        "    mov qword ptr [rip + .Lgraph_observe_count], rax\n",
        ".Lgraph_observe_done:\n",
        "    ret\n",
        ".globl verify\n",
        ".type verify, @function\n",
        "verify:\n",
        "    cmp qword ptr [rip + .Lgraph_observe_count], 7\n",
        "    jne .Lgraph_failure\n",
        "    cmp qword ptr [rip + .Lgraph_free_count], 4\n",
        "    jne .Lgraph_failure\n",
        "    ret\n",
        ".size verify, .-verify\n",
        ".Lgraph_failure:\n",
        "    mov rax, 60\n",
        "    mov rdi, 98\n",
        "    syscall\n",
        ".size observe, .-observe\n",
        ".section .data\n",
        ".p2align 3\n",
        ".Lgraph_expected:\n",
        "    .quad 3, 50, 4, 30, 2, 40, 1\n",
        ".Lgraph_observe_count:\n",
        "    .quad 0\n",
        ".Lgraph_free_count:\n",
        "    .quad 0\n",
    )
}
