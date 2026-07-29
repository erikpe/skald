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
    let reasons = [
        MirTerminationReason::ObjectCastFailure,
        MirTerminationReason::OptionalAccessFailure,
        MirTerminationReason::OptionalGuardOverflow,
        MirTerminationReason::OptionalPinnedMutation,
        MirTerminationReason::ArrayAllocationFailure,
        MirTerminationReason::ArrayIndexOutOfBounds,
        MirTerminationReason::ArrayInvalidSliceBounds,
        MirTerminationReason::ArraySliceLengthMismatch,
    ];
    definition.body.blocks = reasons
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
    for (index, message) in [
        "checked object cast failed",
        "optional value is absent",
        "optional presence guard overflow",
        "cannot mutate a guarded optional value",
        "array allocation failed",
        "array index out of bounds",
        "array slice bounds are invalid",
        "array slice length mismatch",
    ]
    .into_iter()
    .enumerate()
    {
        let symbol = format!(".Lska_panic_message_{index}");
        assert_eq!(
            output.matches(&format!(".type {symbol}, @object")).count(),
            1
        );
        assert!(output.contains(&format!("lea rdi, [rip + {symbol}]")));
        assert!(output.contains(&format!("mov rsi, {}", message.len())));
    }
    assert_eq!(output.matches("call ska_rt_panic").count(), 8);
    assert!(!output.contains("ud2"));
    assert_system_assembler_accepts(&output);
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
