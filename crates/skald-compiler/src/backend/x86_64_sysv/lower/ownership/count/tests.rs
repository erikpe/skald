use super::*;
use crate::{
    backend::x86_64_sysv::{
        emit,
        machine::{AssemblyFunction, AssemblyProgram},
    },
    test_support::run_native_assembly_output,
};

#[test]
fn release_frees_original_header_after_finalizer_changes_owner_and_clobbers_callers() {
    let complete = Label::new("release_complete".into());
    let mut instructions = vec![
        Instruction::Push(Register::Rbp),
        Instruction::Move {
            source: Register::Rsp.into(),
            destination: Register::Rbp.into(),
        },
        Instruction::LoadSymbolAddress {
            symbol: "owner_slot".into(),
            destination: Register::Rax,
        },
        Instruction::Move {
            source: value::memory(Register::Rax, 0),
            destination: Register::Rax.into(),
        },
    ];
    emit_release_loaded_handle(
        Label::new("release_invalid".into()),
        Label::new("release_last".into()),
        complete.clone(),
        0,
        None,
        call::TraceAttribution::SourceBodyFromOmittedHelper,
        &mut instructions,
    );
    instructions.extend([
        Instruction::Label(complete),
        Instruction::Leave,
        Instruction::Return,
    ]);
    let program = AssemblyProgram {
        functions: vec![AssemblyFunction {
            symbol: "release_owner".into(),
            exported: false,
            instructions,
        }],
        static_slots: vec![],
        dispatch_tables: vec![],
        literal_backings: vec![],
        panic_messages: vec![],
        runtime_trace: Default::default(),
    };
    let mut assembly = emit::emit(&program);
    assembly.push_str(PROBE);
    let output = run_native_assembly_output(&assembly);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

// These probes check the allocation identity and lifecycle order, independent
// of the selector's register or temporary-stack choices. The synthetic header
// follows the runtime ABI; its storage is never passed to the real allocator.
const PROBE: &str = r#"
.text
.macro clobber_callers
    .irp reg,rax,rcx,rdx,rsi,rdi,r8,r9,r10,r11
        xor \reg, \reg
    .endr
    .irp reg,xmm0,xmm1,xmm2,xmm3,xmm4,xmm5,xmm6,xmm7,xmm8,xmm9,xmm10,xmm11,xmm12,xmm13,xmm14,xmm15
        pxor \reg, \reg
    .endr
.endm
.globl main
.type main, @function
main:
    push rbp
    mov rbp, rsp
    call release_owner
    cmp QWORD PTR [rip + finalizer_count], 1
    jne probe_failure
    cmp QWORD PTR [rip + free_count], 1
    jne probe_failure
    lea rax, [rip + replacement_header]
    cmp QWORD PTR [rip + owner_slot], rax
    jne probe_failure
    cmp QWORD PTR [rip + replacement_header], 1
    jne probe_failure
    xor eax, eax
    leave
    ret
.size main, .-main

.type finalize_probe, @function
finalize_probe:
    lea rax, [rip + original_payload]
    cmp rdi, rax
    jne probe_failure
    cmp QWORD PTR [rip + original_header], 0
    jne probe_failure
    cmp QWORD PTR [rip + free_count], 0
    jne probe_failure
    cmp QWORD PTR [rip + finalizer_count], 0
    jne probe_failure
    inc QWORD PTR [rip + finalizer_count]
    lea rax, [rip + replacement_header]
    mov QWORD PTR [rip + owner_slot], rax
    clobber_callers
    ret
.size finalize_probe, .-finalize_probe

.type ska_rt_free, @function
ska_rt_free:
    lea rax, [rip + original_header]
    cmp rdi, rax
    jne probe_failure
    cmp QWORD PTR [rip + finalizer_count], 1
    jne probe_failure
    cmp QWORD PTR [rip + free_count], 0
    jne probe_failure
    inc QWORD PTR [rip + free_count]
    clobber_callers
    ret
.size ska_rt_free, .-ska_rt_free

probe_failure:
    ud2
.data
.p2align 3
original_header: .quad 1, metadata
original_payload: .quad 123
replacement_header: .quad 1, metadata
replacement_payload: .quad 456
metadata: .quad finalize_probe
owner_slot: .quad original_header
finalizer_count: .quad 0
free_count: .quad 0
"#;
