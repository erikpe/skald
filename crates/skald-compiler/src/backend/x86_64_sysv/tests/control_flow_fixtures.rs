use super::*;

pub(super) fn conditional_return_mir(condition_value: bool) -> MirProgram {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let span = function.span;
    let condition = ValueId::new(function.function, 0);
    let true_value = ValueId::new(function.function, 1);
    let false_value = ValueId::new(function.function, 2);
    function.values = vec![
        fixture_value(condition, MirType::Bool, span),
        fixture_value(true_value, MirType::I64, span),
        fixture_value(false_value, MirType::I64, span),
    ];
    let entry = BlockId::new(function.function, 0);
    let true_block = BlockId::new(function.function, 1);
    let false_block = BlockId::new(function.function, 2);
    function.body.entry = entry;
    function.body.blocks = vec![
        fixture_block(
            entry,
            vec![fixture_assign(
                condition,
                MirRvalueKind::ConstantBool(condition_value),
                MirType::Bool,
                span,
            )],
            Some(MirTerminator::Branch {
                condition,
                true_target: true_block,
                false_target: false_block,
                span,
            }),
            span,
        ),
        fixture_block(
            true_block,
            vec![fixture_assign(
                true_value,
                MirRvalueKind::ConstantI64(37),
                MirType::I64,
                span,
            )],
            Some(MirTerminator::Return {
                value: Some(true_value),
                span,
            }),
            span,
        ),
        fixture_block(
            false_block,
            vec![fixture_assign(
                false_value,
                MirRvalueKind::ConstantI64(12),
                MirType::I64,
                span,
            )],
            Some(MirTerminator::Return {
                value: Some(false_value),
                span,
            }),
            span,
        ),
    ];
    assert!(verify_mir(&mir).is_ok());
    mir
}

pub(super) fn branch_call_diamond_mir() -> MirProgram {
    let mut mir = lower_text(concat!(
        "fn left() -> i64 { return 11; }\n",
        "fn right() -> i64 { return 22; }\n",
        "fn main() -> i64 { var result: i64 = 0; return result; }\n",
    ));
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let span = function.span;
    let storage = function.storage[0].id;
    let condition = ValueId::new(function.function, 0);
    let left_result = ValueId::new(function.function, 1);
    let right_result = ValueId::new(function.function, 2);
    let joined_result = ValueId::new(function.function, 3);
    function.values = [
        (condition, MirType::Bool),
        (left_result, MirType::I64),
        (right_result, MirType::I64),
        (joined_result, MirType::I64),
    ]
    .into_iter()
    .map(|(id, ty)| fixture_value(id, ty, span))
    .collect();
    let entry = BlockId::new(function.function, 0);
    let true_block = BlockId::new(function.function, 1);
    let false_block = BlockId::new(function.function, 2);
    let join = BlockId::new(function.function, 3);
    function.body.entry = entry;
    function.body.blocks = vec![
        fixture_block(
            entry,
            vec![
                fixture_storage_live(storage, span),
                fixture_assign(
                    condition,
                    MirRvalueKind::ConstantBool(true),
                    MirType::Bool,
                    span,
                ),
            ],
            Some(MirTerminator::Branch {
                condition,
                true_target: true_block,
                false_target: false_block,
                span,
            }),
            span,
        ),
        call_and_store_block(
            true_block,
            FunctionId::new(0),
            left_result,
            storage,
            join,
            span,
        ),
        call_and_store_block(
            false_block,
            FunctionId::new(1),
            right_result,
            storage,
            join,
            span,
        ),
        fixture_block(
            join,
            vec![
                fixture_assign(
                    joined_result,
                    MirRvalueKind::Load(storage.into()),
                    MirType::I64,
                    span,
                ),
                fixture_storage_dead(storage, span),
            ],
            Some(MirTerminator::Return {
                value: Some(joined_result),
                span,
            }),
            span,
        ),
    ];
    assert!(verify_mir(&mir).is_ok());
    mir
}

fn call_and_store_block(
    id: BlockId,
    target: FunctionId,
    result: ValueId,
    storage: crate::mir::StorageId,
    join: BlockId,
    span: crate::source::Span,
) -> MirBasicBlock {
    fixture_block(
        id,
        vec![
            fixture_call(
                MirCallTarget::Direct(target),
                None,
                Vec::new(),
                Some(result),
                None,
                span,
            ),
            fixture_store(storage.into(), result, span),
        ],
        Some(MirTerminator::Goto { target: join, span }),
        span,
    )
}
