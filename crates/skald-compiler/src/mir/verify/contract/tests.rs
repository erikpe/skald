use crate::{
    mir::{
        dump_mir, rewrite::MirLocalIdentitySite, verify_mir, BlockId, MirAliasAccess,
        MirArrayAnchorKind, MirInstruction, MirPlace, MirPlaceBase, MirRvalueKind, MirStorageKind,
        MirTerminator, MirType,
    },
    test_support::lower_source_to_mir,
};

use super::{
    classify_instruction, classify_local_identity_site, classify_proof_record,
    classify_rvalue_kind, classify_storage_kind, classify_storage_phase_availability,
    MirIdentitySiteRole, MirProofDisposition, MirProofRecordKind, MirStoragePhaseAvailability,
    MirVerificationContract,
};
use crate::mir::verify::check_normalized_mir;

#[test]
fn only_permanent_semantic_forms_survive_normalization() {
    assert!(MirProofDisposition::PermanentSemantic.is_permanent());
    assert!(!MirProofDisposition::ConsumableProof.is_permanent());
    assert!(!MirProofDisposition::ExecutableCarrierWithProof.is_permanent());
}

#[test]
fn classification_separates_consumed_proof_from_permanent_attachments() {
    assert_eq!(
        classify_local_identity_site(MirLocalIdentitySite::PathCondition(0)),
        MirIdentitySiteRole::ConsumableProof
    );
    assert_eq!(
        classify_local_identity_site(MirLocalIdentitySite::LogicalExpression(0)),
        MirIdentitySiteRole::ConsumableProof
    );
    assert_eq!(
        classify_local_identity_site(MirLocalIdentitySite::StaticPublicationInitializationExit),
        MirIdentitySiteRole::PermanentAttachment
    );
    assert_eq!(
        classify_local_identity_site(MirLocalIdentitySite::BodyEntry),
        MirIdentitySiteRole::BodyEntry
    );
    assert_eq!(
        classify_local_identity_site(MirLocalIdentitySite::Instruction {
            block: 0,
            instruction: 0,
        }),
        MirIdentitySiteRole::Ordinary
    );
}

#[test]
fn classification_identifies_only_path_carrier_forms_as_mixed() {
    let program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let definition = program.definitions.get(program.entry_function).unwrap();
    let assignment = definition.body.blocks[0]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) => Some(assignment),
            _ => None,
        })
        .expect("return literal must have one assignment");

    assert_eq!(
        classify_proof_record(MirProofRecordKind::PathCondition),
        MirProofDisposition::ConsumableProof
    );
    assert_eq!(
        classify_proof_record(MirProofRecordKind::LogicalExpression),
        MirProofDisposition::ConsumableProof
    );
    assert_eq!(
        classify_storage_kind(MirStorageKind::PathCondition),
        MirProofDisposition::ExecutableCarrierWithProof
    );
    assert_eq!(
        classify_storage_kind(MirStorageKind::ScalarSpill),
        MirProofDisposition::PermanentSemantic
    );
    assert_eq!(
        classify_storage_kind(MirStorageKind::NormalizedPathActivation),
        MirProofDisposition::PermanentSemantic
    );
    assert_eq!(
        classify_rvalue_kind(&assignment.rvalue.kind),
        MirProofDisposition::PermanentSemantic
    );
    assert_eq!(
        classify_instruction(&MirInstruction::Assign(assignment.clone())),
        MirProofDisposition::PermanentSemantic
    );
}

#[test]
fn every_current_storage_kind_has_one_explicit_phase_availability() {
    let phase_stable_kinds = [
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

    for kind in phase_stable_kinds {
        let availability = classify_storage_phase_availability(kind);
        assert_eq!(availability, MirStoragePhaseAvailability::Both, "{kind:?}");
        assert!(availability.permits(MirVerificationContract::ProofRich));
        assert!(availability.permits(MirVerificationContract::Normalized));
    }

    let path_condition = classify_storage_phase_availability(MirStorageKind::PathCondition);
    assert_eq!(path_condition, MirStoragePhaseAvailability::ProofRichOnly);
    assert!(path_condition.permits(MirVerificationContract::ProofRich));
    assert!(!path_condition.permits(MirVerificationContract::Normalized));

    let normalized_activation =
        classify_storage_phase_availability(MirStorageKind::NormalizedPathActivation);
    assert_eq!(
        normalized_activation,
        MirStoragePhaseAvailability::NormalizedOnly
    );
    assert!(!normalized_activation.permits(MirVerificationContract::ProofRich));
    assert!(normalized_activation.permits(MirVerificationContract::Normalized));
}

#[test]
fn normalized_path_activation_has_one_narrow_semantic_query() {
    assert!(MirStorageKind::NormalizedPathActivation.is_normalized_path_activation());
    assert!(!MirStorageKind::PathCondition.is_normalized_path_activation());
    assert!(!MirStorageKind::ScalarSpill.is_normalized_path_activation());
}

#[test]
fn proof_rich_contract_rejects_normalized_path_activation_storage() {
    let mut program =
        lower_source_to_mir("fn main() -> i64 { var active: bool = true; return 0; }");
    reclassify_first_local(&mut program, MirStorageKind::NormalizedPathActivation, true);

    let errors = verify_mir(&program)
        .expect_err("normalized-only storage must not cross the proof-rich seal")
        .to_string();
    assert!(
        errors.contains("is legal only in normalized MIR"),
        "{errors}"
    );
}

#[test]
fn normalized_activation_declaration_contract_is_explicit() {
    let source = "fn main() -> i64 { var active: bool = true; return 0; }";
    let mut valid = lower_source_to_mir(source);
    reclassify_first_local(&mut valid, MirStorageKind::NormalizedPathActivation, true);
    check_normalized_mir(&valid)
        .expect("a generated boolean normalized activation must be structurally valid");
    let dump = dump_mir(&valid);
    assert!(dump.contains("normalized-path-activation <normalized-path-activation> \"active\""));

    let mut source_backed = lower_source_to_mir(source);
    reclassify_first_local(
        &mut source_backed,
        MirStorageKind::NormalizedPathActivation,
        false,
    );
    let errors = check_normalized_mir(&source_backed)
        .expect_err("source bindings cannot claim compiler-owned activation storage")
        .to_string();
    assert!(
        errors.contains("kind does not match its source binding"),
        "{errors}"
    );

    let mut wrong_type =
        lower_source_to_mir("fn main() -> i64 { var result: i64 = 7; return result; }");
    let storage = reclassify_first_local(
        &mut wrong_type,
        MirStorageKind::NormalizedPathActivation,
        true,
    );
    assert_eq!(
        wrong_type
            .definitions
            .get(wrong_type.entry_function)
            .unwrap()
            .storage(storage)
            .unwrap()
            .ty,
        MirType::I64
    );
    let errors = check_normalized_mir(&wrong_type)
        .expect_err("normalized activations must have boolean storage")
        .to_string();
    assert!(
        errors.contains("normalized path-activation storage") && errors.contains("must be `bool`"),
        "{errors}"
    );
}

#[test]
fn normalized_contract_accepts_path_free_executable_mir() {
    let program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    check_normalized_mir(&program).expect("ordinary executable MIR must normalize structurally");
}

#[test]
fn normalized_contract_reuses_shared_structural_checks() {
    let mut program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let entry = program.entry_function;
    let definition = program.definitions.get_mut_for_test(entry).unwrap();
    let block = &mut definition.body.blocks[0];
    let span = block.terminator.as_ref().unwrap().span();
    block.terminator = Some(MirTerminator::Goto {
        target: BlockId::new(entry, 99),
        span,
    });

    let errors = check_normalized_mir(&program)
        .expect_err("normalized MIR must retain ordinary structural verification")
        .to_string();
    assert!(errors.contains("control-flow target"), "{errors}");
    assert!(errors.contains("is not declared"), "{errors}");
}

#[test]
fn normalized_contract_rejects_every_current_path_carrier_family() {
    let program =
        lower_source_to_mir("fn main() -> i64 { if (true && false) { return 1; } return 0; }");
    verify_mir(&program).expect("proof-rich verification must remain valid");
    let errors = check_normalized_mir(&program)
        .expect_err("proof-rich logical MIR must not satisfy the normalized contract")
        .to_string();

    assert!(errors.contains("path-condition record(s)"), "{errors}");
    assert!(errors.contains("logical-expression record(s)"), "{errors}");
    assert!(
        errors.contains("normalized MIR storage")
            && errors.contains("retains executable carrier with proof provenance"),
        "{errors}"
    );
    assert!(
        errors.contains("normalized MIR value")
            && errors.contains("retains executable carrier with proof provenance"),
        "{errors}"
    );
}

#[test]
fn mechanically_normalized_logical_shape_reaches_shared_checks() {
    let mut program =
        lower_source_to_mir("fn main() -> i64 { if (true && false) { return 1; } return 0; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    for storage in &mut definition.storage {
        if storage.kind == MirStorageKind::PathCondition {
            storage.kind = MirStorageKind::ScalarSpill;
        }
    }
    for block in &mut definition.body.blocks {
        for instruction in &mut block.instructions {
            let MirInstruction::Assign(assignment) = instruction else {
                continue;
            };
            let activation = match &assignment.rvalue.kind {
                MirRvalueKind::PathCondition(condition) => condition.activation,
                _ => continue,
            };
            assignment.rvalue.kind = MirRvalueKind::Load(MirPlace::base(activation));
        }
    }
    definition.body.path_conditions.clear();
    definition.body.logical_expressions.clear();

    check_normalized_mir(&program)
        .expect("normalized checks must accept the exact executable logical shape");
}

#[test]
fn normalized_contract_still_checks_source_visible_primitive_initialization() {
    let mut program =
        lower_source_to_mir("fn main() -> i64 { var result: i64 = 7; return result; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let local = definition
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::Local)
        .unwrap()
        .id;
    for block in &mut definition.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(
                instruction,
                MirInstruction::Store(store)
                    if store.destination.base == MirPlaceBase::Storage(local)
                        && store.destination.projections.is_empty()
            )
        });
    }

    let errors = check_normalized_mir(&program)
        .expect_err("normalization authority cannot excuse source-visible storage")
        .to_string();
    assert!(
        errors.contains("loaded without initialization on every incoming path"),
        "{errors}"
    );
}

#[test]
fn both_contracts_accept_an_initialized_ordinary_scalar_spill() {
    let mut program =
        lower_source_to_mir("fn main() -> i64 { var result: i64 = 7; return result; }");
    reclassify_result_local_as_scalar_spill(&mut program);

    verify_mir(&program).expect("proof-rich MIR must check and accept the initialized spill");
    check_normalized_mir(&program)
        .expect("normalized MIR must retain the initialized-spill baseline");
}

#[test]
fn normalized_scalar_spill_exception_has_an_explicit_before_state() {
    let mut program =
        lower_source_to_mir("fn main() -> i64 { var result: i64 = 7; return result; }");
    let spill = reclassify_result_local_as_scalar_spill(&mut program);
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    for block in &mut definition.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(
                instruction,
                MirInstruction::Store(store)
                    if store.destination.base == MirPlaceBase::Storage(spill)
                        && store.destination.projections.is_empty()
            )
        });
    }

    let errors = verify_mir(&program)
        .expect_err("proof-rich MIR must reject an uninitialized ordinary spill")
        .to_string();
    assert!(
        errors.contains("loaded without initialization on every incoming path"),
        "{errors}"
    );
    check_normalized_mir(&program)
        .expect("the current broad normalized spill exception is the baseline narrowed later");
}

fn reclassify_result_local_as_scalar_spill(
    program: &mut crate::mir::MirProgram,
) -> crate::mir::StorageId {
    reclassify_first_local(program, MirStorageKind::ScalarSpill, true)
}

fn reclassify_first_local(
    program: &mut crate::mir::MirProgram,
    kind: MirStorageKind,
    clear_source: bool,
) -> crate::mir::StorageId {
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let storage = definition
        .storage
        .iter_mut()
        .find(|storage| storage.kind == MirStorageKind::Local)
        .expect("fixture must lower one local result");
    storage.kind = kind;
    if clear_source {
        storage.source = None;
    }
    storage.id
}
