//! Shared fixtures for cleanup capability and pass tests.

use crate::{
    mir::{
        test_fixtures::{assign, storage_dead, storage_live, store, value},
        MirInstruction, MirPlace, MirRvalueKind, MirStorage, MirStorageKind, MirType,
        MirUnaryOperation, StorageId, ValueId,
    },
    passes::{run_mir_pipeline_measured, MirOptimizationProfile, VerifiedFinalMirProgram},
    test_support::lower_source_to_final_mir,
};

use super::super::super::{
    resolve_mir_pass_schedule,
    seal::{reseal_final_mir, UnverifiedFinalMirProgram},
    MirPassSchedule,
};

pub(in crate::passes::pipeline) const SINGLE_DEAD_ACTIVATION_SOURCE: &str = "fn main() -> i64 {
    var flag: bool = true;
    if (flag && true) { return 1; }
    return 0;
}";

fn none() -> MirPassSchedule {
    resolve_mir_pass_schedule(MirOptimizationProfile::None, std::iter::empty()).unwrap()
}

pub(in crate::passes::pipeline) fn normalized(source: &str) -> VerifiedFinalMirProgram {
    run_mir_pipeline_measured(lower_source_to_final_mir(source), &none())
        .result
        .unwrap()
}

pub(in crate::passes::pipeline) fn make_activation_dead(
    definition: &mut crate::mir::MirFunctionDefinition,
    activation: StorageId,
) -> Vec<ValueId> {
    let loads = definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(&assignment.rvalue.kind, MirRvalueKind::Load(place)
                    if place == &MirPlace::base(activation)) =>
            {
                Some(assignment.result)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let branch_blocks = definition
        .body
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| match block.terminator {
            Some(crate::mir::MirTerminator::Branch { condition, .. })
                if loads.contains(&condition) =>
            {
                Some(index)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for block_index in branch_blocks {
        let replacement = ValueId::new(definition.callable(), definition.values.len());
        let span = definition.body.blocks[block_index].span;
        definition
            .values
            .push(value(replacement, MirType::Bool, span));
        definition.body.blocks[block_index]
            .instructions
            .push(assign(
                replacement,
                MirRvalueKind::ConstantBool(true),
                MirType::Bool,
                span,
            ));
        let Some(crate::mir::MirTerminator::Branch { condition, .. }) =
            &mut definition.body.blocks[block_index].terminator
        else {
            unreachable!()
        };
        *condition = replacement;
    }
    loads
}

pub(in crate::passes::pipeline) fn append_complete_dead_activation(
    definition: &mut crate::mir::MirFunctionDefinition,
    block_index: usize,
    source_value: bool,
) -> StorageId {
    let storage = StorageId::new(definition.callable(), definition.storage.len());
    let source = ValueId::new(definition.callable(), definition.values.len());
    let result = ValueId::new(definition.callable(), definition.values.len() + 1);
    let span = definition.body.blocks[block_index].span;
    definition.storage.push(MirStorage {
        id: storage,
        source: None,
        name: format!("dead activation in block {block_index}"),
        kind: MirStorageKind::NormalizedPathActivation,
        ty: MirType::Bool,
        span,
    });
    definition.values.extend([
        value(source, MirType::Bool, span),
        value(result, MirType::Bool, span),
    ]);
    definition.body.blocks[block_index].instructions.extend([
        assign(
            source,
            MirRvalueKind::ConstantBool(source_value),
            MirType::Bool,
            span,
        ),
        storage_live(storage, span),
        store(MirPlace::base(storage), source, span),
        assign(
            result,
            MirRvalueKind::Load(MirPlace::base(storage)),
            MirType::Bool,
            span,
        ),
        storage_dead(storage, span),
    ]);
    storage
}

pub(in crate::passes::pipeline) fn append_materially_used_activation(
    definition: &mut crate::mir::MirFunctionDefinition,
    block_index: usize,
    source_value: bool,
) -> StorageId {
    let storage = append_complete_dead_activation(definition, block_index, source_value);
    let loaded = definition
        .values
        .last()
        .expect("complete activation fixture must declare its load result")
        .id;
    let material_result = ValueId::new(definition.callable(), definition.values.len());
    let span = definition.body.blocks[block_index].span;
    definition
        .values
        .push(value(material_result, MirType::Bool, span));
    definition.body.blocks[block_index]
        .instructions
        .push(assign(
            material_result,
            MirRvalueKind::Unary {
                operation: MirUnaryOperation::LogicalNotBool,
                operand: loaded,
            },
            MirType::Bool,
            span,
        ));
    storage
}

pub(in crate::passes::pipeline) fn append_declaration_only_dead_activation(
    definition: &mut crate::mir::MirFunctionDefinition,
) -> StorageId {
    let id = StorageId::new(definition.callable(), definition.storage.len());
    definition.storage.push(MirStorage {
        id,
        source: None,
        name: "declaration-only dead activation".to_owned(),
        kind: MirStorageKind::NormalizedPathActivation,
        ty: MirType::Bool,
        span: definition.span,
    });
    id
}

pub(in crate::passes::pipeline) fn dead_single_activation() -> VerifiedFinalMirProgram {
    let verified = normalized(SINGLE_DEAD_ACTIVATION_SOURCE);
    let (mut program, authority) = verified.invalidate_for_final_transformation().into_parts();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let activation = definition
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::NormalizedPathActivation)
        .unwrap()
        .id;
    assert!(!make_activation_dead(definition, activation).is_empty());
    reseal_final_mir(UnverifiedFinalMirProgram::from_parts(program, authority)).unwrap()
}
