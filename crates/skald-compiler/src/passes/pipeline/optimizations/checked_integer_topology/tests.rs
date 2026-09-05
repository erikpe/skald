use crate::{
    identity::FunctionId,
    mir::{
        BlockId, MirPathCondition, MirStorage, MirStorageKind, MirTerminator, MirType,
        PathConditionId, StorageId,
    },
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::passes::pipeline::optimizations::checked_integer_protocol::{
    observe_checked_integer_protocols, CheckedIntegerProtocolObservation,
    CheckedIntegerProtocolRejectionReason,
};

fn observations(program: &crate::mir::MirProgram) -> Vec<CheckedIntegerTopologyObservation> {
    program
        .executable_definitions()
        .flat_map(|definition| observe_checked_integer_topologies(definition).unwrap())
        .collect()
}

fn entry_definition_mut(
    program: &mut crate::mir::MirProgram,
) -> &mut crate::mir::MirFunctionDefinition {
    program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap()
}

#[test]
fn observes_every_checked_variant_without_requiring_constant_operands() {
    let program = lower_source_to_final_mir(concat!(
        "fn div_i64(value: i64) -> i64 { return value / 5; }\n",
        "fn rem_i64(value: i64) -> i64 { return value % 5; }\n",
        "fn div_u64(value: u64) -> u64 { return value / 5u; }\n",
        "fn rem_u64(value: u64) -> u64 { return value % 5u; }\n",
        "fn div_u8(value: u8) -> u8 { return value / 5u8; }\n",
        "fn rem_u8(value: u8) -> u8 { return value % 5u8; }\n",
        "fn shl_i64(value: i64) -> i64 { return value << 2u; }\n",
        "fn shr_i64(value: i64) -> i64 { return value >> 2u; }\n",
        "fn shl_u64(value: u64) -> u64 { return value << 2u; }\n",
        "fn shr_u64(value: u64) -> u64 { return value >> 2u; }\n",
        "fn shl_u8(value: u8) -> u8 { return value << 2u; }\n",
        "fn shr_u8(value: u8) -> u8 { return value >> 2u; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let observed = observations(&program);

    assert_eq!(observed.len(), 12);
    assert!(observed
        .iter()
        .all(|entry| matches!(entry, CheckedIntegerTopologyObservation::Protocol(_))));
    assert!(observed[..6].iter().all(|entry| matches!(
        entry,
        CheckedIntegerTopologyObservation::Protocol(topology)
            if matches!(topology.check, CheckedIntegerProtocolCheck::Division(_))
    )));
    assert!(observed[6..].iter().all(|entry| matches!(
        entry,
        CheckedIntegerTopologyObservation::Protocol(topology)
            if matches!(topology.check, CheckedIntegerProtocolCheck::Shift(_))
    )));
}

#[test]
fn structural_observation_precedes_the_legacy_constant_adapter() {
    let program = lower_source_to_final_mir(
        "fn divide(value: i64) -> i64 { return value / 2; } fn main() -> i64 { return 0; }",
    );
    let definition = program
        .executable_definitions()
        .find(|definition| {
            definition.body().blocks.iter().any(|block| {
                matches!(
                    block.terminator,
                    Some(MirTerminator::IntegerDivisorCheck { .. })
                )
            })
        })
        .unwrap();

    assert!(matches!(
        observe_checked_integer_topologies(definition)
            .unwrap()
            .as_slice(),
        [CheckedIntegerTopologyObservation::Protocol(_)]
    ));
    assert!(matches!(
        observe_checked_integer_protocols(definition)
            .unwrap()
            .as_slice(),
        [CheckedIntegerProtocolObservation::Rejected {
            reason: CheckedIntegerProtocolRejectionReason::DynamicOperand,
            ..
        }]
    ));
}

#[test]
fn records_exact_owned_sites_spans_and_protected_status_without_mutation() {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let original = program.clone();
    let initial = observe_checked_integer_topologies(
        program
            .definitions
            .get(program.entry_function)
            .unwrap()
            .into(),
    )
    .unwrap();
    assert_eq!(
        program, original,
        "structural observation must be read-only"
    );

    let definition = entry_definition_mut(&mut program);
    let CheckedIntegerTopologyObservation::Protocol(topology) = &initial[0] else {
        panic!("expected checked topology");
    };
    let check_block = topology.check_block;
    let success = topology.success_block;
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
        active_predecessor: success,
        inactive_predecessor: success,
        merge: success,
        span: definition.span,
    });

    let observed = observe_checked_integer_topologies((&*definition).into()).unwrap();
    let CheckedIntegerTopologyObservation::Protocol(topology) = &observed[0] else {
        panic!("protected topology remains structurally observable");
    };
    assert_eq!(topology.check_block, check_block);
    assert!(topology.protected);
    assert_eq!(topology.result_store.block, topology.success_block);
    assert_eq!(
        topology.check_span,
        definition
            .block(check_block)
            .unwrap()
            .terminator
            .as_ref()
            .unwrap()
            .span()
    );

    assert!(matches!(
        observe_checked_integer_protocols((&*definition).into())
            .unwrap()
            .as_slice(),
        [CheckedIntegerProtocolObservation::Rejected {
            reason: CheckedIntegerProtocolRejectionReason::ProtectedTopology,
            ..
        }]
    ));
}

#[test]
fn malformed_identities_error_and_ordinary_shape_changes_reject() {
    let mut bad_block = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let owner = bad_block.entry_function;
    let definition = entry_definition_mut(&mut bad_block);
    let success = definition
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match block.terminator.as_mut() {
            Some(MirTerminator::IntegerDivisorCheck { success_target, .. }) => Some(success_target),
            _ => None,
        })
        .unwrap();
    *success = BlockId::new(FunctionId::new(owner.index() + 1), 0);
    assert!(matches!(
        observe_checked_integer_topologies((&*definition).into()),
        Err(MirRewriteError::InvalidReference {
            failure: MirReferenceFailure::Foreign,
            ..
        })
    ));

    let mut malformed = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let definition = entry_definition_mut(&mut malformed);
    let topology = observe_checked_integer_topologies((&*definition).into()).unwrap();
    let CheckedIntegerTopologyObservation::Protocol(topology) = &topology[0] else {
        unreachable!();
    };
    definition.body.blocks[topology.success_block.index()]
        .instructions
        .swap(0, 1);
    assert!(matches!(
        observe_checked_integer_topologies((&*definition).into())
            .unwrap()
            .as_slice(),
        [CheckedIntegerTopologyObservation::Rejected {
            reason: CheckedIntegerTopologyRejectionReason::NonCanonicalTopology,
            ..
        }]
    ));
}

#[test]
fn repeated_queries_preserve_callable_then_block_order() {
    let program = lower_source_to_final_mir(concat!(
        "fn first(value: i64) -> i64 { return (value / 2) + (value % 3); }\n",
        "fn second(value: u64) -> u64 { return (value << 2u) + (value >> 1u); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let first = observations(&program);
    assert_eq!(first, observations(&program));
    let blocks = first
        .iter()
        .map(|observation| match observation {
            CheckedIntegerTopologyObservation::Protocol(topology) => topology.check_block,
            CheckedIntegerTopologyObservation::Rejected { check_block, .. } => *check_block,
        })
        .collect::<Vec<_>>();
    assert!(blocks.windows(2).all(|window| {
        window[0].callable() < window[1].callable()
            || (window[0].callable() == window[1].callable()
                && window[0].index() < window[1].index())
    }));
}
