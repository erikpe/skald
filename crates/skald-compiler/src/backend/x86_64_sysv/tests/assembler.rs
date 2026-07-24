use super::*;

#[test]
fn emits_a_deterministic_minimal_function() {
    let source = "fn main() -> i64 { return 42; }";
    let expected = concat!(
        ".intel_syntax noprefix\n",
        ".text\n",
        ".p2align 4\n",
        ".type .Lska_fn_0, @function\n",
        ".Lska_fn_0:\n",
        "    push rbp\n",
        "    mov rbp, rsp\n",
        "    sub rsp, 16\n",
        ".Lska_fn_0_block_0:\n",
        "    mov rax, 42\n",
        "    mov qword ptr [rbp - 8], rax\n",
        "    mov rax, qword ptr [rbp - 8]\n",
        "    jmp .Lska_fn_0_epilogue\n",
        ".Lska_fn_0_epilogue:\n",
        "    leave\n",
        "    ret\n",
        ".size .Lska_fn_0, .-.Lska_fn_0\n",
        "\n",
        ".p2align 4\n",
        ".globl main\n",
        ".type main, @function\n",
        "main:\n",
        "    push rbp\n",
        "    mov rbp, rsp\n",
        "    call ska_rt_abi_v5\n",
        "    call .Lska_fn_0\n",
        "    leave\n",
        "    ret\n",
        ".size main, .-main\n",
        "\n",
        ".section .note.GNU-stack,\"\",@progbits\n",
    );

    assert_eq!(assembly(source), expected);
    assert_eq!(assembly(source), assembly(source));
}

#[test]
fn generated_text_is_accepted_by_the_system_assembler() {
    let straight_line = assembly(concat!(
        "fn calculate(a: i64, b: i64) -> i64 { return -a * b + 3; }\n",
        "fn main() -> i64 { return calculate(6, 7); }",
    ));
    let multi_block = emit_assembly(Target::X86_64SysV, &branch_call_diamond_mir()).unwrap();

    assert_system_assembler_accepts(&straight_line);
    assert_system_assembler_accepts(&multi_block);
}
