use super::super::{frame, layout};
use super::*;

#[test]
fn computes_nested_class_layouts_and_aligned_object_frame_slots() {
    let (program, ids) = projected_object_program();
    let data = layout::DataLayout::compute(&program).unwrap();

    assert_eq!(data.ty(MirType::Class(ids.nested)).unwrap().size(), 16);
    assert_eq!(data.ty(MirType::Class(ids.container)).unwrap().size(), 32);
    assert_eq!(data.field(ids.nested_small).unwrap().offset, 0);
    assert_eq!(data.field(ids.nested_payload).unwrap().offset, 8);
    assert_eq!(data.field(ids.container_tag).unwrap().offset, 0);
    assert_eq!(data.field(ids.container_nested).unwrap().offset, 8);

    let function = program.definitions.get(program.entry_function).unwrap();
    let frame = frame::FrameLayout::plan(function.into(), &data).unwrap();
    assert_eq!(frame.storage(ids.first), -32);
    assert_eq!(frame.storage(ids.empty), -33);
    assert_eq!(frame.storage(ids.second), -72);
    assert_eq!(frame.size(), 128);
}

#[test]
fn lowers_nested_projected_places_with_width_correct_accesses() {
    let (program, _) = projected_object_program();
    assert!(verify_mir(&program).is_ok());

    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert!(output.contains("subq $128, %rsp"));
    assert!(output.contains("movb %al, -24(%rbp)"));
    assert!(output.contains("movzbq -24(%rbp), %rax"));
    assert!(output.contains("movsd %xmm14, -16(%rbp)"));
    assert!(output.contains("movsd -16(%rbp), %xmm14"));
    assert!(output.contains("movb %al, -32(%rbp)"));
    assert!(output.contains("movzbq -32(%rbp), %rax"));
}

#[test]
fn projected_place_assembly_is_deterministic_and_accepted_by_the_assembler() {
    let (program, _) = projected_object_program();
    let first = emit_assembly(Target::X86_64SysV, &program).unwrap();
    let second = emit_assembly(Target::X86_64SysV, &program).unwrap();

    assert_eq!(first, second);
    assert_system_assembler_accepts(&first);
}

#[test]
fn lowers_initializer_and_method_bodies_with_identity_based_symbols() {
    let program = counter_member_program();
    verify_mir(&program).unwrap();

    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert!(output.contains(".Lska_class_0_init_0:"));
    assert!(output.contains(".Lska_class_0_method_0:"));
    assert!(output.contains(".Lska_class_0_method_1:"));
    assert!(output.contains(".Lska_class_0_method_2:"));
    assert!(output.contains("leaq -8(%rbp), %rdi"));
    assert!(output.contains("call .Lska_class_0_init_0"));
    assert!(output.contains("call .Lska_class_0_method_0"));
    assert!(output.contains("call .Lska_class_0_method_1"));
    assert!(output.contains("call .Lska_class_0_method_2"));
    assert!(output.contains("call .Lska_fn_1"));
    assert!(output.contains("movq %rdi, -8(%rbp)"));
    assert_system_assembler_accepts(&format!("{output}\n{}", println_i64_stub()));
}

#[test]
fn dumps_member_receiver_storage_and_bodies_deterministically() {
    let program = counter_member_program();
    let dump = crate::mir::dump_mir(&program);

    assert_eq!(dump, crate::mir::dump_mir(&program));
    assert!(dump.contains("MemberDefinition c0:init0"));
    assert!(dump.contains("Receiver c0:init0:s0"));
    assert!(dump.contains("c0:init0:s0 receiver c0:init0:self \"self\" : class c0"));
    assert!(dump.contains("MemberDefinition c0:method0"));
    assert!(dump.contains("call f1(c0:method0:v0, c0:method0:v1)"));
}

#[test]
fn verifier_rejects_corrupt_member_receiver_metadata() {
    let mut program = counter_member_program();
    let callable = crate::identity::CallableId::Method(MethodId::new(ClassId::new(0), 0));
    let definition = program
        .member_definitions
        .get_mut_for_test(callable)
        .unwrap();
    definition.storage[0].ty = MirType::I64;

    let errors = verify_mir(&program).unwrap_err().to_string();
    assert!(errors.contains("receiver storage has the wrong class type"));
}

#[test]
fn lowers_exhausted_mixed_receiver_abi_through_stack_arguments() {
    let program = exhausted_receiver_abi_program();
    verify_mir(&program).unwrap();
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();

    assert!(output.contains("subq $16, %rsp"));
    assert!(output.contains("movq %rax, (%rsp)"));
    assert!(output.contains("movsd %xmm14, 8(%rsp)"));
    assert!(output.contains("movq 16(%rbp), %rax"));
    assert!(output.contains("movsd 24(%rbp), %xmm14"));
    assert_system_assembler_accepts(&output);
}

pub(super) fn println_i64_stub() -> &'static str {
    concat!(
        ".section .rodata\n",
        ".Lobj4_output:\n",
        "    .ascii \"42\\n\"\n",
        ".text\n",
        ".globl ska_rt_println_i64\n",
        ".type ska_rt_println_i64, @function\n",
        "ska_rt_println_i64:\n",
        "    cmpq $42, %rdi\n",
        "    jne .Lobj4_bad_value\n",
        "    movq $1, %rax\n",
        "    movq $1, %rdi\n",
        "    leaq .Lobj4_output(%rip), %rsi\n",
        "    movq $3, %rdx\n",
        "    syscall\n",
        "    ret\n",
        ".Lobj4_bad_value:\n",
        "    movq $60, %rax\n",
        "    movq $99, %rdi\n",
        "    syscall\n",
        ".size ska_rt_println_i64, .-ska_rt_println_i64\n",
    )
}
