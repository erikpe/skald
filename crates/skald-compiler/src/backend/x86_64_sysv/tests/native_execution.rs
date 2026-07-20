use super::*;

#[test]
fn verified_f64_mir_executes_through_internal_and_external_abi_boundaries() {
    let mut output = emit_assembly(Target::X86_64SysV, &f64_arithmetic_program()).unwrap();
    output.push_str(concat!(
        "\n.text\n",
        ".globl validate_f64\n",
        ".type validate_f64, @function\n",
        "validate_f64:\n",
        "    movq %xmm0, %rax\n",
        "    movabsq $0xc008000000000000, %rcx\n",
        "    cmpq %rcx, %rax\n",
        "    setne %al\n",
        "    movzbq %al, %rax\n",
        "    ret\n",
        ".size validate_f64, .-validate_f64\n",
    ));

    assert!(run_native_assembly(&output).success());
}

#[test]
fn external_f64_results_are_read_from_xmm0() {
    let mut program = f64_arithmetic_program();
    program.declarations.entries_mut_for_test()[0].linkage = MirFunctionLinkage::External {
        symbol: "compute".to_owned(),
    };
    program.definitions.remove_for_test(FunctionId::new(0));
    verify_mir(&program).unwrap();

    let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert!(output.contains("call compute\n    movsd %xmm0,"));
    output.push_str(concat!(
        "\n.text\n",
        ".globl compute\n",
        ".type compute, @function\n",
        "compute:\n",
        "    movabsq $0xc008000000000000, %rax\n",
        "    movq %rax, %xmm0\n",
        "    ret\n",
        ".size compute, .-compute\n",
        ".globl validate_f64\n",
        ".type validate_f64, @function\n",
        "validate_f64:\n",
        "    movq %xmm0, %rax\n",
        "    movabsq $0xc008000000000000, %rcx\n",
        "    cmpq %rcx, %rax\n",
        "    setne %al\n",
        "    movzbq %al, %rax\n",
        "    ret\n",
        ".size validate_f64, .-validate_f64\n",
    ));
    assert!(run_native_assembly(&output).success());
}

#[test]
fn hand_built_conditional_executes_both_branch_directions() {
    for (condition, expected_status) in [(true, 37), (false, 12)] {
        let mir = conditional_return_mir(condition);
        let output = emit_assembly(Target::X86_64SysV, &mir).unwrap();
        let status = run_native_assembly(&output);

        assert_eq!(status.code(), Some(expected_status));
    }
}

#[test]
fn hand_built_members_construct_mutate_and_print_through_receiver_calls() {
    let program = counter_member_program();
    let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    output.push_str(println_i64_stub());

    let result = run_native_assembly_output(&output);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"42\n");
    assert!(result.stderr.is_empty());
}
