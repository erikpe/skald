use super::super::{frame, layout};
use super::*;
use crate::test_support::INLINE_FIELD_SOURCE;

#[test]
fn lays_out_and_addresses_deep_source_subobjects_from_every_storage_base() {
    let program = lower_text(INLINE_FIELD_SOURCE);
    let data = layout::DataLayout::compute(&program).unwrap();
    let root = ClassId::new(0);
    let empty = ClassId::new(1);
    let leaf = ClassId::new(2);
    let branch = ClassId::new(3);
    let root_left = FieldId::new(root, 1);
    let branch_leaf = FieldId::new(branch, 2);
    let leaf_small = FieldId::new(leaf, 0);
    let leaf_value = FieldId::new(leaf, 1);

    assert_eq!(data.ty(MirType::Class(empty)).unwrap().size(), 1);
    assert_eq!(data.ty(MirType::Class(leaf)).unwrap().size(), 16);
    assert_eq!(data.ty(MirType::Class(branch)).unwrap().size(), 32);
    assert_eq!(data.ty(MirType::Class(root)).unwrap().size(), 72);
    assert_eq!(data.field(root_left).unwrap().offset, 8);
    assert_eq!(data.field(branch_leaf).unwrap().offset, 8);
    assert_eq!(data.field(leaf_value).unwrap().offset, 8);

    let main = program.definitions.get(FunctionId::new(3)).unwrap();
    let main_frame = frame::FrameLayout::plan(main.into(), &data).unwrap();
    let direct = main_frame
        .place(
            &program,
            main.into(),
            &data,
            &MirPlace::base(main.storage[0].id)
                .project_field(root_left)
                .project_field(branch_leaf)
                .project_field(leaf_value),
        )
        .unwrap();
    assert_eq!(direct.base(), frame::FramePlaceBase::Direct);
    assert_eq!(direct.displacement(), -48);
    assert_eq!(direct.ty(), MirType::I64);

    let adjust = program
        .member_definition(MethodId::new(root, 1).into())
        .unwrap();
    let adjust_frame = frame::FrameLayout::plan(adjust.into(), &data).unwrap();
    let receiver = adjust_frame
        .place(
            &program,
            adjust.into(),
            &data,
            &MirPlace::base(adjust.receiver)
                .project_field(root_left)
                .project_field(branch_leaf)
                .project_field(leaf_small),
        )
        .unwrap();
    assert!(matches!(
        receiver.base(),
        frame::FramePlaceBase::Receiver { .. }
    ));
    assert_eq!(receiver.displacement(), 16);
    assert_eq!(receiver.ty(), MirType::U8);
    assert!(receiver.uses_byte_access());

    let forward = program.definitions.get(FunctionId::new(2)).unwrap();
    let forward_frame = frame::FrameLayout::plan(forward.into(), &data).unwrap();
    let alias = forward_frame
        .place(
            &program,
            forward.into(),
            &data,
            &MirPlace::alias_parameter(forward.parameters[0])
                .project_field(root_left)
                .project_field(branch_leaf)
                .project_field(leaf_value),
        )
        .unwrap();
    assert!(matches!(alias.base(), frame::FramePlaceBase::Alias { .. }));
    assert_eq!(alias.displacement(), 24);
    assert_eq!(alias.ty(), MirType::I64);
}

#[test]
fn source_projected_assembly_is_deterministic_and_accepted() {
    let first = assembly(INLINE_FIELD_SOURCE);
    let second = assembly(INLINE_FIELD_SOURCE);

    assert_eq!(first, second);
    assert!(first.contains("call .Lska_class_3_init_0"));
    assert!(first.contains("call .Lska_class_2_method_1"));
    assert_system_assembler_accepts(&first);
}

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
    assert!(output.contains("sub rsp, 128"));
    assert!(output.contains("mov byte ptr [rbp - 24], al"));
    assert!(output.contains("movzx rax, byte ptr [rbp - 24]"));
    assert!(output.contains("movsd qword ptr [rbp - 16], xmm14"));
    assert!(output.contains("movsd xmm14, qword ptr [rbp - 16]"));
    assert!(output.contains("mov byte ptr [rbp - 32], al"));
    assert!(output.contains("movzx rax, byte ptr [rbp - 32]"));
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
    assert!(output.contains("lea rdi, [rbp - 8]"));
    assert!(output.contains("call .Lska_class_0_init_0"));
    assert!(output.contains("call .Lska_class_0_method_0"));
    assert!(output.contains("call .Lska_class_0_method_1"));
    assert!(output.contains("call .Lska_class_0_method_2"));
    assert!(output.contains("call .Lska_fn_1"));
    assert!(output.contains("mov qword ptr [rbp - 8], rdi"));
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
    assert!(dump.contains("call f1(value(c0:method0:v0), value(c0:method0:v1))"));
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

    assert!(output.contains("sub rsp, 32"));
    assert!(output.contains("mov qword ptr [rsp + 16], rax"));
    assert!(output.contains("movsd qword ptr [rsp + 24], xmm14"));
    assert!(output.contains("mov rax, qword ptr [rbp + 32]"));
    assert!(output.contains("movsd xmm14, qword ptr [rbp + 40]"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn alias_homes_are_pointer_sized_and_indirect_places_lower_deterministically() {
    let (program, ids) = alias_counter_program();
    verify_mir(&program).unwrap();
    let data = layout::DataLayout::compute(&program).unwrap();
    assert_eq!(data.ty(MirType::Class(ids.class)).unwrap().size(), 32);

    let function = program.definitions.get(ids.add).unwrap();
    let planned = frame::FrameLayout::plan(function.into(), &data).unwrap();
    assert_eq!(planned.storage(function.parameters[0]), -8);
    assert_eq!(planned.storage(function.parameters[1]), -32);
    assert_eq!(planned.size(), 64);

    let first = emit_assembly(Target::X86_64SysV, &program).unwrap();
    let second = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert_eq!(first, second);
    assert!(first.contains(".Lska_fn_3:"));
    assert!(first.contains(".Lska_fn_4:"));
    assert!(first.contains("mov qword ptr [rbp - 8], rdi"));
    assert!(first.contains("mov rdi, qword ptr [rbp - 8]"));
    assert!(first.contains("call .Lska_fn_3"));
    assert!(first.contains(".Lska_class_0_init_0:"));
    assert!(first.contains(".Lska_class_0_init_1:"));
    assert!(first.contains(".Lska_class_0_init_0_block_0:"));
    assert!(first.contains(".Lska_class_0_init_1_block_0:"));
    assert!(first.contains("call .Lska_class_0_init_1"));
    assert!(first.contains("call .Lska_class_0_method_3"));
    assert_system_assembler_accepts(&format!("{first}\n{}", println_i64_stub()));
}

#[test]
fn lowers_exhausted_receiver_alias_and_sse_arguments_through_ordered_stack_slots() {
    let program = exhausted_receiver_alias_abi_program();
    verify_mir(&program).unwrap();
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();

    assert!(output.contains("sub rsp, 128"));
    assert!(output.contains("lea rax, [rbp - 32]"));
    assert!(output.contains("mov qword ptr [rsp], rax"));
    assert!(output.contains("movsd qword ptr [rsp + 120], xmm14"));
    assert!(output.contains("mov rax, qword ptr [rbp + 16]"));
    assert!(output.contains("movsd xmm14, qword ptr [rbp + 136]"));
    assert_system_assembler_accepts(&output);
}

pub(super) fn println_i64_stub() -> &'static str {
    concat!(
        ".section .rodata\n",
        ".Lprintln_i64_output:\n",
        "    .ascii \"42\\n\"\n",
        ".text\n",
        ".globl ska_rt_println_i64\n",
        ".type ska_rt_println_i64, @function\n",
        "ska_rt_println_i64:\n",
        "    cmp rdi, 42\n",
        "    jne .Lprintln_i64_bad_value\n",
        "    mov rax, 1\n",
        "    mov rdi, 1\n",
        "    lea rsi, [rip + .Lprintln_i64_output]\n",
        "    mov rdx, 3\n",
        "    syscall\n",
        "    ret\n",
        ".Lprintln_i64_bad_value:\n",
        "    mov rax, 60\n",
        "    mov rdi, 99\n",
        "    syscall\n",
        ".size ska_rt_println_i64, .-ska_rt_println_i64\n",
    )
}
