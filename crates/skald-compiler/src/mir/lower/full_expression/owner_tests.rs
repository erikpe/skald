use crate::{
    hir::{HirBlock, HirControlEffects, HirLiteralDataTable, Type},
    identity::{CallableId, ClassId, FunctionId},
    mir::{
        MirAssignment, MirCleanup, MirInstruction, MirPathCondition, MirPlace, MirRvalue,
        MirRvalueKind, MirStorage, MirStorageKind, MirStore, MirTerminator, MirType,
        PathConditionId, StorageId,
    },
    source::SourceDatabase,
};

use super::super::{BodyLowerer, BodyLoweringInput, FullExpressionTemporary};

#[test]
fn full_expression_owner_emits_reverse_conditional_cleanup_and_lifetime_graphs() {
    let callable = CallableId::Function(FunctionId::new(0));
    let mut sources = SourceDatabase::new();
    let source = sources.add("test.ska", "");
    let span = sources.get(source).unwrap().span(0, 0).unwrap();
    let source_body = HirBlock {
        statements: Vec::new(),
        effects: HirControlEffects::fallthrough(),
        span,
    };
    let literal_data = HirLiteralDataTable::default();
    let input = BodyLoweringInput {
        callable,
        parameters: &[],
        locals: &[],
        source_body: &source_body,
        return_type: Type::Unit,
        receiver_class: None,
        string_language_item: None,
        literal_data: &literal_data,
    };
    let mut lowerer = BodyLowerer::new(input);
    let activation = StorageId::new(callable, 0);
    let first = StorageId::new(callable, 1);
    let second = StorageId::new(callable, 2);
    let result = StorageId::new(callable, 3);
    let class = ClassId::new(0);
    lowerer.storage = vec![
        MirStorage {
            id: activation,
            source: None,
            name: "condition".to_owned(),
            kind: MirStorageKind::PathCondition,
            ty: MirType::Bool,
            span,
        },
        MirStorage {
            id: first,
            source: None,
            name: "first".to_owned(),
            kind: MirStorageKind::Temporary,
            ty: MirType::Class(class),
            span,
        },
        MirStorage {
            id: second,
            source: None,
            name: "second".to_owned(),
            kind: MirStorageKind::Temporary,
            ty: MirType::Class(class),
            span,
        },
        MirStorage {
            id: result,
            source: None,
            name: "result".to_owned(),
            kind: MirStorageKind::ScalarSpill,
            ty: MirType::I64,
            span,
        },
    ];
    let active = lowerer.body.allocate_block(span);
    let inactive = lowerer.body.allocate_block(span);
    let merge = lowerer.body.allocate_block(span);
    let condition = MirPathCondition {
        id: PathConditionId::new(callable, 0),
        parent: None,
        activation,
        active_predecessor: active,
        inactive_predecessor: inactive,
        merge,
        span,
    };
    let condition_id = lowerer.register_full_expression_condition(condition);
    lowerer.begin_storage_lifetime(activation, span);
    lowerer.track_full_expression_storage(result, span);
    let branch_value = lowerer.new_value(MirType::Bool, span);
    lowerer.emit(MirInstruction::Assign(MirAssignment {
        result: branch_value,
        rvalue: MirRvalue {
            kind: MirRvalueKind::ConstantBool(true),
            ty: MirType::Bool,
        },
        span,
    }));
    lowerer.terminate(MirTerminator::Branch {
        condition: branch_value,
        true_target: active,
        false_target: inactive,
        span,
    });

    lowerer.body.select_block(active).unwrap();
    lowerer.select_full_expression_condition(Some(condition_id));
    for storage in [first, second] {
        lowerer.track_full_expression_storage(storage, span);
        lowerer
            .full_expression
            .register_temporary(FullExpressionTemporary::Inline(MirCleanup {
                destination: MirPlace::base(storage),
                target: class,
                span,
            }));
    }
    let selected_true = lowerer.new_value(MirType::Bool, span);
    lowerer.emit(MirInstruction::Assign(MirAssignment {
        result: selected_true,
        rvalue: MirRvalue {
            kind: MirRvalueKind::ConstantBool(true),
            ty: MirType::Bool,
        },
        span,
    }));
    lowerer.emit(MirInstruction::Store(MirStore {
        destination: MirPlace::base(activation),
        value: selected_true,
        span,
    }));
    lowerer.terminate(MirTerminator::Goto {
        target: merge,
        span,
    });

    lowerer.body.select_block(inactive).unwrap();
    lowerer.select_full_expression_condition(None);
    let selected_false = lowerer.new_value(MirType::Bool, span);
    lowerer.emit(MirInstruction::Assign(MirAssignment {
        result: selected_false,
        rvalue: MirRvalue {
            kind: MirRvalueKind::ConstantBool(false),
            ty: MirType::Bool,
        },
        span,
    }));
    lowerer.emit(MirInstruction::Store(MirStore {
        destination: MirPlace::base(activation),
        value: selected_false,
        span,
    }));
    lowerer.terminate(MirTerminator::Goto {
        target: merge,
        span,
    });

    lowerer.body.select_block(merge).unwrap();
    let secured = lowerer.new_value(MirType::I64, span);
    lowerer.emit(MirInstruction::Assign(MirAssignment {
        result: secured,
        rvalue: MirRvalue {
            kind: MirRvalueKind::ConstantI64(41),
            ty: MirType::I64,
        },
        span,
    }));
    lowerer.emit(MirInstruction::Store(MirStore {
        destination: MirPlace::base(result),
        value: secured,
        span,
    }));
    lowerer.finish_full_expression(span);
    lowerer.terminate(MirTerminator::Return { value: None, span });
    let body = lowerer.body.finish();

    let cleaned: Vec<_> = body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::EndFullExpression(end) if end.temporaries.len() == 1 => {
                Some(end.temporaries[0].destination.base.expect_local_storage())
            }
            _ => None,
        })
        .collect();
    let ended: Vec<_> = body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::StorageDead(end) => Some(end.storage),
            _ => None,
        })
        .collect();
    let branch_count = body
        .blocks
        .iter()
        .filter(|block| matches!(block.terminator, Some(MirTerminator::Branch { .. })))
        .count();

    assert_eq!(cleaned, [second, first]);
    assert_eq!(ended, [second, first, result, activation]);
    assert_eq!(branch_count, 5);
    assert_eq!(body.path_conditions.len(), 1);
}
