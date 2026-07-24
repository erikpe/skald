use super::*;

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
