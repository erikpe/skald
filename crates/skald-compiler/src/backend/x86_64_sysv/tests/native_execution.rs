use super::*;
use crate::test_support::INLINE_FIELD_SOURCE;

#[test]
fn eager_boolean_truth_tables_execute_with_canonical_results() {
    let output = emit_assembly(Target::X86_64SysV, &eager_boolean_program()).unwrap();

    assert_eq!(run_native_assembly(&output).code(), Some(91));
}

#[test]
fn integer_bitwise_edge_patterns_execute_for_every_width() {
    let output = emit_assembly(Target::X86_64SysV, &integer_bitwise_program()).unwrap();

    assert_eq!(run_native_assembly(&output).code(), Some(91));
}

#[test]
fn integer_comparison_boundaries_store_canonical_booleans_and_branch_natively() {
    let output = assembly(concat!(
        "fn main() -> i64 {\n",
        "  var signed: bool = -9223372036854775808 < -1;\n",
        "  var unsigned: bool = 9223372036854775808u < 18446744073709551615u;\n",
        "  var byte: bool = 128u8 < 255u8;\n",
        "  if (signed) {\n",
        "    if (unsigned) {\n",
        "      if (byte) {\n",
        "        if (18446744073709551615u < 0u) { return 2; }\n",
        "        return 91;\n",
        "      }\n",
        "    }\n",
        "  }\n",
        "  return 1;\n",
        "}\n",
    ));

    assert_eq!(run_native_assembly(&output).code(), Some(91));
}

#[test]
fn integer_cast_boundaries_execute_without_runtime_support() {
    let output = assembly(concat!(
        "fn narrow(value: u64) -> u8 { return (u8) value; }\n",
        "fn widen(value: u8) -> u64 { return (u64) value; }\n",
        "fn reinterpret(value: u64) -> i64 { return (i64) value; }\n",
        "fn main() -> i64 {\n",
        "  if (narrow(258u) == 2u8) {\n",
        "    if (narrow(18446744073709551615u) == 255u8) {\n",
        "      if (widen(255u8) == 255u) {\n",
        "        if (reinterpret(18446744073709551615u) == -1) {\n",
        "          return 87;\n",
        "        }\n",
        "      }\n",
        "    }\n",
        "  }\n",
        "  return 1;\n",
        "}\n",
    ));

    assert_eq!(run_native_assembly(&output).code(), Some(87));
}

#[test]
fn receiverless_static_methods_use_method_symbols_and_stack_arguments() {
    let program = lower_source_to_mir(concat!(
        "class Math {\n",
        "  init() {}\n",
        "  static fn sum(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64) -> i64 {\n",
        "    return a + b + c + d + e + f + g;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { return Math.sum(1, 2, 3, 4, 5, 6, 7); }\n",
    ));
    verify_mir(&program).unwrap();
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();

    assert!(output.contains(".Lska.class.main.Math.c0.method.sum.m0:"));
    assert!(output.contains("call .Lska.class.main.Math.c0.method.sum.m0"));
    assert!(output.contains("sub rsp, 16"));
    assert_eq!(run_native_assembly(&output).code(), Some(28));
}

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
    output.push_str(record_i64_stub());

    let result = run_native_assembly_output(&output);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"42\n");
    assert!(result.stderr.is_empty());
}

#[test]
fn hand_built_aliases_mutate_forward_overlap_initialize_and_mix_abi_classes() {
    let (program, _) = alias_counter_program();
    let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    output.push_str(alias_record_i64_stub());

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
        "extern fn test_record_i64(value: i64) -> unit;\n",
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
        "    test_record_i64(counter.value);\n",
        "    return 0;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(alias_record_i64_stub());

    let result = run_native_assembly_output(&output);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"50\n");
    assert!(result.stderr.is_empty());
}
