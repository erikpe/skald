use crate::{
    identity::CallableId,
    mir::{
        rewrite::{rewrite_program, MirRewriteError},
        test_fixtures::{assign, storage_dead, storage_live, store, value},
        MirAliasAccess, MirArrayAnchorKind, MirInstruction, MirPlace, MirRvalueKind, MirStorage,
        MirStorageKind, MirType, StorageId, ValueId,
    },
    passes::{
        redundancy::analyze_dead_normalized_path_activations, run_mir_pipeline_measured,
        MirOptimizationProfile,
    },
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::passes::pipeline::{
    execution::{
        model::MirFinalPassChange, MirFinalPassCapability, MirFinalPassOutcome, MirPassData,
        MirPassFailure,
    },
    seal::{reseal_final_mir, UnverifiedFinalMirProgram},
};

const SINGLE_SOURCE: &str = "fn main() -> i64 {
    var flag: bool = true;
    if (flag && true) { return 1; }
    return 0;
}";

fn none() -> crate::passes::MirPassSchedule {
    crate::passes::resolve_mir_pass_schedule(MirOptimizationProfile::None, std::iter::empty())
        .unwrap()
}

fn normalized(source: &str) -> crate::passes::VerifiedFinalMirProgram {
    run_mir_pipeline_measured(lower_source_to_final_mir(source), &none())
        .result
        .unwrap()
}

fn make_activation_dead(
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

fn dead_single() -> crate::passes::VerifiedFinalMirProgram {
    let verified = normalized(SINGLE_SOURCE);
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

fn apply(
    verified: crate::passes::VerifiedFinalMirProgram,
    plan: MirFinalStorageCleanupPlan,
) -> (
    crate::passes::VerifiedFinalMirProgram,
    Vec<crate::mir::rewrite::MirCallableRewriteResult>,
) {
    let changed = match MirFinalPassCapability::new(verified).cleanup_dead_path_activations(plan) {
        Ok(changed) => changed,
        Err(_) => panic!("certified cleanup unexpectedly failed"),
    };
    let outcome = match changed.finish(MirPassData::changed(1)) {
        Ok(outcome) => outcome,
        Err(_) => panic!("exact cleanup accounting unexpectedly failed"),
    };
    let MirFinalPassOutcome::Changed {
        change: MirFinalPassChange::Rewrite(rewrite),
        ..
    } = outcome
    else {
        panic!("cleanup must produce a rewrite")
    };
    let reports = rewrite.callables().to_vec();
    let verified = reseal_final_mir(rewrite.into_unverified()).unwrap();
    (verified, reports)
}

#[test]
fn capability_removes_one_complete_protocol_and_dense_commit_reports_exact_changes() {
    let verified = dead_single();
    let owner = CallableId::Function(verified.entry_function);
    let observation = analyze_dead_normalized_path_activations(&verified);
    let candidate = observation.callables()[0].candidates()[0].clone();
    let expected_instructions = candidate.instructions().len();
    let expected_values = candidate.load_results().len();
    let definition = verified
        .program()
        .definitions
        .get(verified.entry_function)
        .unwrap();
    let retained_storage = definition
        .storage
        .iter()
        .map(|storage| storage.id)
        .find(|storage| storage.index() > candidate.storage().index())
        .expect("fixture must exercise dense storage remapping");
    let retained_value = definition
        .values
        .iter()
        .map(|value| value.id)
        .find(|value| {
            candidate
                .load_results()
                .iter()
                .all(|removed| value.index() > removed.id.index())
        })
        .expect("fixture must exercise dense value remapping");
    let plan = MirFinalStorageCleanupPlan::prepare(&verified).unwrap();

    assert_eq!(plan.changed_callables(), 1);
    assert_eq!(plan.summary().storages(), 1);
    assert_eq!(plan.summary().values(), expected_values);
    assert_eq!(plan.summary().instructions(), expected_instructions);

    let (cleaned, reports) = apply(verified, plan);
    let definition = cleaned
        .program()
        .definitions
        .get(cleaned.entry_function)
        .unwrap();
    assert!(definition
        .storage
        .iter()
        .all(|storage| storage.kind != MirStorageKind::NormalizedPathActivation));
    let report = reports
        .iter()
        .find(|report| report.callable == owner)
        .unwrap();
    assert_eq!(report.changes.storage.removed, 1);
    assert_eq!(report.changes.values.removed, expected_values);
    assert_eq!(report.changes.storage.inserted, 0);
    assert_eq!(report.changes.values.inserted, 0);
    assert!(matches!(
        report.maps.storage.committed(candidate.storage()),
        Err(MirRewriteError::DeletedIdentity { .. })
    ));
    assert_eq!(
        report.maps.storage.committed(retained_storage).unwrap(),
        StorageId::new(owner, retained_storage.index() - 1)
    );
    let removed_before = candidate
        .load_results()
        .iter()
        .filter(|removed| removed.id.index() < retained_value.index())
        .count();
    assert_eq!(
        report.maps.values.committed(retained_value).unwrap(),
        ValueId::new(owner, retained_value.index() - removed_before)
    );
    assert_eq!(
        cleaned.reachability(),
        &crate::passes::reachability::analyze_reachability(cleaned.program()).unwrap()
    );
    assert_eq!(
        analyze_dead_normalized_path_activations(&cleaned)
            .counts()
            .proven(),
        0
    );
}

#[test]
fn multiple_candidates_sharing_one_block_are_removed_in_one_batched_rewrite() {
    let verified = dead_single();
    let (mut program, authority) = verified.invalidate_for_final_transformation().into_parts();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let existing = definition
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::NormalizedPathActivation)
        .unwrap()
        .id;
    assert!(definition
        .body
        .blocks
        .iter()
        .any(|block| block.instructions.iter().any(
            |instruction| matches!(instruction, MirInstruction::Store(operation)
            if operation.destination == MirPlace::base(existing))
        )));
    let storage = StorageId::new(definition.callable(), definition.storage.len());
    let source = ValueId::new(definition.callable(), definition.values.len());
    let result = ValueId::new(definition.callable(), definition.values.len() + 1);
    let span = definition.span;
    definition.storage.push(MirStorage {
        id: storage,
        source: None,
        name: "second dead activation".to_owned(),
        kind: MirStorageKind::NormalizedPathActivation,
        ty: MirType::Bool,
        span,
    });
    definition.values.push(value(source, MirType::Bool, span));
    definition.values.push(value(result, MirType::Bool, span));
    definition.body.blocks[0].instructions.extend([
        assign(
            source,
            MirRvalueKind::ConstantBool(true),
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
    let verified =
        reseal_final_mir(UnverifiedFinalMirProgram::from_parts(program, authority)).unwrap();
    let plan = MirFinalStorageCleanupPlan::prepare(&verified).unwrap();
    assert_eq!(plan.summary().storages(), 2);
    let expected_instructions = plan.summary().instructions();

    let (cleaned, reports) = apply(verified, plan);
    let definition = cleaned
        .program()
        .definitions
        .get(cleaned.entry_function)
        .unwrap();
    assert!(definition
        .storage
        .iter()
        .all(|storage| storage.kind != MirStorageKind::NormalizedPathActivation));
    assert_eq!(reports[0].changes.storage.removed, 2);
    assert!(expected_instructions >= 4);
}

#[test]
fn declaration_instruction_and_use_changes_make_a_plan_stale_before_mutation() {
    let verified = dead_single();
    let plan = MirFinalStorageCleanupPlan::prepare(&verified).unwrap();
    let owner = CallableId::Function(verified.entry_function);
    let observation = analyze_dead_normalized_path_activations(&verified);
    let candidate = &observation.callables()[0].candidates()[0];

    let mut declaration_changed = verified.program().clone();
    declaration_changed
        .definitions
        .get_mut_for_test(declaration_changed.entry_function)
        .unwrap()
        .storage[candidate.storage().index()]
    .name
    .push_str(" stale");
    assert!(matches!(
        plan.validate_program(&declaration_changed),
        Err(MirRewriteError::StaleCallableSnapshot { callable, .. }) if callable == owner
    ));

    let mut instruction_changed = verified.program().clone();
    let site = &candidate.instructions()[0];
    instruction_changed
        .definitions
        .get_mut_for_test(instruction_changed.entry_function)
        .unwrap()
        .body
        .blocks[site.block().index()]
    .instructions
    .remove(site.instruction());
    assert!(matches!(
        plan.validate_program(&instruction_changed),
        Err(MirRewriteError::StaleCallableSnapshot { callable, .. }) if callable == owner
    ));

    let mut use_changed = verified.program().clone();
    let definition = use_changed
        .definitions
        .get_mut_for_test(use_changed.entry_function)
        .unwrap();
    let result = candidate.load_results()[0].id;
    let branch = definition
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            Some(crate::mir::MirTerminator::Branch { condition, .. }) => Some(condition),
            _ => None,
        })
        .unwrap();
    *branch = result;
    assert!(matches!(
        plan.validate_program(&use_changed),
        Err(MirRewriteError::StaleCallableSnapshot { callable, .. }) if callable == owner
    ));
}

#[test]
fn every_non_activation_storage_kind_is_rejected_by_the_exact_plan() {
    let verified = dead_single();
    let plan = MirFinalStorageCleanupPlan::prepare(&verified).unwrap();
    let candidate =
        analyze_dead_normalized_path_activations(&verified).callables()[0].candidates()[0].clone();
    let excluded = [
        MirStorageKind::Return,
        MirStorageKind::Receiver,
        MirStorageKind::Parameter,
        MirStorageKind::AliasParameter(MirAliasAccess::ReadOnly),
        MirStorageKind::AliasParameter(MirAliasAccess::Mutable),
        MirStorageKind::CheckedView(MirAliasAccess::ReadOnly),
        MirStorageKind::CheckedView(MirAliasAccess::Mutable),
        MirStorageKind::Local,
        MirStorageKind::Argument,
        MirStorageKind::Temporary,
        MirStorageKind::SharedAnchor,
        MirStorageKind::ScalarSpill,
        MirStorageKind::PrimitiveAlias,
        MirStorageKind::PathCondition,
        MirStorageKind::OptionalUnwrap,
        MirStorageKind::SharedAllocation,
        MirStorageKind::ArrayBacking,
        MirStorageKind::ArrayProduced,
        MirStorageKind::ArraySlice,
        MirStorageKind::ArrayPosition,
        MirStorageKind::ArrayAnchor(MirArrayAnchorKind::InlineOwner),
        MirStorageKind::ArrayAnchor(MirArrayAnchorKind::InlineBacking),
        MirStorageKind::ArrayAnchor(MirArrayAnchorKind::StableSharedOwner),
        MirStorageKind::ArrayAnchor(MirArrayAnchorKind::CopiedSharedOwner),
        MirStorageKind::ArrayAnchor(MirArrayAnchorKind::AdoptedSharedOwner),
        MirStorageKind::ArrayAnchor(MirArrayAnchorKind::SecuredOptionalSharedOwner),
        MirStorageKind::ArrayAlias(MirAliasAccess::ReadOnly),
        MirStorageKind::ArrayAlias(MirAliasAccess::Mutable),
    ];
    for kind in excluded {
        let mut changed = verified.program().clone();
        changed
            .definitions
            .get_mut_for_test(changed.entry_function)
            .unwrap()
            .storage[candidate.storage().index()]
        .kind = kind;
        assert!(
            matches!(
                plan.validate_program(&changed),
                Err(MirRewriteError::StaleCallableSnapshot { .. })
            ),
            "{kind:?}"
        );
    }
}

#[test]
fn invariant_rejects_partial_extra_instruction_value_and_storage_mutations() {
    let verified = dead_single();
    let observation = analyze_dead_normalized_path_activations(&verified);
    let candidates = observation.callables()[0].candidates().to_vec();
    let owner = CallableId::Function(verified.entry_function);
    let candidate = candidates[0].clone();

    let partial = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let invariant = MirFinalStorageCleanupInvariant::capture(edit, &candidates)?;
            edit.remove_storage(candidate.storage())?;
            invariant.verify(edit)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(
        partial,
        MirRewriteError::UnsupportedFinalStorageCleanupMutation { callable: owner }
    );

    let extra = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let invariant = MirFinalStorageCleanupInvariant::capture(edit, &candidates)?;
            MirFinalStorageCleanupEdit::new(edit).remove_dead_path_activations(&candidates)?;
            let retained = edit.storage_ids().next().unwrap();
            let kind = edit.storage(retained)?.kind;
            edit.replace_storage_kind(retained, kind, MirStorageKind::Temporary)?;
            invariant.verify(edit)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(
        extra,
        MirRewriteError::UnsupportedFinalStorageCleanupMutation { callable: owner }
    );

    let extra_value = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let invariant = MirFinalStorageCleanupInvariant::capture(edit, &candidates)?;
            MirFinalStorageCleanupEdit::new(edit).remove_dead_path_activations(&candidates)?;
            let retained = edit.value_ids().next().unwrap();
            edit.remove_value(retained)?;
            invariant.verify(edit)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(
        extra_value,
        MirRewriteError::UnsupportedFinalStorageCleanupMutation { callable: owner }
    );

    let extra_instruction = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let invariant = MirFinalStorageCleanupInvariant::capture(edit, &candidates)?;
            MirFinalStorageCleanupEdit::new(edit).remove_dead_path_activations(&candidates)?;
            let block = edit.block_order()[0];
            edit.rewrite_block_instructions(block, |instructions| {
                instructions.iter().skip(1).cloned().collect()
            })?;
            invariant.verify(edit)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(
        extra_instruction,
        MirRewriteError::UnsupportedFinalStorageCleanupMutation { callable: owner }
    );

    let storage_creation = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let invariant = MirFinalStorageCleanupInvariant::capture(edit, &candidates)?;
            MirFinalStorageCleanupEdit::new(edit).remove_dead_path_activations(&candidates)?;
            let template = edit.storage(edit.storage_ids().next().unwrap())?.clone();
            edit.allocate_storage(|id| MirStorage {
                id,
                source: None,
                name: "uncertified insertion".to_owned(),
                kind: MirStorageKind::Temporary,
                ty: template.ty,
                span: template.span,
            })?;
            invariant.verify(edit)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(
        storage_creation,
        MirRewriteError::UnsupportedFinalStorageCleanupMutation { callable: owner }
    );

    let unrelated_deletion = rewrite_program(verified.program().clone(), |callable, edit| {
        if callable == owner {
            let invariant = MirFinalStorageCleanupInvariant::capture(edit, &candidates)?;
            MirFinalStorageCleanupEdit::new(edit).remove_dead_path_activations(&candidates)?;
            let retained = edit.storage_ids().next().unwrap();
            edit.remove_storage(retained)?;
            invariant.verify(edit)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(
        unrelated_deletion,
        MirRewriteError::UnsupportedFinalStorageCleanupMutation { callable: owner }
    );
}

#[test]
fn stale_multi_callable_plan_is_rejected_atomically_before_invalidation() {
    let source = "fn helper(flag: bool) -> i64 {
        if (flag && true) { return 1; }
        return 0;
    }
    fn main() -> i64 {
        var flag: bool = true;
        if (flag && true) { return helper(flag); }
        return 0;
    }";
    let verified = normalized(source);
    let (mut program, authority) = verified.invalidate_for_final_transformation().into_parts();
    let helper = program
        .declarations
        .iter()
        .find(|declaration| declaration.name == "helper")
        .unwrap()
        .id;
    for function in [program.entry_function, helper] {
        let definition = program.definitions.get_mut_for_test(function).unwrap();
        let activations = definition
            .storage
            .iter()
            .filter(|storage| storage.kind == MirStorageKind::NormalizedPathActivation)
            .map(|storage| storage.id)
            .collect::<Vec<_>>();
        for activation in activations {
            make_activation_dead(definition, activation);
        }
    }
    let verified =
        reseal_final_mir(UnverifiedFinalMirProgram::from_parts(program, authority)).unwrap();
    let plan = MirFinalStorageCleanupPlan::prepare(&verified).unwrap();
    assert_eq!(plan.changed_callables(), 2);

    // Keep the first callable's candidate exact and make only the later entry
    // callable stale. Whole-program validation must still happen before the
    // capability invalidates or edits either callable.
    let partially_stale = normalized(source);
    let (mut partially_stale_program, partially_stale_authority) = partially_stale
        .invalidate_for_final_transformation()
        .into_parts();
    let helper = partially_stale_program
        .declarations
        .iter()
        .find(|declaration| declaration.name == "helper")
        .unwrap()
        .id;
    let definition = partially_stale_program
        .definitions
        .get_mut_for_test(helper)
        .unwrap();
    let activations = definition
        .storage
        .iter()
        .filter(|storage| storage.kind == MirStorageKind::NormalizedPathActivation)
        .map(|storage| storage.id)
        .collect::<Vec<_>>();
    for activation in activations {
        make_activation_dead(definition, activation);
    }
    let partially_stale = reseal_final_mir(UnverifiedFinalMirProgram::from_parts(
        partially_stale_program,
        partially_stale_authority,
    ))
    .unwrap();
    let original = partially_stale.program().clone();
    let retained_snapshot = partially_stale.clone();
    let stale_callable = CallableId::Function(partially_stale.entry_function);
    let error =
        match MirFinalPassCapability::new(partially_stale).cleanup_dead_path_activations(plan) {
            Ok(_) => panic!("stale multi-callable plan unexpectedly committed"),
            Err(error) => error,
        };
    assert!(matches!(
        error,
        MirPassFailure::Rewrite(MirRewriteError::StaleCallableSnapshot { callable, .. })
            if callable == stale_callable
    ));
    assert_eq!(
        analyze_dead_normalized_path_activations(&retained_snapshot)
            .counts()
            .proven(),
        1
    );
    assert_eq!(retained_snapshot.program(), &original);
}

#[test]
fn cfg_and_storage_cleanup_remain_distinct_closed_capabilities() {
    fn accept_cfg(_: &mut crate::passes::pipeline::execution::final_cfg::MirFinalCfgEdit<'_>) {}
    fn accept_cleanup(_: &mut MirFinalStorageCleanupEdit<'_>) {}

    let cfg: fn(&mut crate::passes::pipeline::execution::final_cfg::MirFinalCfgEdit<'_>) =
        accept_cfg;
    let cleanup: fn(&mut MirFinalStorageCleanupEdit<'_>) = accept_cleanup;
    assert_ne!(
        std::any::type_name_of_val(&cfg),
        std::any::type_name_of_val(&cleanup)
    );
}
