use crate::{
    identity::{CallableId, OptionalTypeId},
    mir::{
        BlockId, MirBasicBlock, MirBody, MirInstruction, MirLogicalExpression, MirLogicalOperation,
        MirOptionalViewBegin, MirOptionalViewEnd, MirPathCondition, MirPlace, MirStorage,
        MirStorageKind, MirTerminator, MirType, MirValue, OptionalGuardId, PathConditionId,
        StorageId, ValueId,
    },
    test_support::lower_source_to_mir,
};

use super::MirCallableEdit;

pub(in crate::mir::rewrite) fn fixture_parts(
) -> (CallableId, Vec<MirStorage>, Vec<MirValue>, MirBody) {
    let program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let definition = program
        .definitions
        .get(program.entry_function)
        .expect("entry definition");
    let callable = definition.callable();
    let span = definition.span;
    let storage_id = |index| StorageId::new(callable, index);
    let value_id = |index| ValueId::new(callable, index);
    let block_id = |index| BlockId::new(callable, index);
    let path_id = |index| PathConditionId::new(callable, index);
    let guard = OptionalGuardId::new(callable, 2);
    let optional = OptionalTypeId::new(0);

    let storage = (0..3)
        .map(|index| MirStorage {
            id: storage_id(index),
            source: None,
            name: format!("storage{index}"),
            kind: MirStorageKind::Temporary,
            ty: MirType::I64,
            span,
        })
        .collect();
    let values = (0..3)
        .map(|index| MirValue {
            id: value_id(index),
            ty: MirType::I64,
            span,
        })
        .collect();
    let logical = |selected_result| MirLogicalExpression {
        operation: MirLogicalOperation::And,
        condition: path_id(0),
        result: storage_id(0),
        left_result: value_id(0),
        split: block_id(0),
        selection: block_id(0),
        right_entry: block_id(1),
        right_exit: block_id(1),
        right_result: value_id(1),
        short: block_id(1),
        join: block_id(1),
        selected_result,
        span,
    };
    let body = MirBody {
        entry: block_id(0),
        blocks: vec![
            MirBasicBlock {
                id: block_id(0),
                instructions: vec![],
                terminator: Some(MirTerminator::BeginOptionalView {
                    begin: MirOptionalViewBegin {
                        optional,
                        guard,
                        source: MirPlace::base(storage_id(0)),
                        payload: MirType::I64,
                        span,
                    },
                    success_target: block_id(1),
                    absent_target: block_id(1),
                    overflow_target: block_id(1),
                    span,
                }),
                span,
            },
            MirBasicBlock {
                id: block_id(1),
                instructions: vec![MirInstruction::EndOptionalView(MirOptionalViewEnd {
                    optional,
                    guard,
                    source: MirPlace::base(storage_id(0)),
                    payload: MirType::I64,
                    span,
                })],
                terminator: Some(MirTerminator::Return {
                    value: Some(value_id(0)),
                    span,
                }),
                span,
            },
        ],
        path_conditions: vec![
            MirPathCondition {
                id: path_id(0),
                parent: None,
                activation: storage_id(1),
                active_predecessor: block_id(0),
                inactive_predecessor: block_id(0),
                merge: block_id(1),
                span,
            },
            MirPathCondition {
                id: path_id(1),
                parent: Some(path_id(0)),
                activation: storage_id(2),
                active_predecessor: block_id(0),
                inactive_predecessor: block_id(0),
                merge: block_id(1),
                span,
            },
        ],
        logical_expressions: vec![logical(value_id(1)), logical(value_id(2))],
    };
    (callable, storage, values, body)
}

pub(in crate::mir::rewrite) fn edit() -> MirCallableEdit {
    let (callable, storage, values, body) = fixture_parts();
    MirCallableEdit::from_dense_parts(callable, storage, values, body)
        .expect("fixture opens as sparse callable state")
}

pub(super) fn empty_block(identity: BlockId, span: crate::source::Span) -> MirBasicBlock {
    MirBasicBlock {
        id: identity,
        instructions: vec![],
        terminator: Some(MirTerminator::Return { value: None, span }),
        span,
    }
}
