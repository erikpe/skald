use super::*;

#[test]
fn lowers_source_conditionals_to_deterministic_block_branches() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  if (false) { return 1; }\n",
        "  elif (true) { return 2; }\n",
        "  else { return 3; }\n",
        "}\n",
    );

    let output = complete_assembly(source);
    assert_eq!(output, complete_assembly(source));
    assert!(output.contains(".Lska.fn.main.main.f0.block_0:"));
    assert!(output.contains("jne .Lska.fn.main.main.f0.block_1"));
    assert!(output.contains("jmp .Lska.fn.main.main.f0.block_2"));
    assert!(output.contains(".Lska.fn.main.main.f0.block_4:"));
}

#[test]
fn logical_activation_frame_baseline_is_stable_across_pipeline_profiles() {
    let source = concat!(
        "fn choose(left: bool, right: bool) -> bool { return left && right; }\n",
        "fn main() -> i64 {\n",
        "  if (choose(true, true)) { return 42; }\n",
        "  return 0;\n",
        "}\n",
    );

    let without_optimizations = complete_assembly(source);
    let default = assembly(source);
    assert_eq!(without_optimizations, complete_assembly(source));
    assert_eq!(default, assembly(source));

    for output in [&without_optimizations, &default] {
        let choose = function_assembly(output, ".Lska.fn.main.choose.f0");
        assert!(choose.contains("sub rsp,"), "{choose}");
        assert!(choose.contains("[rbp -"), "{choose}");
        assert_eq!(run_native_assembly(output).code(), Some(42), "{output}");
    }
}

#[test]
fn lowers_forward_and_backward_jumps_in_stable_block_order() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let span = function.span;
    function.values.clear();
    function.body.blocks[0].instructions.clear();
    let second = BlockId::new(function.function, 1);
    function.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: second,
        span,
    });
    function.body.blocks.push(MirBasicBlock {
        id: second,
        instructions: Vec::new(),
        terminator: Some(MirTerminator::Goto {
            target: function.body.entry,
            span,
        }),
        span,
    });
    assert!(verify_mir(&mir).is_ok());

    let output = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    let first_position = output.find(".Lska.fn.main.main.f0.block_0:").unwrap();
    let second_position = output.find(".Lska.fn.main.main.f0.block_1:").unwrap();
    assert!(first_position < second_position);
    assert!(
        output.contains(".Lska.fn.main.main.f0.block_0:\n    jmp .Lska.fn.main.main.f0.block_1")
    );
    assert!(
        output.contains(".Lska.fn.main.main.f0.block_1:\n    jmp .Lska.fn.main.main.f0.block_0")
    );
}

#[test]
fn lowers_boolean_branches_and_returns_in_both_arms() {
    let output = emit_assembly(Target::X86_64SysV, &conditional_return_mir(true)).unwrap();

    assert!(output.contains(
        "mov rax, qword ptr [rbp - 8]\n    test rax, rax\n    jne .Lska.fn.main.main.f0.block_1\n    jmp .Lska.fn.main.main.f0.block_2"
    ));
    assert!(output.contains(".Lska.fn.main.main.f0.block_1:\n    mov rax, 37"));
    assert!(output.contains(".Lska.fn.main.main.f0.block_2:\n    mov rax, 12"));
    assert_eq!(
        output.matches("jmp .Lska.fn.main.main.f0.epilogue").count(),
        2
    );
    assert_eq!(output.matches(".Lska.fn.main.main.f0.epilogue:").count(), 1);
}

#[test]
fn lowers_a_diamond_with_branch_local_calls_and_a_storage_join() {
    let output = emit_assembly(Target::X86_64SysV, &branch_call_diamond_mir()).unwrap();

    for index in 0..=3 {
        assert_eq!(
            output
                .matches(&format!(".Lska.fn.main.main.f2.block_{index}:"))
                .count(),
            1
        );
    }
    assert!(output.contains(".Lska.fn.main.main.f2.block_1:\n    call .Lska.fn.main.left.f0"));
    assert!(output.contains(".Lska.fn.main.main.f2.block_2:\n    call .Lska.fn.main.right.f1"));
    assert_eq!(
        output.matches("jmp .Lska.fn.main.main.f2.block_3").count(),
        2
    );
    assert!(output.contains(".Lska.fn.main.main.f2.block_3:\n    mov rax, qword ptr [rbp - 8]"));
}

#[test]
fn jumps_to_a_non_first_entry_before_emitting_blocks_in_id_order() {
    let mut mir = conditional_return_mir(true);
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    function.body.entry = function.body.blocks[1].id;
    assert!(verify_mir(&mir).is_ok());

    let output = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    let entry_jump = output.find("jmp .Lska.fn.main.main.f0.block_1").unwrap();
    let first_block = output.find(".Lska.fn.main.main.f0.block_0:").unwrap();
    let selected_block = output.find(".Lska.fn.main.main.f0.block_1:").unwrap();
    assert!(entry_jump < first_block);
    assert!(first_block < selected_block);
}
