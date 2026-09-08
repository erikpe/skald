//! Shared fixtures for cleanup capability and pass tests.

use crate::{
    mir::{
        test_fixtures::{assign, value},
        MirInstruction, MirPlace, MirRvalueKind, MirStorageKind, MirType, StorageId, ValueId,
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
