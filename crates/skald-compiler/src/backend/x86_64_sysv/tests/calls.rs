use super::*;

#[test]
fn lowers_u64_payloads_arithmetic_and_integer_class_calls() {
    let output = assembly(concat!(
        "fn seventh(a: u64, b: u64, c: u64, d: u64, e: u64, f: u64, g: u64) -> u64 {\n",
        "  return (a + b) * c - g;\n",
        "}\n",
        "fn main() -> i64 { var value: u64 = seventh(0xffffffffffffffffu, 2u, 3u, 4u, 5u, 6u, 7u); return 0; }",
    ));

    assert!(output.contains("mov rax, 0xffffffffffffffff"));
    assert!(output.contains("add rax, rcx"));
    assert!(output.contains("imul rax, rcx"));
    assert!(output.contains("sub rax, rcx"));
    assert!(output.contains("mov qword ptr [rbp - 8], rdi"));
    assert!(output.contains("mov rax, qword ptr [rbp + 16]"));
    assert!(output.contains("call .Lska.fn.main.seventh.f0"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn external_u64_calls_use_rax_for_the_full_width_result() {
    let output = assembly(concat!(
        "extern fn foreign_u64(value: u64) -> u64;\n",
        "fn main() -> i64 { var value: u64 = foreign_u64(0xffffffffffffffffu); return 0; }",
    ));

    assert!(output.contains("mov rax, 0xffffffffffffffff"));
    assert!(output.contains("mov rdi, qword ptr [rbp - 16]"));
    assert!(output.contains("call foreign_u64\n    mov qword ptr [rbp"));
}

#[test]
fn canonicalizes_u8_arithmetic_parameters_calls_and_returns() {
    let output = assembly(concat!(
        "fn seventh(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8, g: u8) -> u8 {\n",
        "  return (a + b) * c - g;\n",
        "}\n",
        "fn main() -> i64 { var value: u8 = seventh(0xffu8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8); return 0; }",
    ));

    assert!(output.contains("mov rax, rdi\n    movzx rax, al"));
    assert!(output.contains("mov rax, qword ptr [rbp + 16]\n    movzx rax, al"));
    assert!(output.matches("movzx rax, al").count() >= 12);
    assert!(output.contains("add rax, rcx\n    movzx rax, al"));
    assert!(output.contains("imul rax, rcx\n    movzx rax, al"));
    assert!(output.contains("sub rax, rcx\n    movzx rax, al"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn external_u8_results_are_zero_extended_before_storage() {
    let output = assembly(concat!(
        "extern fn foreign_u8(value: u8) -> u8;\n",
        "fn main() -> i64 { var value: u8 = foreign_u8(0xffu8); return 0; }",
    ));

    assert!(output.contains("call foreign_u8\n    movzx rax, al\n    mov qword ptr [rbp"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn lowers_verified_f64_mir_with_sse2_and_xmm_abi_results() {
    let program = f64_arithmetic_program();
    verify_mir(&program).unwrap();
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();

    assert!(output.contains("mov rax, 4609434218613702656\n    movq xmm14, rax"));
    assert!(output.contains("mulsd xmm14, xmm15"));
    assert!(output.contains("xorpd xmm14, xmm15"));
    assert!(output.contains("addsd xmm14, xmm15"));
    assert!(output.contains("subsd xmm14, xmm15"));
    assert!(output.contains("movsd qword ptr [rbp - 8], xmm14"));
    assert!(output.contains("movsd xmm0, qword ptr [rbp - 72]"));
    assert!(output.contains("call .Lska.fn.main.compute.f0\n    movsd qword ptr [rbp"));
    assert!(output.contains("movsd xmm0, ") && output.contains("\n    call validate_f64"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn source_f64_uses_independent_integer_and_sse_argument_registers() {
    let output = assembly(concat!(
        "extern fn observe(value: f64) -> unit;\n",
        "fn choose(integer: i64, floating: f64, other: i64, another: f64) -> f64 { return floating + another; }\n",
        "fn main() -> i64 { observe(choose(1, 1.5, 2, 2.25)); return 0; }",
    ));

    assert!(output.contains("mov qword ptr [rbp - 8], rdi"));
    assert!(output.contains("movsd qword ptr [rbp - 16], xmm0"));
    assert!(output.contains("mov qword ptr [rbp - 24], rsi"));
    assert!(output.contains("movsd qword ptr [rbp - 32], xmm1"));
    assert!(output.contains("addsd xmm14, xmm15"));
    assert!(output.contains("call .Lska.fn.main.choose.f1\n    movsd qword ptr [rbp"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn mixed_scalar_layout_independently_exhausts_register_classes() {
    let program = mixed_exhausted_abi_program();
    verify_mir(&program).unwrap();
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();

    assert!(output.contains("mov qword ptr [rbp - 8], rdi"));
    assert!(output.contains("movsd qword ptr [rbp"));
    assert!(output.contains("mov rax, qword ptr [rbp + 16]"));
    assert!(output.contains("movsd xmm14, qword ptr [rbp + 24]"));
    assert!(output.contains("sub rsp, 16"));
    assert!(output.contains("mov qword ptr [rsp], rax"));
    assert!(output.contains("movsd qword ptr [rsp + 8], xmm14"));
    assert!(output.contains("add rsp, 16"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn unit_calls_and_returns_do_not_move_a_fictitious_result() {
    let output = assembly(concat!(
        "fn notify(value: i64) -> unit {}\n",
        "fn main() -> i64 { notify(42); return 7; }\n",
    ));

    assert!(output.contains("call .Lska.fn.main.notify.f0\n    mov rax, 7"));
    assert!(!output.contains("call .Lska.fn.main.notify.f0\n    mov qword ptr [rbp"));
    assert!(output.contains(
        ".Lska.fn.main.notify.f0:\n    push rbp\n    mov rbp, rsp\n    sub rsp, 16\n    mov qword ptr [rbp - 8], rdi\n.Lska.fn.main.notify.f0.block_0:\n    jmp .Lska.fn.main.notify.f0.epilogue\n.Lska.fn.main.notify.f0.epilogue:\n    leave\n    ret"
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
        "mov qword ptr [rbp - 8], rdi",
        "mov qword ptr [rbp - 16], rsi",
        "mov qword ptr [rbp - 24], rdx",
        "mov qword ptr [rbp - 32], rcx",
        "mov qword ptr [rbp - 40], r8",
        "mov qword ptr [rbp - 48], r9",
    ] {
        assert!(output.contains(spill), "missing `{spill}` in:\n{output}");
    }
    assert!(output.contains("mov rax, qword ptr [rbp + 16]"));
    assert!(output.contains("sub rsp, 16"));
    assert!(output.contains("mov qword ptr [rsp], rax"));
    assert!(output.contains("call .Lska.fn.main.seventh.f0\n    add rsp, 16"));
}

#[test]
fn emits_a_c_compatible_entry_boundary() {
    let output = assembly("fn helper() -> i64 { return 1; } fn main() -> i64 { return 2; }");

    assert!(output.contains(".globl main\n.type main, @function\nmain:"));
    assert!(output.contains(concat!(
        "main:\n",
        "    push rbp\n",
        "    mov rbp, rsp\n",
        "    call ska_rt_abi_v9\n",
        "    call .Lska.fn.main.main.f1",
    )));
    assert!(!output.contains(".globl .Lska.fn."));
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
    assert!(output.contains(".Lska.fn.main.main.f1:"));
}

#[test]
fn lowers_boolean_values_through_internal_and_external_abi_boundaries() {
    let output = assembly(concat!(
        "extern fn external_flag(value: bool) -> bool;\n",
        "fn identity(value: bool) -> bool { return value; }\n",
        "fn main() -> i64 { var flag: bool = identity(true); var external: bool = external_flag(flag); return 0; }\n",
    ));

    assert!(output.contains("mov rax, 1"));
    assert!(output.contains("call .Lska.fn.main.identity.f1"));
    assert!(output.contains("call external_flag\n    movzx rax, al"));
    assert!(output.contains("mov qword ptr [rbp - 8], rdi"));
}
