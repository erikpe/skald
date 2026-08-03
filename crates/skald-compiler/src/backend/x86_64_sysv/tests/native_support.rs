pub(super) use crate::test_support::{
    assert_system_assembler_accepts, run_native_assembly, run_native_assembly_output,
};

pub(super) fn native_panic_reporter() -> &'static str {
    concat!(
        "\n.section .rodata\n",
        ".Lpanic_prefix: .ascii \"panic: \"\n",
        ".Lpanic_newline: .byte 10\n",
        "\n.text\n",
        ".globl ska_rt_panic\n",
        ".type ska_rt_panic, @function\n",
        "ska_rt_panic:\n",
        "    mov r8, rdi\n",
        "    mov r9, rsi\n",
        "    mov rax, 1\n",
        "    mov rdi, 2\n",
        "    lea rsi, [rip + .Lpanic_prefix]\n",
        "    mov rdx, 7\n",
        "    syscall\n",
        "    mov rax, 1\n",
        "    mov rdi, 2\n",
        "    mov rsi, r8\n",
        "    mov rdx, r9\n",
        "    syscall\n",
        "    mov rax, 1\n",
        "    mov rdi, 2\n",
        "    lea rsi, [rip + .Lpanic_newline]\n",
        "    mov rdx, 1\n",
        "    syscall\n",
        "    mov rax, 60\n",
        "    mov rdi, 1\n",
        "    syscall\n",
        ".size ska_rt_panic, .-ska_rt_panic\n",
    )
}

pub(super) fn native_allocator() -> &'static str {
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
