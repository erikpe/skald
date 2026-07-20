use super::*;

#[test]
fn body_builder_allocates_and_selects_blocks_in_stable_order() {
    let mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir.definitions.get(mir.entry_function).unwrap();
    let mut builder = MirBodyBuilder::new(function.function, function.span);
    let entry = builder.entry();
    let second = builder.allocate_block(function.span);
    let third = builder.allocate_block(function.span);

    assert_eq!(builder.current(), entry);
    assert_eq!(second.index(), 1);
    assert_eq!(third.index(), 2);
    builder.select_block(third).unwrap();
    assert_eq!(builder.current(), third);
    let body = builder.finish();
    assert_eq!(body.entry, entry);
    assert_eq!(
        body.blocks.iter().map(|block| block.id).collect::<Vec<_>>(),
        [entry, second, third]
    );
}

#[test]
fn body_builder_rejects_emission_and_duplicate_termination_after_a_terminator() {
    let mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir.definitions.get(mir.entry_function).unwrap();
    let mut builder = MirBodyBuilder::new(function.function, function.span);
    let entry = builder.entry();
    builder
        .terminate(MirTerminator::Return {
            value: None,
            span: function.span,
        })
        .unwrap();

    assert_eq!(
        builder
            .terminate(MirTerminator::Return {
                value: None,
                span: function.span,
            })
            .unwrap_err(),
        MirBuildError::BlockAlreadyTerminated(entry)
    );
    assert_eq!(
        builder
            .push_instruction(MirInstruction::Store(MirStore {
                storage: StorageId::new(function.function, 0),
                value: ValueId::new(function.function, 0),
                span: function.span,
            }))
            .unwrap_err(),
        MirBuildError::BlockAlreadyTerminated(entry)
    );
}

#[test]
fn body_builder_rejects_unknown_and_foreign_block_selection() {
    let mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir.definitions.get(mir.entry_function).unwrap();
    let mut builder = MirBodyBuilder::new(function.function, function.span);

    for unknown in [
        BlockId::new(function.function, 1),
        BlockId::new(FunctionId::new(99), 0),
    ] {
        assert_eq!(
            builder.select_block(unknown).unwrap_err(),
            MirBuildError::UnknownBlock(unknown)
        );
    }
}

#[test]
fn body_builder_uses_the_complete_callable_identity_as_block_owner() {
    let mir = lower_text("fn main() -> i64 { return 0; }");
    let span = mir.definitions.get(mir.entry_function).unwrap().span;
    let class = ClassId::new(3);
    let method = MethodId::new(class, 2);
    let initializer = InitializerId::new(class, 0);
    let mut builder = MirBodyBuilder::new(method, span);

    assert_eq!(builder.entry().callable(), method.into());
    assert_eq!(builder.allocate_block(span).callable(), method.into());

    let foreign = BlockId::new(initializer, 0);
    assert_eq!(
        builder.select_block(foreign).unwrap_err(),
        MirBuildError::UnknownBlock(foreign)
    );
}
