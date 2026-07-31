use super::*;

#[test]
fn pools_and_reports_every_static_termination_reason_in_stable_order() {
    let mut program = lower_text("fn main() -> i64 { return 0; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let span = definition.span;
    definition.values.clear();
    definition.body.blocks = MirTerminationReason::ALL
        .into_iter()
        .enumerate()
        .map(|(index, reason)| MirBasicBlock {
            id: BlockId::new(definition.function, index),
            instructions: Vec::new(),
            terminator: Some(MirTerminator::Terminate { reason, span }),
            span,
        })
        .collect();

    verify_mir(&program).unwrap();
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert_eq!(
        output,
        emit_assembly(Target::X86_64SysV, &program).unwrap(),
        "static message pooling must be deterministic"
    );
    for (index, message) in [
        (0, "checked object cast failed"),
        (1, "optional value is absent"),
        (2, "optional presence guard overflow"),
        (3, "cannot mutate a guarded optional value"),
        (4, "array allocation failed"),
        (5, "array index out of bounds"),
        (6, "array slice bounds are invalid"),
        (7, "array slice length mismatch"),
        // Index 8 remains the pre-existing ownership-overflow message.
        (9, "shift count out of range"),
    ] {
        let symbol = format!(".Lska_panic_message_{index}");
        assert_eq!(
            output.matches(&format!(".type {symbol}, @object")).count(),
            1
        );
        assert!(output.contains(&format!(
            "{symbol}:\n    .ascii \"{message}\"\n.size {symbol}, .-{symbol}\n"
        )));
        assert!(output.contains(&format!("lea rdi, [rip + {symbol}]")));
        assert!(output.contains(&format!("mov rsi, {}", message.len())));
    }
    assert_eq!(output.matches("call ska_rt_panic").count(), 9);
    assert!(!output.contains("ud2"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn every_static_termination_reason_reports_exact_native_stderr() {
    for (reason, message) in MirTerminationReason::ALL.into_iter().zip([
        "checked object cast failed",
        "optional value is absent",
        "optional presence guard overflow",
        "cannot mutate a guarded optional value",
        "array allocation failed",
        "array index out of bounds",
        "array slice bounds are invalid",
        "array slice length mismatch",
        "shift count out of range",
    ]) {
        let mut program = lower_text("fn main() -> i64 { return 0; }");
        let definition = program
            .definitions
            .get_mut_for_test(program.entry_function)
            .unwrap();
        definition.values.clear();
        definition.body.blocks[0].instructions.clear();
        definition.body.blocks[0].terminator = Some(MirTerminator::Terminate {
            reason,
            span: definition.span,
        });

        verify_mir(&program).unwrap();
        let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
        output.push_str(native_panic_reporter());
        let result = run_native_assembly_output(&output);
        assert_eq!(result.status.code(), Some(1), "{reason:?}");
        assert!(result.stdout.is_empty(), "{reason:?}");
        assert_eq!(
            result.stderr,
            format!("panic: {message}\n").as_bytes(),
            "{reason:?}"
        );
    }
}

#[test]
fn emits_only_used_static_messages() {
    let mut program = lower_text("fn main() -> i64 { return 0; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    definition.values.clear();
    definition.body.blocks[0].instructions.clear();
    definition.body.blocks[0].terminator = Some(MirTerminator::Terminate {
        reason: MirTerminationReason::ArrayIndexOutOfBounds,
        span: definition.span,
    });

    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert_eq!(output.matches(".type .Lska_panic_message_").count(), 1);
    assert!(output.contains(".type .Lska_panic_message_5, @object"));
}
