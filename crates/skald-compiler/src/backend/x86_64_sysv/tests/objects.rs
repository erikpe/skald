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
    let frame = frame::FrameLayout::plan(function, &data).unwrap();
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
