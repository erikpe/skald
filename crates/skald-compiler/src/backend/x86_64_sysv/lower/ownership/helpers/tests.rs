use super::*;
use crate::{
    backend::x86_64_sysv::{emit, lower::terminator::PanicMessagePool, machine::AssemblyProgram},
    test_support::run_native_assembly_output,
};

#[test]
fn generated_retain_helper_aligns_the_stack_before_reporting_exhaustion() {
    let functions = vec![lower_retain()];
    let panic_messages = PanicMessagePool::build(&functions).into_assembly();
    let program = AssemblyProgram {
        functions,
        panic_messages,
        static_slots: vec![],
        dispatch_tables: vec![],
        literal_backings: vec![],
        runtime_trace: Default::default(),
    };
    let mut assembly = emit::emit(&program);
    assembly.push_str(PROBE);
    let output = run_native_assembly_output(&assembly);
    assert!(output.status.success(), "{output:?}\n{assembly}");
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

// Exhaustion is a valid reporting edge. Check the runtime ABI at callee entry,
// independent of the caller's choice of prologue or scratch reservation.
const PROBE: &str = r#"
.text
.globl main
.type main, @function
main:
    push rbp
    mov rbp, rsp
    lea rdi, [rip + exhausted_handle]
    call .Lska_shared_handle_retain
    ud2
.size main, .-main
.globl ska_rt_panic
.type ska_rt_panic, @function
ska_rt_panic:
    mov rax, rsp
    and rax, 15
    cmp rax, 8
    jne probe_failure
    cmp rsi, 24
    jne probe_failure
    mov eax, 60
    xor edi, edi
    syscall
probe_failure:
    ud2
.size ska_rt_panic, .-ska_rt_panic
.data
.p2align 3
exhausted_handle: .quad 0xfffffffffffffffe
"#;
