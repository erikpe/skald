use crate::{
    mir::{
        rewrite::MirLocalIdentitySite, verify_mir, BlockId, MirInstruction, MirPlace, MirPlaceBase,
        MirRvalueKind, MirStorageKind, MirTerminator,
    },
    test_support::lower_source_to_mir,
};

use super::{
    classify_instruction, classify_local_identity_site, classify_proof_record,
    classify_rvalue_kind, classify_storage_kind, MirIdentitySiteRole, MirProofDisposition,
    MirProofRecordKind,
};
use crate::mir::verify::check_normalized_mir;

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
        classify_rvalue_kind(&assignment.rvalue.kind),
        MirProofDisposition::PermanentSemantic
    );
    assert_eq!(
        classify_instruction(&MirInstruction::Assign(assignment.clone())),
        MirProofDisposition::PermanentSemantic
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
        errors.contains("retains path-condition storage"),
        "{errors}"
    );
    assert!(
        errors.contains("retains a path-condition rvalue"),
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
