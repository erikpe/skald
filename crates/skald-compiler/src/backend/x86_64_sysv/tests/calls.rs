use super::*;

#[test]
fn lowers_u64_payloads_arithmetic_and_integer_class_calls() {
    let output = assembly(concat!(
        "fn seventh(a: u64, b: u64, c: u64, d: u64, e: u64, f: u64, g: u64) -> u64 {\n",
        "  return (a + b) * c - g;\n",
        "}\n",
        "fn main() -> i64 { var value: u64 = seventh(18446744073709551615u, 2u, 3u, 4u, 5u, 6u, 7u); return 0; }",
    ));

    assert!(output.contains("movabsq $0xffffffffffffffff, %rax"));
    assert!(output.contains("addq %rcx, %rax"));
    assert!(output.contains("imulq %rcx, %rax"));
    assert!(output.contains("subq %rcx, %rax"));
    assert!(output.contains("movq %rdi, -8(%rbp)"));
    assert!(output.contains("movq 16(%rbp), %rax"));
    assert!(output.contains("call .Lska_fn_0"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn external_u64_calls_use_rax_for_the_full_width_result() {
    let output = assembly(concat!(
        "extern fn foreign_u64(value: u64) -> u64;\n",
        "fn main() -> i64 { var value: u64 = foreign_u64(18446744073709551615u); return 0; }",
    ));

    assert!(output.contains("movabsq $0xffffffffffffffff, %rax"));
    assert!(output.contains("movq -16(%rbp), %rdi"));
    assert!(output.contains("call foreign_u64\n    movq %rax,"));
}

#[test]
fn canonicalizes_u8_arithmetic_parameters_calls_and_returns() {
    let output = assembly(concat!(
        "fn seventh(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8, g: u8) -> u8 {\n",
        "  return (a + b) * c - g;\n",
        "}\n",
        "fn main() -> i64 { var value: u8 = seventh(255u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8); return 0; }",
    ));

    assert!(output.contains("movq %rdi, %rax\n    movzbq %al, %rax"));
    assert!(output.contains("movq 16(%rbp), %rax\n    movzbq %al, %rax"));
    assert!(output.matches("movzbq %al, %rax").count() >= 12);
    assert!(output.contains("addq %rcx, %rax\n    movzbq %al, %rax"));
    assert!(output.contains("imulq %rcx, %rax\n    movzbq %al, %rax"));
    assert!(output.contains("subq %rcx, %rax\n    movzbq %al, %rax"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn external_u8_results_are_zero_extended_before_storage() {
    let output = assembly(concat!(
        "extern fn foreign_u8(value: u8) -> u8;\n",
        "fn main() -> i64 { var value: u8 = foreign_u8(255u8); return 0; }",
    ));

    assert!(output.contains("call foreign_u8\n    movzbq %al, %rax\n    movq %rax,"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn lowers_verified_f64_mir_with_sse2_and_xmm_abi_results() {
    let program = f64_arithmetic_program();
    verify_mir(&program).unwrap();
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();

    assert!(output.contains("movabsq $4609434218613702656, %rax\n    movq %rax, %xmm14"));
    assert!(output.contains("mulsd %xmm15, %xmm14"));
    assert!(output.contains("xorpd %xmm15, %xmm14"));
    assert!(output.contains("addsd %xmm15, %xmm14"));
    assert!(output.contains("subsd %xmm15, %xmm14"));
    assert!(output.contains("movsd %xmm14, -8(%rbp)"));
    assert!(output.contains("movsd -72(%rbp), %xmm0"));
    assert!(output.contains("call .Lska_fn_0\n    movsd %xmm0,"));
    assert!(output.contains("movsd ") && output.contains(", %xmm0\n    call validate_f64"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn source_f64_uses_independent_integer_and_sse_argument_registers() {
    let output = assembly(concat!(
        "extern fn observe(value: f64) -> unit;\n",
        "fn choose(integer: i64, floating: f64, other: i64, another: f64) -> f64 { return floating + another; }\n",
        "fn main() -> i64 { observe(choose(1, 1.5, 2, 2.25)); return 0; }",
    ));

    assert!(output.contains("movq %rdi, -8(%rbp)"));
    assert!(output.contains("movsd %xmm0, -16(%rbp)"));
    assert!(output.contains("movq %rsi, -24(%rbp)"));
    assert!(output.contains("movsd %xmm1, -32(%rbp)"));
    assert!(output.contains("addsd %xmm15, %xmm14"));
    assert!(output.contains("call .Lska_fn_1\n    movsd %xmm0,"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn mixed_scalar_layout_independently_exhausts_register_classes() {
    let program = mixed_exhausted_abi_program();
    verify_mir(&program).unwrap();
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();

    assert!(output.contains("movq %rdi, -8(%rbp)"));
    assert!(output.contains("movsd %xmm0,"));
    assert!(output.contains("movq 16(%rbp), %rax"));
    assert!(output.contains("movsd 24(%rbp), %xmm14"));
    assert!(output.contains("subq $16, %rsp"));
    assert!(output.contains("movq %rax, (%rsp)"));
    assert!(output.contains("movsd %xmm14, 8(%rsp)"));
    assert!(output.contains("addq $16, %rsp"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn unit_calls_and_returns_do_not_move_a_fictitious_result() {
    let output = assembly(concat!(
        "fn notify(value: i64) -> unit {}\n",
        "fn main() -> i64 { notify(42); return 7; }\n",
    ));

    assert!(output.contains("call .Lska_fn_0\n    movabsq $7, %rax"));
    assert!(!output.contains("call .Lska_fn_0\n    movq %rax,"));
    assert!(output.contains(
        ".Lska_fn_0:\n    pushq %rbp\n    movq %rsp, %rbp\n    subq $16, %rsp\n    movq %rdi, -8(%rbp)\n.Lska_fn_0_block_0:\n    jmp .Lska_fn_0_epilogue\n.Lska_fn_0_epilogue:\n    leave\n    ret"
    ));
}

#[test]
fn lowers_register_and_stack_arguments_at_the_abi_boundary() {
    let output = assembly(concat!(
        "fn seventh(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, ",
        "g: i64) -> i64 { return g; }\n",
        "fn main() -> i64 { return seventh(1, 2, 3, 4, 5, 6, 7); }",
    ));

    for spill in [
        "movq %rdi, -8(%rbp)",
        "movq %rsi, -16(%rbp)",
        "movq %rdx, -24(%rbp)",
        "movq %rcx, -32(%rbp)",
        "movq %r8, -40(%rbp)",
        "movq %r9, -48(%rbp)",
    ] {
        assert!(output.contains(spill), "missing `{spill}` in:\n{output}");
    }
    assert!(output.contains("movq 16(%rbp), %rax"));
    assert!(output.contains("subq $16, %rsp"));
    assert!(output.contains("movq %rax, (%rsp)"));
    assert!(output.contains("call .Lska_fn_0\n    addq $16, %rsp"));
}

#[test]
fn emits_a_c_compatible_entry_boundary() {
    let output = assembly("fn helper() -> i64 { return 1; } fn main() -> i64 { return 2; }");

    assert!(output.contains(".globl main\n.type main, @function\nmain:"));
    assert!(output.contains("main:\n    pushq %rbp\n    movq %rsp, %rbp\n    call .Lska_fn_1"));
    assert!(!output.contains(".globl .Lska_fn_"));
}

#[test]
fn external_calls_use_the_declared_symbol_without_emitting_a_body() {
    let mir = lower_text(concat!(
        // Deliberately resembles an old internal symbol. The leading dot on
        // target-private symbols keeps the two namespaces disjoint.
        "extern fn ska_fn_1(value: i64) -> i64;\n",
        "fn main() -> i64 { return ska_fn_1(9); }\n",
    ));

    let output = emit_assembly(Target::X86_64SysV, &mir).unwrap();

    assert!(output.contains("call ska_fn_1"));
    assert!(!output.contains("\nska_fn_1:\n"));
    assert!(output.contains(".Lska_fn_1:"));
}

#[test]
fn lowers_boolean_values_through_internal_and_external_abi_boundaries() {
    let output = assembly(concat!(
        "extern fn external_flag(value: bool) -> bool;\n",
        "fn identity(value: bool) -> bool { return value; }\n",
        "fn main() -> i64 { var flag: bool = identity(true); var external: bool = external_flag(flag); return 0; }\n",
    ));

    assert!(output.contains("movabsq $1, %rax"));
    assert!(output.contains("call .Lska_fn_1"));
    assert!(output.contains("call external_flag\n    movzbq %al, %rax"));
    assert!(output.contains("movq %rdi, -8(%rbp)"));
}
