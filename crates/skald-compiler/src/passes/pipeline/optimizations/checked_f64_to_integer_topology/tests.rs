use crate::{
    identity::FunctionId,
    mir::{
        test_fixtures::checked_primitive_cast_program, BlockId, MirF64ToIntegerRange,
        MirIntegerType, MirPathCondition, MirStorage, MirStorageKind, MirTerminator, MirType,
        PathConditionId, StorageId,
    },
};

use super::*;

fn entry_definition(program: &crate::mir::MirProgram) -> &crate::mir::MirFunctionDefinition {
    program.definitions.get(program.entry_function).unwrap()
}

fn entry_definition_mut(
    program: &mut crate::mir::MirProgram,
) -> &mut crate::mir::MirFunctionDefinition {
    program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap()
}

fn one_topology(program: &crate::mir::MirProgram) -> CheckedF64ToIntegerTopology {
    let observations =
        observe_checked_f64_to_integer_topologies(entry_definition(program).into()).unwrap();
    let [CheckedF64ToIntegerTopologyObservation::Protocol(topology)] = observations.as_slice()
    else {
        panic!("expected one canonical checked floating-cast topology: {observations:?}");
    };
    topology.as_ref().clone()
}

#[test]
fn observes_exact_owned_topology_for_every_integer_target_without_mutation() {
    for target in [MirIntegerType::I64, MirIntegerType::U64, MirIntegerType::U8] {
        let program = checked_primitive_cast_program(0x400e_0000_0000_0000, target, 3);
        let original = program.clone();
        let topology = one_topology(&program);
        let definition = entry_definition(&program);

        assert_eq!(program, original, "observation must be read-only");
        assert_eq!(topology.relation(), MirF64ToIntegerRange { target });
        assert_eq!(topology.source(), (topology.check.source, MirType::F64));
        assert_eq!(
            topology.result(),
            (topology.check.result, topology.relation().result_type())
        );
        assert_eq!(topology.source_load.site.block, topology.success_block);
        assert_eq!(topology.source_load.site.instruction, 0);
        assert_eq!(topology.result_assignment.site.instruction, 1);
        assert_eq!(topology.result_store.instruction, 2);
        assert_eq!(topology.result_store.block, topology.success_block);
        assert_eq!(topology.result_reload.site.block, topology.join_block);
        assert_eq!(topology.result_reload.site.instruction, 0);
        assert_eq!(
            topology.check_span,
            definition
                .block(topology.check_block)
                .unwrap()
                .terminator
                .as_ref()
                .unwrap()
                .span()
        );
        assert!(!topology.protected);
    }
}

#[test]
fn records_protected_protocols_and_all_control_flow_spans() {
    let mut program = checked_primitive_cast_program(0x400e_0000_0000_0000, MirIntegerType::I64, 3);
    let topology = one_topology(&program);
    let definition = entry_definition_mut(&mut program);
    let activation = StorageId::new(definition.callable(), definition.storage.len());
    definition.storage.push(MirStorage {
        id: activation,
        source: None,
        name: "protocol-proof".to_owned(),
        kind: MirStorageKind::PathCondition,
        ty: MirType::Bool,
        span: definition.span,
    });
    definition.body.path_conditions.push(MirPathCondition {
        id: PathConditionId::new(definition.callable(), 0),
        parent: None,
        activation,
        active_predecessor: topology.success_block,
        inactive_predecessor: topology.success_block,
        merge: topology.join_block,
        span: definition.span,
    });
    let span = definition.span;

    let topology = one_topology(&program);
    assert!(topology.protected);
    assert_eq!(topology.failure_span, span);
    assert_eq!(topology.result_store_span, span);
    assert_eq!(topology.success_edge_span, span);
}

#[test]
fn malformed_identities_error_and_shape_changes_reject_deterministically() {
    let mut foreign = checked_primitive_cast_program(0x400e_0000_0000_0000, MirIntegerType::I64, 3);
    let owner = foreign.entry_function;
    let definition = entry_definition_mut(&mut foreign);
    let success = definition
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match block.terminator.as_mut() {
            Some(MirTerminator::PrimitiveCastRangeCheck { success_target, .. }) => {
                Some(success_target)
            }
            _ => None,
        })
        .unwrap();
    *success = BlockId::new(FunctionId::new(owner.index() + 1), 0);
    assert!(matches!(
        observe_checked_f64_to_integer_topologies((&*definition).into()),
        Err(MirRewriteError::InvalidReference {
            failure: crate::mir::rewrite::MirReferenceFailure::Foreign,
            ..
        })
    ));

    let mut reordered =
        checked_primitive_cast_program(0x400e_0000_0000_0000, MirIntegerType::I64, 3);
    let topology = one_topology(&reordered);
    entry_definition_mut(&mut reordered).body.blocks[topology.success_block.index()]
        .instructions
        .swap(0, 1);
    let first =
        observe_checked_f64_to_integer_topologies(entry_definition(&reordered).into()).unwrap();
    assert_eq!(
        first,
        observe_checked_f64_to_integer_topologies(entry_definition(&reordered).into()).unwrap()
    );
    assert!(matches!(
        first.as_slice(),
        [CheckedF64ToIntegerTopologyObservation::Rejected {
            reason: CheckedF64ToIntegerTopologyRejectionReason::NonCanonicalTopology,
            ..
        }]
    ));
}

#[test]
fn mismatched_conversion_and_duplicate_predecessor_reject() {
    let mut mismatched =
        checked_primitive_cast_program(0x400e_0000_0000_0000, MirIntegerType::I64, 3);
    let topology = one_topology(&mismatched);
    let definition = entry_definition_mut(&mut mismatched);
    let crate::mir::MirInstruction::Assign(assignment) =
        &mut definition.body.blocks[topology.success_block.index()].instructions[1]
    else {
        unreachable!();
    };
    let crate::mir::MirRvalueKind::CheckedF64ToInteger { relation, .. } =
        &mut assignment.rvalue.kind
    else {
        unreachable!();
    };
    *relation = MirF64ToIntegerRange {
        target: MirIntegerType::U64,
    };
    assert!(matches!(
        observe_checked_f64_to_integer_topologies((&*definition).into())
            .unwrap()
            .as_slice(),
        [CheckedF64ToIntegerTopologyObservation::Rejected { .. }]
    ));

    let mut duplicate =
        checked_primitive_cast_program(0x400e_0000_0000_0000, MirIntegerType::I64, 3);
    let topology = one_topology(&duplicate);
    let definition = entry_definition_mut(&mut duplicate);
    definition.body.blocks[topology.join_block.index()].terminator = Some(MirTerminator::Goto {
        target: topology.success_block,
        span: definition.span,
    });
    assert!(matches!(
        observe_checked_f64_to_integer_topologies((&*definition).into())
            .unwrap()
            .as_slice(),
        [CheckedF64ToIntegerTopologyObservation::Rejected { .. }]
    ));
}
