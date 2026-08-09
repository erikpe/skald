use super::*;

#[test]
fn emits_a_deterministic_minimal_function() {
    let source = "fn main() -> i64 { return 42; }";
    let expected = concat!(
        ".intel_syntax noprefix\n",
        ".text\n",
        ".p2align 4\n",
        ".type .Lska.fn.main.main.f0, @function\n",
        ".Lska.fn.main.main.f0:\n",
        "    push rbp\n",
        "    mov rbp, rsp\n",
        "    sub rsp, 16\n",
        ".Lska.fn.main.main.f0.block_0:\n",
        "    mov rax, 42\n",
        "    mov qword ptr [rbp - 8], rax\n",
        "    mov rax, qword ptr [rbp - 8]\n",
        "    jmp .Lska.fn.main.main.f0.epilogue\n",
        ".Lska.fn.main.main.f0.epilogue:\n",
        "    leave\n",
        "    ret\n",
        ".size .Lska.fn.main.main.f0, .-.Lska.fn.main.main.f0\n",
        "\n",
        ".p2align 4\n",
        ".globl main\n",
        ".type main, @function\n",
        "main:\n",
        "    push rbp\n",
        "    mov rbp, rsp\n",
        "    call ska_rt_abi_v9\n",
        "    call .Lska.fn.main.main.f0\n",
        "    leave\n",
        "    ret\n",
        ".size main, .-main\n",
        "\n",
        ".section .note.GNU-stack,\"\",@progbits\n",
    );

    let first = assembly(source);

    assert_eq!(first, expected);
    assert_eq!(first, assembly(source));
    assert!(!first.contains("ska_rt_io_"));
}

#[test]
fn generated_text_is_accepted_by_the_system_assembler() {
    let straight_line = assembly(concat!(
        "fn calculate(a: i64, b: i64) -> i64 { return -a * b + 3; }\n",
        "fn main() -> i64 { return calculate(6, 7); }",
    ));
    let multi_block = emit_assembly(Target::X86_64SysV, &branch_call_diamond_mir()).unwrap();
    let comparisons = assembly(concat!(
        "fn compare_i64(left: i64, right: i64) -> bool { return left < right; }\n",
        "fn compare_u64(left: u64, right: u64) -> bool { return left >= right; }\n",
        "fn compare_u8(left: u8, right: u8) -> bool { return left != right; }\n",
        "fn main() -> i64 { if (compare_i64(-1, 0)) { return 1; } return 0; }\n",
    ));
    let eager_booleans = emit_assembly(Target::X86_64SysV, &eager_boolean_program()).unwrap();
    let integer_bitwise = emit_assembly(Target::X86_64SysV, &integer_bitwise_program()).unwrap();
    let conditional_cleanup =
        emit_assembly(Target::X86_64SysV, &fixture_conditional_cleanup_program()).unwrap();
    let casts = assembly(concat!(
        "fn narrow(value: u64) -> u8 { return (u8) value; }\n",
        "fn widen(value: u8) -> i64 { return (i64) value; }\n",
        "fn reinterpret(value: i64) -> u64 { return (u64) value; }\n",
        "fn main() -> i64 { return widen(narrow(reinterpret(-1))); }\n",
    ));

    assert_system_assembler_accepts(&straight_line);
    assert_system_assembler_accepts(&multi_block);
    assert_system_assembler_accepts(&comparisons);
    assert_system_assembler_accepts(&eager_booleans);
    assert_system_assembler_accepts(&integer_bitwise);
    assert_system_assembler_accepts(&conditional_cleanup);
    assert_system_assembler_accepts(&casts);
}
