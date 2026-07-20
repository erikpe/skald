use super::*;

#[test]
fn malformed_f64_mir_is_a_structured_backend_error() {
    let mut program = f64_arithmetic_program();
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected f64 constant assignment");
    };
    assignment.rvalue.ty = MirType::I64;

    let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
    assert!(error.message.contains("input MIR failed verification"));
    assert!(error.message.contains("f64 constant is not `f64`"));
}

#[test]
fn uses_no_unpreserved_callee_saved_scratch_registers() {
    let output = assembly("fn main() -> i64 { return (2 + 3) * 4; }");

    for register in ["%rbx", "%r12", "%r13", "%r14", "%r15"] {
        assert!(!output.contains(register));
    }
    assert!(output.contains("pushq %rbp"));
    assert!(output.contains("leave"));
}

#[test]
fn malformed_control_flow_is_a_structured_backend_error() {
    let mut mir = conditional_return_mir(true);
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let Some(MirTerminator::Branch { true_target, .. }) = &mut function.body.blocks[0].terminator
    else {
        panic!("expected branch terminator");
    };
    *true_target = BlockId::new(function.function, 99);

    let error = emit_assembly(Target::X86_64SysV, &mir).unwrap_err();
    assert_eq!(error.target(), Target::X86_64SysV);
    assert!(error
        .message()
        .contains("control-flow target f0:b99 is not declared"));
}
