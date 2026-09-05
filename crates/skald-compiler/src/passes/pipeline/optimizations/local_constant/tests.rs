use crate::{
    mir::{
        rewrite::{
            storage_use_census_for_definition, MirStoragePlaceUse, MirStorageUseRole,
            MirStorageWriteAuthorization,
        },
        MirInstruction, MirPlace, MirPlaceBase, MirPlaceProjection, MirSharedRelease, MirStorage,
        MirStorageKind, MirType, MirValue, StorageId, ValueId,
    },
    test_support::lower_source_to_final_mir,
};

use super::carrier::{
    certify_checked_integer_carriers, CheckedCarrierCertificationObservation,
    CheckedCarrierProtocolRole, CheckedCarrierRejectionReason,
};

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

fn only_checked_storage(
    program: &crate::mir::MirProgram,
    role: CheckedCarrierProtocolRole,
) -> crate::mir::StorageId {
    certify_checked_integer_carriers(entry_definition(program).into())
        .unwrap()
        .into_iter()
        .find_map(|observation| match observation {
            CheckedCarrierCertificationObservation::Certified(certificate)
                if certificate.protocol_owner().role() == role =>
            {
                Some(certificate.storage())
            }
            _ => None,
        })
        .unwrap()
}

fn rejection_reasons(program: &crate::mir::MirProgram) -> Vec<CheckedCarrierRejectionReason> {
    certify_checked_integer_carriers(entry_definition(program).into())
        .unwrap()
        .into_iter()
        .filter_map(|observation| match observation {
            CheckedCarrierCertificationObservation::Rejected { reason, .. } => Some(reason),
            CheckedCarrierCertificationObservation::Certified(_) => None,
        })
        .collect()
}

#[test]
fn certifies_operand_and_result_carriers_with_exact_evidence() {
    let program = lower_source_to_final_mir("fn main() -> i64 { return 17 / 5; }");
    let observations = certify_checked_integer_carriers(entry_definition(&program).into()).unwrap();

    assert_eq!(observations.len(), 3);
    for (observation, role) in observations.iter().zip([
        CheckedCarrierProtocolRole::FirstOperand,
        CheckedCarrierProtocolRole::SecondOperand,
        CheckedCarrierProtocolRole::Result,
    ]) {
        let CheckedCarrierCertificationObservation::Certified(certificate) = observation else {
            panic!("canonical checked carrier must certify: {observation:?}");
        };
        assert_eq!(certificate.declaration().kind, MirStorageKind::ScalarSpill);
        assert_eq!(certificate.ty(), MirType::I64);
        assert_eq!(certificate.protocol_owner().role(), role);
        assert_eq!(
            certificate.protocol_owner().check_block().callable(),
            certificate.storage().callable()
        );
        assert_eq!(certificate.loads().len(), 1);
        assert_eq!(
            certificate.store().source().callable(),
            certificate.storage().callable()
        );
        assert_eq!(
            certificate.loads()[0].result().callable(),
            certificate.storage().callable()
        );
        assert!(certificate.lifetime().live().block.callable() == certificate.storage().callable());
        assert!(certificate.lifetime().dead().block.callable() == certificate.storage().callable());
        let _ = certificate.store().site();
        let _ = certificate.store().span();
        let _ = certificate.loads()[0].site();
        let _ = certificate.loads()[0].span();
    }
}

#[test]
fn census_is_deterministic_and_classifies_checked_carrier_accesses() {
    let program = lower_source_to_final_mir("fn main() -> i64 { return 8 >> 2u; }");
    let definition = entry_definition(&program);
    let first = storage_use_census_for_definition(definition.into()).unwrap();
    assert_eq!(
        first,
        storage_use_census_for_definition(definition.into()).unwrap()
    );

    let carrier = only_checked_storage(&program, CheckedCarrierProtocolRole::FirstOperand);
    let entry = first.get(carrier).unwrap();
    assert_eq!(entry.storage(), carrier);
    assert!(matches!(
        entry.declaration(),
        crate::mir::rewrite::MirLocalIdentitySite::StorageDeclaration(_)
    ));
    assert!(entry.uses().iter().any(|site| {
        site.role()
            == MirStorageUseRole::OrdinaryWrite {
                place: MirStoragePlaceUse::ExactBase,
                authorization: MirStorageWriteAuthorization::None,
            }
    }));
    assert!(entry.uses().iter().any(|site| {
        site.role() == MirStorageUseRole::OrdinaryRead(MirStoragePlaceUse::ExactBase)
    }));
    assert!(entry
        .uses()
        .iter()
        .any(|site| site.role() == MirStorageUseRole::CheckedProtocol));
    assert!(entry
        .uses()
        .iter()
        .any(|site| site.role() == MirStorageUseRole::LifetimeLive));
    assert!(entry
        .uses()
        .iter()
        .any(|site| site.role() == MirStorageUseRole::LifetimeDead));
}

#[test]
fn rejects_wrong_kind_and_type_without_considering_unrelated_spills() {
    let mut wrong_kind = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let carrier = only_checked_storage(&wrong_kind, CheckedCarrierProtocolRole::FirstOperand);
    entry_definition_mut(&mut wrong_kind).storage[carrier.index()].kind =
        MirStorageKind::PathCondition;
    // Topology itself becomes non-canonical, so no carrier observation leaks
    // through the topology boundary.
    assert!(
        certify_checked_integer_carriers(entry_definition(&wrong_kind).into())
            .unwrap()
            .is_empty()
    );

    let mut wrong_type = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let carrier = only_checked_storage(&wrong_type, CheckedCarrierProtocolRole::FirstOperand);
    entry_definition_mut(&mut wrong_type).storage[carrier.index()].ty = MirType::U64;
    assert!(
        certify_checked_integer_carriers(entry_definition(&wrong_type).into())
            .unwrap()
            .is_empty()
    );

    let mut unrelated = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let definition = entry_definition_mut(&mut unrelated);
    let id = StorageId::new(definition.callable(), definition.storage.len());
    definition.storage.push(MirStorage {
        id,
        source: None,
        name: "unrelated-spill".to_owned(),
        kind: MirStorageKind::ScalarSpill,
        ty: MirType::I64,
        span: definition.span,
    });
    assert_eq!(
        certify_checked_integer_carriers(entry_definition(&unrelated).into())
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn rejects_multiple_authorized_projected_and_extra_load_accesses() {
    let base = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let carrier = only_checked_storage(&base, CheckedCarrierProtocolRole::FirstOperand);

    let mut duplicate = base.clone();
    let definition = entry_definition_mut(&mut duplicate);
    let store = definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Store(store) if store.destination == MirPlace::base(carrier) => {
                Some(store.clone())
            }
            _ => None,
        })
        .unwrap();
    definition.body.blocks[0]
        .instructions
        .insert(1, MirInstruction::Store(store));
    assert!(rejection_reasons(&duplicate)
        .contains(&CheckedCarrierRejectionReason::MissingOrMultipleStores));

    let mut authorized = base.clone();
    let definition = entry_definition_mut(&mut authorized);
    let store = definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Store(store) if store.destination == MirPlace::base(carrier) => {
                Some(store)
            }
            _ => None,
        })
        .unwrap();
    let field = crate::identity::FieldId::new(crate::identity::ClassId::new(0), 0);
    store.authorization = Some(crate::mir::MirCellWriteAuthorization { field });
    assert!(rejection_reasons(&authorized).contains(&CheckedCarrierRejectionReason::InvalidAccess));

    let mut projected = base.clone();
    let definition = entry_definition_mut(&mut projected);
    let store = definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Store(store) if store.destination == MirPlace::base(carrier) => {
                Some(store)
            }
            _ => None,
        })
        .unwrap();
    store
        .destination
        .projections
        .push(MirPlaceProjection::Field(field));
    assert!(rejection_reasons(&projected).contains(&CheckedCarrierRejectionReason::InvalidAccess));

    let mut extra_load = base;
    let definition = entry_definition_mut(&mut extra_load);
    let mut load = definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(assignment.rvalue.kind, crate::mir::MirRvalueKind::Load(ref place) if *place == MirPlace::base(carrier)) =>
            {
                Some(assignment.clone())
            }
            _ => None,
        })
        .unwrap();
    let result = ValueId::new(definition.callable(), definition.values.len());
    load.result = result;
    definition.values.push(MirValue {
        id: result,
        ty: load.rvalue.ty,
        span: load.span,
    });
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::Assign(load));
    assert!(rejection_reasons(&extra_load)
        .contains(&CheckedCarrierRejectionReason::MissingOrMultipleLoads));
}

#[test]
fn rejects_attachment_and_lifetime_barriers() {
    let base = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let carrier = only_checked_storage(&base, CheckedCarrierProtocolRole::FirstOperand);

    let mut attached = base.clone();
    entry_definition_mut(&mut attached).parameters.push(carrier);
    assert!(rejection_reasons(&attached).contains(&CheckedCarrierRejectionReason::InvalidAccess));

    let mut no_dead = base;
    let definition = entry_definition_mut(&mut no_dead);
    for block in &mut definition.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(instruction, MirInstruction::StorageDead(dead) if dead.storage == carrier)
        });
    }
    assert!(rejection_reasons(&no_dead)
        .contains(&CheckedCarrierRejectionReason::MissingOrMultipleLifetimeMarkers));
}

#[test]
fn rejects_alias_call_ownership_and_cross_callable_accesses() {
    let base = lower_source_to_final_mir(concat!(
        "fn identity(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { return identity(8) / 2; }\n",
    ));
    let carrier = only_checked_storage(&base, CheckedCarrierProtocolRole::FirstOperand);

    let mut alias = base.clone();
    let definition = entry_definition_mut(&mut alias);
    let load = definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(assignment.rvalue.kind, crate::mir::MirRvalueKind::Load(ref place) if *place == MirPlace::base(carrier)) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .unwrap();
    let crate::mir::MirRvalueKind::Load(place) = &mut load.rvalue.kind else {
        unreachable!();
    };
    place.base = MirPlaceBase::AliasParameter(carrier);
    assert!(
        certify_checked_integer_carriers(entry_definition(&alias).into())
            .unwrap()
            .is_empty()
    );

    let mut call = base.clone();
    let definition = entry_definition_mut(&mut call);
    definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap()
        .shared_result = Some(carrier);
    assert!(rejection_reasons(&call).contains(&CheckedCarrierRejectionReason::InvalidAccess));

    let mut ownership = base.clone();
    let definition = entry_definition_mut(&mut ownership);
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::SharedRelease(MirSharedRelease {
            owner: carrier,
            span: definition.span,
        }));
    assert!(rejection_reasons(&ownership).contains(&CheckedCarrierRejectionReason::InvalidAccess));

    let mut foreign = base;
    let definition = entry_definition_mut(&mut foreign);
    let foreign_storage = StorageId::new(
        crate::identity::FunctionId::new(usize::MAX),
        carrier.index(),
    );
    definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::StorageDead(dead) if dead.storage == carrier => Some(dead),
            _ => None,
        })
        .unwrap()
        .storage = foreign_storage;
    assert!(matches!(
        certify_checked_integer_carriers(entry_definition(&foreign).into()),
        Err(crate::mir::rewrite::MirRewriteError::InvalidReference {
            failure: crate::mir::rewrite::MirReferenceFailure::Foreign,
            ..
        })
    ));
}

#[test]
fn rejects_non_dominating_store_and_misordered_lifetime() {
    let base = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let carrier = only_checked_storage(&base, CheckedCarrierProtocolRole::FirstOperand);

    let mut nondominating = base.clone();
    let definition = entry_definition_mut(&mut nondominating);
    let load_block = definition
        .body
        .blocks
        .iter()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(instruction, MirInstruction::Assign(assignment) if matches!(assignment.rvalue.kind, crate::mir::MirRvalueKind::Load(ref place) if *place == MirPlace::base(carrier)))
            })
        })
        .unwrap()
        .id;
    definition.body.entry = load_block;
    assert!(rejection_reasons(&nondominating)
        .contains(&CheckedCarrierRejectionReason::StoreDoesNotDominateLoad));

    let mut lifetime = base;
    let definition = entry_definition_mut(&mut lifetime);
    let block = definition
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block.instructions.iter().any(
                |instruction| matches!(instruction, MirInstruction::Store(store) if store.destination == MirPlace::base(carrier)),
            )
        })
        .unwrap();
    let live = block
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::StorageLive(live) if live.storage == carrier)
        })
        .unwrap();
    let store = block
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Store(store) if store.destination == MirPlace::base(carrier))
        })
        .unwrap();
    block.instructions.swap(live, store);
    assert!(
        rejection_reasons(&lifetime).contains(&CheckedCarrierRejectionReason::IncompatibleLifetime)
    );
}

#[test]
fn census_classifies_alias_call_ownership_and_proof_roles() {
    let program = lower_source_to_final_mir(concat!(
        "class Item { init() {} }\n",
        "fn sink(ref value: i64) -> i64 { return value; }\n",
        "fn consume(item: Item) -> unit {}\n",
        "fn main() -> i64 { var value: i64 = 8; var item: Item = Item(); consume(item); return sink(value) + (8 / 2); }\n",
    ));
    let roles = program
        .executable_definitions()
        .flat_map(|definition| {
            storage_use_census_for_definition(definition)
                .unwrap()
                .iter()
                .flat_map(|entry| entry.uses().iter().map(|site| site.role()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert!(roles.contains(&MirStorageUseRole::Attachment));
    assert!(roles.contains(&MirStorageUseRole::Call));
    assert!(roles.contains(&MirStorageUseRole::OrdinaryRead(MirStoragePlaceUse::Alias)));
    assert!(roles.contains(&MirStorageUseRole::OwnershipOrLifecycle));

    let logical = lower_source_to_final_mir(concat!(
        "fn logical() -> bool { return true && false; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let definition = logical
        .executable_definitions()
        .find(|definition| !definition.logical_expressions().is_empty())
        .unwrap();
    let roles = storage_use_census_for_definition(definition)
        .unwrap()
        .iter()
        .flat_map(|entry| entry.uses().iter().map(|site| site.role()))
        .collect::<Vec<_>>();
    assert!(roles.contains(&MirStorageUseRole::ProofMetadata));
}

#[path = "tests/solver.rs"]
mod solver;
