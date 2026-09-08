use crate::{
    identity::{CallableId, ClassId},
    mir::{
        test_fixtures::{assign, storage_dead, storage_live, store, value},
        MirInstruction, MirPlace, MirPlaceProjection, MirRvalueKind, MirStorage, MirStorageKind,
        MirTerminator, MirType, StorageId, ValueId,
    },
    passes::{run_mir_pipeline_measured, MirOptimizationProfile},
    test_support::lower_source_to_final_mir,
};

use super::*;

fn none() -> crate::passes::MirPassSchedule {
    crate::passes::resolve_mir_pass_schedule(MirOptimizationProfile::None, std::iter::empty())
        .unwrap()
}

fn normalized_verified() -> crate::passes::VerifiedFinalMirProgram {
    run_mir_pipeline_measured(
        lower_source_to_final_mir(
            "fn main() -> i64 {\n\
                 var flag: bool = true;\n\
                 if (flag && true) { return 1; }\n\
                 return 0;\n\
             }",
        ),
        &none(),
    )
    .result
    .unwrap()
}

fn normalized_program() -> crate::mir::MirProgram {
    normalized_verified().program().clone()
}

fn activation(program: &crate::mir::MirProgram) -> StorageId {
    program
        .definitions
        .get(program.entry_function)
        .unwrap()
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::NormalizedPathActivation)
        .map(|storage| storage.id)
        .expect("fixture must contain a normalized activation")
}

fn make_activation_dead(
    program: &mut crate::mir::MirProgram,
    activation: StorageId,
) -> Vec<ValueId> {
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let source = definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Store(store) if store.destination == MirPlace::base(activation) => {
                Some(store.value)
            }
            _ => None,
        })
        .expect("activation must have a store source");
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
    assert!(!loads.is_empty());
    let mut replaced = 0;
    for block in &mut definition.body.blocks {
        if let Some(MirTerminator::Branch { condition, .. }) = &mut block.terminator {
            if loads.contains(condition) {
                *condition = source;
                replaced += 1;
            }
        }
    }
    assert_eq!(replaced, loads.len(), "fixture loads must feed branches");
    loads
}

fn analyze_entry(
    program: &crate::mir::MirProgram,
) -> Result<DeadPathActivationCallableObservation, MirRewriteError> {
    analyze_unverified_definition(
        program
            .definitions
            .get(program.entry_function)
            .unwrap()
            .into(),
    )
}

#[test]
fn verified_analysis_is_read_only_deterministic_and_blocks_material_loads() {
    let verified = normalized_verified();
    let program = verified.program().clone();
    let first = analyze_dead_normalized_path_activations(&verified);
    let second = analyze_dead_normalized_path_activations(&verified);

    assert_eq!(first, second);
    assert_eq!(verified.program(), &program);
    assert_eq!(first.counts().inspected(), 1);
    assert_eq!(first.counts().proven(), 0);
    assert_eq!(first.counts().blocked(), 1);
    assert_eq!(first.counts().interesting(), 1);
    assert_eq!(first.counts().non_candidates(), 0);
    assert_eq!(first.counts().affected_callables(), 1);
    assert_eq!(
        first.callables()[0].blocked_activations()[0].blockers(),
        &[DeadPathActivationBlocker::MaterialLoadResult]
    );
}

#[test]
fn dead_protocol_owns_exact_declaration_instruction_and_load_result_snapshots() {
    let mut program = normalized_program();
    let selected = activation(&program);
    let load_results = make_activation_dead(&mut program, selected);
    let observation = analyze_entry(&program).unwrap();
    let candidate = &observation.candidates()[0];

    assert_eq!(observation.counts().inspected(), 1);
    assert_eq!(observation.counts().proven(), 1);
    assert_eq!(observation.counts().blocked(), 0);
    assert_eq!(candidate.storage(), selected);
    assert_eq!(candidate.declaration_index(), selected.index());
    assert_eq!(candidate.declaration().source, None);
    assert_eq!(candidate.declaration().ty, MirType::Bool);
    assert_eq!(
        candidate
            .load_results()
            .iter()
            .map(|value| value.id)
            .collect::<Vec<_>>(),
        load_results
    );
    assert!(candidate.instructions().windows(2).all(|pair| {
        (pair[0].block(), pair[0].instruction()) < (pair[1].block(), pair[1].instruction())
    }));
    for site in candidate.instructions() {
        let definition = program.definitions.get(program.entry_function).unwrap();
        assert_eq!(
            site.expected(),
            &definition.body.blocks[site.block().index()].instructions[site.instruction()]
        );
    }
    assert_eq!(
        candidate.removable_values_upper_bound(),
        u64::try_from(load_results.len()).unwrap()
    );
    assert_eq!(observation.counts().removable_storages_upper_bound(), 1);
    assert_eq!(
        observation.counts().removable_instructions_upper_bound(),
        candidate.removable_instructions_upper_bound()
    );
    assert_eq!(observation.examples()[0].storage(), selected);
    assert!(observation.examples()[0].reasons().is_empty());
}

#[test]
fn multiple_exact_protocol_sites_contribute_to_one_atomic_candidate() {
    let mut program = normalized_program();
    let selected = activation(&program);
    make_activation_dead(&mut program, selected);
    let original = analyze_entry(&program).unwrap().candidates()[0].clone();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    for site in original.instructions() {
        let mut duplicate = site.expected().clone();
        if let MirInstruction::Assign(assignment) = &mut duplicate {
            let id = ValueId::new(definition.callable(), definition.values.len());
            let mut declaration = definition.values[assignment.result.index()].clone();
            declaration.id = id;
            definition.values.push(declaration);
            assignment.result = id;
        }
        definition.body.blocks[site.block().index()]
            .instructions
            .push(duplicate);
    }

    let observation = analyze_entry(&program).unwrap();
    let candidate = &observation.candidates()[0];
    for kind in [
        DeadPathActivationInstructionKind::Load,
        DeadPathActivationInstructionKind::Store,
        DeadPathActivationInstructionKind::LifetimeLive,
        DeadPathActivationInstructionKind::LifetimeDead,
    ] {
        let original_count = original
            .instructions()
            .iter()
            .filter(|site| site.kind() == kind)
            .count();
        let expanded_count = candidate
            .instructions()
            .iter()
            .filter(|site| site.kind() == kind)
            .count();
        assert_eq!(expanded_count, original_count * 2, "{kind:?}");
    }
    assert_eq!(
        candidate.load_results().len(),
        original.load_results().len() * 2
    );
    assert_eq!(
        observation.counts().removable_instructions_upper_bound(),
        original.removable_instructions_upper_bound() * 2
    );
}

#[test]
fn declaration_only_candidates_are_valid_and_candidate_order_is_dense() {
    let mut program = normalized_program();
    let selected = activation(&program);
    make_activation_dead(&mut program, selected);
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    for index in 0..2 {
        let id = StorageId::new(definition.callable(), definition.storage.len());
        definition.storage.push(MirStorage {
            id,
            source: None,
            name: format!("unused activation {index}"),
            kind: MirStorageKind::NormalizedPathActivation,
            ty: MirType::Bool,
            span: definition.span,
        });
    }

    let observation = analyze_entry(&program).unwrap();
    assert_eq!(observation.counts().proven(), 3);
    assert_eq!(observation.counts().removable_storages_upper_bound(), 3);
    assert!(observation
        .candidates()
        .windows(2)
        .all(|pair| pair[0].storage() < pair[1].storage()));
    assert!(observation.candidates()[1].instructions().is_empty());
    assert!(observation.candidates()[1].load_results().is_empty());
    let declaration_only = observation
        .examples()
        .iter()
        .find(|example| example.storage() == observation.candidates()[1].storage())
        .unwrap();
    assert_eq!(declaration_only.block(), None);
    assert_eq!(declaration_only.instruction(), None);
}

#[test]
fn one_exact_load_store_and_lifetime_pair_form_a_complete_candidate() {
    let mut program = normalized_program();
    let existing = activation(&program);
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let source = definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Store(operation)
                if operation.destination == MirPlace::base(existing) =>
            {
                Some(operation.value)
            }
            _ => None,
        })
        .unwrap();
    let storage_id = StorageId::new(definition.callable(), definition.storage.len());
    let value_id = ValueId::new(definition.callable(), definition.values.len());
    let span = definition.span;
    definition.storage.push(MirStorage {
        id: storage_id,
        source: None,
        name: "one exact protocol".to_owned(),
        kind: MirStorageKind::NormalizedPathActivation,
        ty: MirType::Bool,
        span,
    });
    definition.values.push(value(value_id, MirType::Bool, span));
    definition.body.blocks[0].instructions.extend([
        storage_live(storage_id, span),
        store(MirPlace::base(storage_id), source, span),
        assign(
            value_id,
            MirRvalueKind::Load(MirPlace::base(storage_id)),
            MirType::Bool,
            span,
        ),
        storage_dead(storage_id, span),
    ]);

    let observation = analyze_entry(&program).unwrap();
    let candidate = observation
        .candidates()
        .iter()
        .find(|candidate| candidate.storage() == storage_id)
        .unwrap();
    assert_eq!(candidate.instructions().len(), 4);
    assert_eq!(candidate.load_results().len(), 1);
    for kind in [
        DeadPathActivationInstructionKind::Load,
        DeadPathActivationInstructionKind::Store,
        DeadPathActivationInstructionKind::LifetimeLive,
        DeadPathActivationInstructionKind::LifetimeDead,
    ] {
        assert_eq!(
            candidate
                .instructions()
                .iter()
                .filter(|site| site.kind() == kind)
                .count(),
            1,
            "{kind:?}"
        );
    }
}

#[test]
fn source_and_type_contracts_block_only_normalized_activations() {
    let mut program = normalized_program();
    let selected = activation(&program);
    make_activation_dead(&mut program, selected);
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    definition.storage[selected.index()].source =
        Some(crate::identity::BindingId::Receiver(definition.callable()));
    definition.storage[selected.index()].ty = MirType::U64;
    let blocked = analyze_entry(&program).unwrap();
    assert_eq!(
        blocked.blocked_activations()[0].blockers(),
        &[
            DeadPathActivationBlocker::SourceBinding,
            DeadPathActivationBlocker::NonBooleanStorage,
        ]
    );

    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    definition.storage[selected.index()].kind = MirStorageKind::ScalarSpill;
    let ignored = analyze_entry(&program).unwrap();
    assert_eq!(ignored.counts().inspected(), 0);
    assert!(ignored.candidates().is_empty());
    assert!(ignored.blocked_activations().is_empty());
}

#[test]
fn load_result_must_have_the_exact_boolean_declaration() {
    let mut program = normalized_program();
    let selected = activation(&program);
    let loads = make_activation_dead(&mut program, selected);
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    definition.values[loads[0].index()].ty = MirType::U64;

    let observation = analyze_entry(&program).unwrap();
    assert_eq!(observation.counts().proven(), 0);
    assert_eq!(
        observation.blocked_activations()[0].blockers(),
        &[DeadPathActivationBlocker::MalformedLoadResult]
    );
}

#[test]
fn every_storage_role_and_place_shape_has_an_explicit_disposition() {
    let direct = [
        MirStorageUseRole::LifetimeLive,
        MirStorageUseRole::LifetimeDead,
        MirStorageUseRole::OrdinaryRead(MirStoragePlaceUse::ExactBase),
        MirStorageUseRole::OrdinaryWrite {
            place: MirStoragePlaceUse::ExactBase,
            authorization: MirStorageWriteAuthorization::None,
        },
    ];
    assert_eq!(
        direct.map(use_disposition),
        [
            UseDisposition::Lifetime(DeadPathActivationInstructionKind::LifetimeLive),
            UseDisposition::Lifetime(DeadPathActivationInstructionKind::LifetimeDead),
            UseDisposition::Load,
            UseDisposition::Store,
        ]
    );

    for place in [MirStoragePlaceUse::Projected, MirStoragePlaceUse::Alias] {
        assert_eq!(
            use_disposition(MirStorageUseRole::OrdinaryRead(place)),
            UseDisposition::Blocker(DeadPathActivationBlocker::NoncanonicalReadPlace)
        );
        for authorization in [
            MirStorageWriteAuthorization::None,
            MirStorageWriteAuthorization::Cell,
            MirStorageWriteAuthorization::Final,
            MirStorageWriteAuthorization::CellAndFinal,
        ] {
            assert_eq!(
                use_disposition(MirStorageUseRole::OrdinaryWrite {
                    place,
                    authorization,
                }),
                UseDisposition::Blocker(DeadPathActivationBlocker::NoncanonicalWritePlace)
            );
        }
    }
    for authorization in [
        MirStorageWriteAuthorization::Cell,
        MirStorageWriteAuthorization::Final,
        MirStorageWriteAuthorization::CellAndFinal,
    ] {
        assert_eq!(
            use_disposition(MirStorageUseRole::OrdinaryWrite {
                place: MirStoragePlaceUse::ExactBase,
                authorization,
            }),
            UseDisposition::Blocker(DeadPathActivationBlocker::AuthorizedWrite)
        );
    }

    let barriers = [
        (
            MirStorageUseRole::Attachment,
            DeadPathActivationBlocker::Attachment,
        ),
        (
            MirStorageUseRole::CheckedProtocol,
            DeadPathActivationBlocker::CheckedProtocol,
        ),
        (
            MirStorageUseRole::ProofMetadata,
            DeadPathActivationBlocker::ProofMetadata,
        ),
        (
            MirStorageUseRole::Alias,
            DeadPathActivationBlocker::AliasExposure,
        ),
        (MirStorageUseRole::Call, DeadPathActivationBlocker::Call),
        (
            MirStorageUseRole::OwnershipOrLifecycle,
            DeadPathActivationBlocker::OwnershipOrLifecycle,
        ),
        (
            MirStorageUseRole::InputOutput,
            DeadPathActivationBlocker::InputOutput,
        ),
        (
            MirStorageUseRole::OtherExecutable,
            DeadPathActivationBlocker::OtherExecutable,
        ),
    ];
    for (role, blocker) in barriers {
        assert_eq!(use_disposition(role), UseDisposition::Blocker(blocker));
    }
}

#[test]
fn projected_reads_and_stale_observations_fail_closed() {
    let mut program = normalized_program();
    let selected = activation(&program);
    let loads = make_activation_dead(&mut program, selected);
    let original = analyze_entry(&program).unwrap();
    let original_candidate = original.candidates()[0].clone();

    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let load = definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) if assignment.result == loads[0] => Some(assignment),
            _ => None,
        })
        .unwrap();
    let MirRvalueKind::Load(place) = &mut load.rvalue.kind else {
        panic!("expected load");
    };
    place
        .projections
        .push(MirPlaceProjection::Base(ClassId::new(0)));

    let fresh = analyze_entry(&program).unwrap();
    assert_eq!(fresh.counts().proven(), 0);
    assert!(fresh.blocked_activations()[0]
        .blockers()
        .contains(&DeadPathActivationBlocker::NoncanonicalReadPlace));
    assert_eq!(original.candidates()[0], original_candidate);
    assert!(fresh.candidates().is_empty());
}

#[test]
fn malformed_dense_identity_is_reported_instead_of_observed_as_a_candidate() {
    let mut program = normalized_program();
    let selected = activation(&program);
    let loads = make_activation_dead(&mut program, selected);
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    definition.values[loads[0].index()].id =
        ValueId::new(CallableId::Function(program.entry_function), usize::MAX);

    assert!(analyze_entry(&program).is_err());
}

#[test]
fn count_merging_saturates_sums_and_preserves_maximum_protocol_size() {
    let mut total = Accumulator::default();
    total.counts.inspected = u64::MAX;
    total.counts.maximum_protocol_size = 11;
    let mut next = Accumulator::default();
    next.counts.inspected = 1;
    next.counts.maximum_protocol_size = 7;
    total.merge(&next);

    assert_eq!(total.counts.inspected(), u64::MAX);
    assert_eq!(total.counts.maximum_protocol_size(), 11);
    assert!(total.counts.saturated());
}
