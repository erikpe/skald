use super::*;
use crate::test_support::INLINE_FIELD_SOURCE;

#[test]
fn source_inline_fields_construct_and_execute_through_deep_places() {
    let output = assembly(INLINE_FIELD_SOURCE);
    let result = run_native_assembly_output(&output);

    assert_eq!(result.status.code(), Some(111));
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
fn verified_f64_mir_executes_through_internal_and_external_abi_boundaries() {
    let mut output = emit_assembly(Target::X86_64SysV, &f64_arithmetic_program()).unwrap();
    output.push_str(concat!(
        "\n.text\n",
        ".globl validate_f64\n",
        ".type validate_f64, @function\n",
        "validate_f64:\n",
        "    movq rax, xmm0\n",
        "    mov rcx, 0xc008000000000000\n",
        "    cmp rax, rcx\n",
        "    setne al\n",
        "    movzx rax, al\n",
        "    ret\n",
        ".size validate_f64, .-validate_f64\n",
    ));

    assert!(run_native_assembly(&output).success());
}

#[test]
fn external_f64_results_are_read_from_xmm0() {
    let mut program = f64_arithmetic_program();
    program.external_links = crate::external::ExternalLinkTable::new(vec![
        crate::external::ExternalLink {
            id: crate::identity::ExternalLinkId::new(0),
            symbol: "compute".to_owned(),
            declarations: vec![FunctionId::new(0)],
        },
        crate::external::ExternalLink {
            id: crate::identity::ExternalLinkId::new(1),
            symbol: "validate_f64".to_owned(),
            declarations: vec![FunctionId::new(1)],
        },
    ]);
    program.declarations.entries_mut_for_test()[0].linkage = MirFunctionLinkage::External {
        link: crate::identity::ExternalLinkId::new(0),
    };
    program.declarations.entries_mut_for_test()[1].linkage = MirFunctionLinkage::External {
        link: crate::identity::ExternalLinkId::new(1),
    };
    program.definitions.remove_for_test(FunctionId::new(0));
    verify_mir(&program).unwrap();

    let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert!(output.contains("call compute\n    movsd qword ptr [rbp"));
    output.push_str(concat!(
        "\n.text\n",
        ".globl compute\n",
        ".type compute, @function\n",
        "compute:\n",
        "    mov rax, 0xc008000000000000\n",
        "    movq xmm0, rax\n",
        "    ret\n",
        ".size compute, .-compute\n",
        ".globl validate_f64\n",
        ".type validate_f64, @function\n",
        "validate_f64:\n",
        "    movq rax, xmm0\n",
        "    mov rcx, 0xc008000000000000\n",
        "    cmp rax, rcx\n",
        "    setne al\n",
        "    movzx rax, al\n",
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

#[test]
fn hand_built_aliases_mutate_forward_overlap_initialize_and_mix_abi_classes() {
    let (program, _) = alias_counter_program();
    let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    output.push_str(alias_println_i64_stub());

    let result = run_native_assembly_output(&output);
    assert!(
        result.status.success(),
        "alias program failed with {:?}: {}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"50\n");
    assert!(result.stderr.is_empty());
}

#[test]
fn source_aliases_lower_through_the_native_backend() {
    let source = concat!(
        "extern fn ska_rt_println_i64(value: i64) -> unit;\n",
        "class Counter {\n",
        "    value: i64;\n",
        "    init(value: i64) { self.value = value; }\n",
        "}\n",
        "fn add(mut ref counter: Counter, amount: i64) -> unit {\n",
        "    counter.value = counter.value + amount;\n",
        "}\n",
        "fn forward(mut ref counter: Counter, amount: i64) -> unit { add(counter, amount); }\n",
        "fn main() -> i64 {\n",
        "    var counter: Counter = Counter(40);\n",
        "    forward(counter, 10);\n",
        "    ska_rt_println_i64(counter.value);\n",
        "    return 0;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(alias_println_i64_stub());

    let result = run_native_assembly_output(&output);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"50\n");
    assert!(result.stderr.is_empty());
}
