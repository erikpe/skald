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
        MirValue {
            id: condition,
            ty: MirType::Bool,
            span,
        },
        MirValue {
            id: true_value,
            ty: MirType::I64,
            span,
        },
        MirValue {
            id: false_value,
            ty: MirType::I64,
            span,
        },
    ];
    let entry = BlockId::new(function.function, 0);
    let true_block = BlockId::new(function.function, 1);
    let false_block = BlockId::new(function.function, 2);
    function.body.entry = entry;
    function.body.blocks = vec![
        MirBasicBlock {
            id: entry,
            instructions: vec![constant_bool(condition, condition_value, span)],
            terminator: Some(MirTerminator::Branch {
                condition,
                true_target: true_block,
                false_target: false_block,
                span,
            }),
            span,
        },
        MirBasicBlock {
            id: true_block,
            instructions: vec![constant_i64(true_value, 37, span)],
            terminator: Some(MirTerminator::Return {
                value: Some(true_value),
                span,
            }),
            span,
        },
        MirBasicBlock {
            id: false_block,
            instructions: vec![constant_i64(false_value, 12, span)],
            terminator: Some(MirTerminator::Return {
                value: Some(false_value),
                span,
            }),
            span,
        },
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
    .map(|(id, ty)| MirValue { id, ty, span })
    .collect();
    let entry = BlockId::new(function.function, 0);
    let true_block = BlockId::new(function.function, 1);
    let false_block = BlockId::new(function.function, 2);
    let join = BlockId::new(function.function, 3);
    function.body.entry = entry;
    function.body.blocks = vec![
        MirBasicBlock {
            id: entry,
            instructions: vec![constant_bool(condition, true, span)],
            terminator: Some(MirTerminator::Branch {
                condition,
                true_target: true_block,
                false_target: false_block,
                span,
            }),
            span,
        },
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
        MirBasicBlock {
            id: join,
            instructions: vec![MirInstruction::Assign(MirAssignment {
                result: joined_result,
                rvalue: MirRvalue {
                    kind: MirRvalueKind::Load(storage.into()),
                    ty: MirType::I64,
                },
                span,
            })],
            terminator: Some(MirTerminator::Return {
                value: Some(joined_result),
                span,
            }),
            span,
        },
    ];
    assert!(verify_mir(&mir).is_ok());
    mir
}

pub(super) fn call_and_store_block(
    id: BlockId,
    target: FunctionId,
    result: ValueId,
    storage: crate::mir::StorageId,
    join: BlockId,
    span: crate::source::Span,
) -> MirBasicBlock {
    MirBasicBlock {
        id,
        instructions: vec![
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Direct(target),
                receiver: None,
                arguments: Vec::new(),
                result: Some(result),
                destination: None,
                span,
            }),
            MirInstruction::Store(MirStore {
                destination: storage.into(),
                value: result,
                span,
            }),
        ],
        terminator: Some(MirTerminator::Goto { target: join, span }),
        span,
    }
}

pub(super) fn constant_bool(
    result: ValueId,
    value: bool,
    span: crate::source::Span,
) -> MirInstruction {
    MirInstruction::Assign(MirAssignment {
        result,
        rvalue: MirRvalue {
            kind: MirRvalueKind::ConstantBool(value),
            ty: MirType::Bool,
        },
        span,
    })
}

pub(super) fn constant_i64(
    result: ValueId,
    value: i64,
    span: crate::source::Span,
) -> MirInstruction {
    MirInstruction::Assign(MirAssignment {
        result,
        rvalue: MirRvalue {
            kind: MirRvalueKind::ConstantI64(value),
            ty: MirType::I64,
        },
        span,
    })
}
