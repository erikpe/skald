use crate::{
    identity::{ArrayTypeId, CallableId, ClassId, FunctionId, FunctionTypeId, InitializerId},
    mir::{
        rewrite::tests::representative_function, BlockId, MirArgument, MirArrayFailure,
        MirArrayInstruction, MirArrayOwnership, MirAssignment, MirBasicBlock, MirBinaryOperation,
        MirCall, MirCallTarget, MirComparisonOperand, MirComparisonPredicate,
        MirFunctionDefinition, MirIndirectCallTarget, MirInstruction, MirIntegerDivisionKind,
        MirIntegerDivisionOperation, MirIntegerType, MirOptionalInitialize, MirOptionalSource,
        MirPlace, MirPrimitiveCast, MirPrimitiveComparison, MirPrimitiveType, MirRvalue,
        MirRvalueKind, MirSharedInitialize, MirStorageLive, MirStore, MirTerminator, MirType,
        MirUnaryOperation, MirValue, StorageId, ValueId,
    },
    test_support::lower_source_to_final_mir,
};

use super::*;

fn value(callable: CallableId, index: usize) -> ValueId {
    ValueId::new(callable, index)
}

fn append_assignment(
    definition: &mut MirFunctionDefinition,
    kind: MirRvalueKind,
    ty: MirType,
) -> (ValueId, usize) {
    let callable = definition.callable();
    let result = value(callable, definition.values.len());
    definition.values.push(MirValue {
        id: result,
        ty,
        span: definition.span,
    });
    let instruction = definition.body.blocks[0].instructions.len();
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::Assign(MirAssignment {
            result,
            rvalue: MirRvalue { kind, ty },
            span: definition.span,
        }));
    (result, instruction)
}

fn ordinary_use_fixture() -> (MirFunctionDefinition, ValueId, Vec<MirValueUseSite>) {
    let program = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let mut definition = program
        .definitions
        .get(program.entry_function)
        .unwrap()
        .clone();
    let selected = value(definition.callable(), 0);
    let block = 0;
    let mut expected = Vec::new();

    let (_, instruction) = append_assignment(
        &mut definition,
        MirRvalueKind::Unary {
            operation: MirUnaryOperation::NegateI64,
            operand: selected,
        },
        MirType::I64,
    );
    expected.push(MirValueUseSite {
        site: MirLocalIdentitySite::Instruction { block, instruction },
        role: MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::UnaryOperand),
    });

    let (_, instruction) = append_assignment(
        &mut definition,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left: selected,
            right: selected,
        },
        MirType::I64,
    );
    expected.extend([
        MirValueUseSite {
            site: MirLocalIdentitySite::Instruction { block, instruction },
            role: MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::BinaryLeft),
        },
        MirValueUseSite {
            site: MirLocalIdentitySite::Instruction { block, instruction },
            role: MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::BinaryRight),
        },
    ]);

    let (_, instruction) = append_assignment(
        &mut definition,
        MirRvalueKind::PrimitiveComparison {
            operation: MirPrimitiveComparison {
                predicate: MirComparisonPredicate::Equal,
                operand: MirComparisonOperand::Integer(MirIntegerType::I64),
            },
            left: selected,
            right: selected,
        },
        MirType::Bool,
    );
    expected.extend([
        MirValueUseSite {
            site: MirLocalIdentitySite::Instruction { block, instruction },
            role: MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::ComparisonLeft),
        },
        MirValueUseSite {
            site: MirLocalIdentitySite::Instruction { block, instruction },
            role: MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::ComparisonRight),
        },
    ]);

    let (_, instruction) = append_assignment(
        &mut definition,
        MirRvalueKind::PrimitiveCast {
            operation: MirPrimitiveCast::new(MirPrimitiveType::I64, MirPrimitiveType::I64),
            operand: selected,
        },
        MirType::I64,
    );
    expected.push(MirValueUseSite {
        site: MirLocalIdentitySite::Instruction { block, instruction },
        role: MirValueUseRole::OrdinaryPrimitiveCast,
    });

    let instruction = definition.body.blocks[0].instructions.len();
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::Store(MirStore {
            destination: MirPlace::base(StorageId::new(selected.callable(), 0)),
            value: selected,
            authorization: None,
            final_authorization: None,
            span: definition.span,
        }));
    expected.push(MirValueUseSite {
        site: MirLocalIdentitySite::Instruction { block, instruction },
        role: MirValueUseRole::OrdinaryStore,
    });

    let instruction = definition.body.blocks[0].instructions.len();
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::Call(MirCall {
            target: MirCallTarget::Indirect(MirIndirectCallTarget {
                callee: selected,
                function_type: FunctionTypeId::new(0),
            }),
            receiver: None,
            arguments: vec![MirArgument::Value(selected)],
            result: None,
            shared_result: None,
            destination: None,
            span: definition.span,
        }));
    expected.extend([
        MirValueUseSite {
            site: MirLocalIdentitySite::Instruction { block, instruction },
            role: MirValueUseRole::OrdinaryCall(MirCallValueUse::Target),
        },
        MirValueUseSite {
            site: MirLocalIdentitySite::Instruction { block, instruction },
            role: MirValueUseRole::OrdinaryCall(MirCallValueUse::Argument(0)),
        },
    ]);

    expected.push(MirValueUseSite {
        site: MirLocalIdentitySite::Terminator(block),
        role: MirValueUseRole::OrdinaryReturn,
    });
    (definition, selected, expected)
}

fn edit_for(definition: &MirFunctionDefinition) -> MirCallableEdit {
    MirCallableEdit::from_dense_parts(
        definition.callable(),
        definition.storage.clone(),
        definition.values.clone(),
        definition.body.clone(),
    )
    .unwrap()
}

#[test]
fn every_semantic_role_has_an_explicit_conservative_forwarding_decision() {
    for role in [
        MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::UnaryOperand),
        MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::BinaryLeft),
        MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::BinaryRight),
        MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::ComparisonLeft),
        MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::ComparisonRight),
        MirValueUseRole::OrdinaryPrimitiveCast,
        MirValueUseRole::OrdinaryStore,
        MirValueUseRole::OrdinaryCall(MirCallValueUse::Target),
        MirValueUseRole::OrdinaryCall(MirCallValueUse::Receiver),
        MirValueUseRole::OrdinaryCall(MirCallValueUse::Argument(3)),
        MirValueUseRole::OrdinaryReturn,
        MirValueUseRole::OrdinaryBranch,
    ] {
        assert!(role.is_forwarding_safe(), "{role:?}");
    }
    for role in [
        MirValueUseRole::CheckedProtocol,
        MirValueUseRole::ProofMetadata,
        MirValueUseRole::OwnershipOrLifecycle,
        MirValueUseRole::InputOutput,
        MirValueUseRole::Unknown,
    ] {
        assert!(!role.is_forwarding_safe(), "{role:?}");
    }
}

#[test]
fn ordinary_uses_are_enumerated_in_structural_and_operand_order() {
    let (definition, selected, expected) = ordinary_use_fixture();
    let sites = value_use_sites_for_definition((&definition).into(), selected).unwrap();

    assert_eq!(sites.callable(), definition.callable());
    assert_eq!(sites.value(), selected);
    assert_eq!(sites.uses(), expected);
    assert!(sites.all_uses_follow_definition_in_same_block());
    assert!(sites.is_forwarding_safe());
}

#[test]
fn dense_and_sparse_queries_return_the_same_snapshot() {
    let (definition, selected, _) = ordinary_use_fixture();
    let edit = edit_for(&definition);

    assert_eq!(
        edit.value_use_sites(selected).unwrap(),
        value_use_sites_for_definition((&definition).into(), selected).unwrap()
    );
}

#[test]
fn position_snapshots_must_be_recomputed_after_a_rewrite() {
    let (definition, selected, _) = ordinary_use_fixture();
    let mut edit = edit_for(&definition);
    let block = BlockId::new(definition.callable(), 0);
    let before = edit.value_use_sites(selected).unwrap();

    edit.rewrite_block_instructions(block, |instructions| {
        let mut rewritten = Vec::with_capacity(instructions.len() + 1);
        rewritten.push(MirInstruction::StorageLive(MirStorageLive {
            storage: StorageId::new(selected.callable(), 0),
            span: definition.span,
        }));
        rewritten.extend_from_slice(instructions);
        rewritten
    })
    .unwrap();
    let after = edit.value_use_sites(selected).unwrap();

    assert_ne!(before, after);
    assert_eq!(
        before.definition(),
        MirLocalIdentitySite::Instruction {
            block: 0,
            instruction: 0,
        }
    );
    assert_eq!(
        after.definition(),
        MirLocalIdentitySite::Instruction {
            block: 0,
            instruction: 1,
        }
    );
}

#[test]
fn proof_io_and_call_roles_remain_distinct_and_block_forwarding() {
    let mut definition = representative_function();
    let selected = value(definition.callable(), 1);
    let span = definition.span;
    definition.body.blocks[0].instructions.insert(
        0,
        MirInstruction::Assign(MirAssignment {
            result: selected,
            rvalue: MirRvalue {
                kind: MirRvalueKind::ConstantI64(0),
                ty: MirType::I64,
            },
            span,
        }),
    );
    let MirInstruction::Call(call) = &mut definition.body.blocks[0].instructions[3] else {
        unreachable!()
    };
    call.arguments.push(MirArgument::Value(selected));

    let sites = value_use_sites_for_definition((&definition).into(), selected).unwrap();
    assert_eq!(
        sites.uses(),
        [
            MirValueUseSite {
                site: MirLocalIdentitySite::Instruction {
                    block: 0,
                    instruction: 3,
                },
                role: MirValueUseRole::OrdinaryCall(MirCallValueUse::Target),
            },
            MirValueUseSite {
                site: MirLocalIdentitySite::Instruction {
                    block: 0,
                    instruction: 3,
                },
                role: MirValueUseRole::OrdinaryCall(MirCallValueUse::Argument(1)),
            },
            MirValueUseSite {
                site: MirLocalIdentitySite::Instruction {
                    block: 0,
                    instruction: 9,
                },
                role: MirValueUseRole::InputOutput,
            },
            MirValueUseSite {
                site: MirLocalIdentitySite::LogicalExpression(0),
                role: MirValueUseRole::ProofMetadata,
            },
        ]
    );
    assert!(!sites.all_uses_follow_definition_in_same_block());
    assert!(!sites.is_forwarding_safe());
}

#[test]
fn metadata_only_uses_are_retained_as_proof_barriers() {
    let definition = representative_function();
    let selected = value(definition.callable(), 2);

    let sites = value_use_sites_for_definition((&definition).into(), selected).unwrap();
    assert_eq!(
        sites.uses(),
        [MirValueUseSite {
            site: MirLocalIdentitySite::LogicalExpression(0),
            role: MirValueUseRole::ProofMetadata,
        }]
    );
    assert!(!sites.is_forwarding_safe());
}

#[test]
fn ordinary_branch_conditions_are_classified_by_the_terminator_variant() {
    let (mut definition, selected, _) = ordinary_use_fixture();
    let target = definition.body.blocks[0].id;
    definition.body.blocks[0].terminator = Some(MirTerminator::Branch {
        condition: selected,
        true_target: target,
        false_target: target,
        span: definition.span,
    });

    let sites = value_use_sites_for_definition((&definition).into(), selected).unwrap();
    assert_eq!(
        sites.uses().last().copied(),
        Some(MirValueUseSite {
            site: MirLocalIdentitySite::Terminator(0),
            role: MirValueUseRole::OrdinaryBranch,
        })
    );
    assert!(sites.is_forwarding_safe());
}

#[test]
fn checked_and_lifecycle_value_uses_are_forwarding_barriers() {
    let (mut definition, selected, _) = ordinary_use_fixture();
    let (_, checked_instruction) = append_assignment(
        &mut definition,
        MirRvalueKind::IntegerDivision {
            operation: MirIntegerDivisionOperation {
                kind: MirIntegerDivisionKind::Quotient,
                operand: MirIntegerType::I64,
            },
            dividend: selected,
            divisor: selected,
        },
        MirType::I64,
    );
    let lifecycle_instruction = definition.body.blocks[0].instructions.len();
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::Initialize(crate::mir::MirInitialize {
            destination: MirPlace::base(StorageId::new(selected.callable(), 0)),
            target: InitializerId::new(ClassId::new(0), 0),
            arguments: vec![MirArgument::Value(selected)],
            span: definition.span,
        }));
    let shared_instruction = definition.body.blocks[0].instructions.len();
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::SharedInitialize(MirSharedInitialize {
            allocation: StorageId::new(selected.callable(), 0),
            target: InitializerId::new(ClassId::new(0), 0),
            arguments: vec![MirArgument::Value(selected)],
            span: definition.span,
        }));
    let optional_instruction = definition.body.blocks[0].instructions.len();
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::OptionalInitialize(MirOptionalInitialize {
            destination: MirPlace::base(StorageId::new(selected.callable(), 0)),
            source: MirOptionalSource::Present(selected),
            span: definition.span,
        }));
    let array_instruction = definition.body.blocks[0].instructions.len();
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::Array(MirArrayInstruction::Allocate {
            backing: StorageId::new(selected.callable(), 0),
            array: ArrayTypeId::new(0),
            length: selected,
            ownership: MirArrayOwnership::Inline,
            failure: MirArrayFailure::AllocationSize,
            span: definition.span,
        }));

    let sites = value_use_sites_for_definition((&definition).into(), selected).unwrap();
    assert!(sites.uses().contains(&MirValueUseSite {
        site: MirLocalIdentitySite::Instruction {
            block: 0,
            instruction: checked_instruction,
        },
        role: MirValueUseRole::CheckedProtocol,
    }));
    assert!(sites.uses().contains(&MirValueUseSite {
        site: MirLocalIdentitySite::Instruction {
            block: 0,
            instruction: lifecycle_instruction,
        },
        role: MirValueUseRole::OwnershipOrLifecycle,
    }));
    for instruction in [shared_instruction, optional_instruction, array_instruction] {
        assert!(sites.uses().contains(&MirValueUseSite {
            site: MirLocalIdentitySite::Instruction {
                block: 0,
                instruction,
            },
            role: MirValueUseRole::OwnershipOrLifecycle,
        }));
    }
    assert!(!sites.is_forwarding_safe());
}

#[test]
fn cross_block_and_use_before_definition_are_not_forwarding_local() {
    let (mut cross_block, selected, _) = ordinary_use_fixture();
    let callable = cross_block.callable();
    let second = BlockId::new(callable, 1);
    cross_block.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: second,
        span: cross_block.span,
    });
    cross_block.body.blocks.push(MirBasicBlock {
        id: second,
        instructions: vec![],
        terminator: Some(MirTerminator::Return {
            value: Some(selected),
            span: cross_block.span,
        }),
        span: cross_block.span,
    });
    assert!(
        !value_use_sites_for_definition((&cross_block).into(), selected)
            .unwrap()
            .all_uses_follow_definition_in_same_block()
    );

    let (mut before_definition, selected, _) = ordinary_use_fixture();
    let extra = value(before_definition.callable(), before_definition.values.len());
    before_definition.values.push(MirValue {
        id: extra,
        ty: MirType::I64,
        span: before_definition.span,
    });
    before_definition.body.blocks[0].instructions.insert(
        0,
        MirInstruction::Assign(MirAssignment {
            result: extra,
            rvalue: MirRvalue {
                kind: MirRvalueKind::Unary {
                    operation: MirUnaryOperation::NegateI64,
                    operand: selected,
                },
                ty: MirType::I64,
            },
            span: before_definition.span,
        }),
    );
    assert!(
        !value_use_sites_for_definition((&before_definition).into(), selected)
            .unwrap()
            .all_uses_follow_definition_in_same_block()
    );
}

#[test]
fn selected_value_failures_distinguish_foreign_unknown_deleted_and_missing() {
    let (definition, selected, _) = ordinary_use_fixture();
    let callable = definition.callable();
    let foreign = value(CallableId::Function(FunctionId::new(99)), selected.index());
    let unknown = value(callable, 999);

    assert_eq!(
        value_use_sites_for_definition((&definition).into(), foreign).unwrap_err(),
        MirRewriteError::ForeignIdentity {
            expected: callable,
            identity: MirLocalIdentity::Value(foreign),
        }
    );
    assert_eq!(
        value_use_sites_for_definition((&definition).into(), unknown).unwrap_err(),
        MirRewriteError::UnknownIdentity {
            identity: MirLocalIdentity::Value(unknown),
        }
    );

    let mut edit = edit_for(&definition);
    edit.remove_value(selected).unwrap();
    assert_eq!(
        edit.value_use_sites(selected).unwrap_err(),
        MirRewriteError::DeletedIdentity {
            identity: MirLocalIdentity::Value(selected),
        }
    );

    let missing = representative_function();
    let missing_value = value(missing.callable(), 1);
    assert_eq!(
        value_use_sites_for_definition((&missing).into(), missing_value).unwrap_err(),
        MirRewriteError::MissingValueDefinition {
            value: missing_value,
        }
    );
}

#[test]
fn malformed_duplicate_definitions_and_references_remain_structured() {
    let (mut duplicate, selected, _) = ordinary_use_fixture();
    let duplicate_instruction = duplicate.body.blocks[0].instructions.len();
    duplicate.body.blocks[0]
        .instructions
        .push(MirInstruction::Assign(MirAssignment {
            result: selected,
            rvalue: MirRvalue {
                kind: MirRvalueKind::ConstantI64(1),
                ty: MirType::I64,
            },
            span: duplicate.span,
        }));
    assert!(matches!(
        value_use_sites_for_definition((&duplicate).into(), selected),
        Err(MirRewriteError::DuplicateValueDefinition {
            value,
            duplicate: MirLocalIdentitySite::Instruction {
                block: 0,
                instruction,
            },
            ..
        }) if value == selected && instruction == duplicate_instruction
    ));

    let (mut invalid_reference, selected, _) = ordinary_use_fixture();
    let foreign = value(CallableId::Function(FunctionId::new(99)), 0);
    invalid_reference.body.blocks[0].terminator = Some(MirTerminator::Return {
        value: Some(foreign),
        span: invalid_reference.span,
    });
    assert!(matches!(
        value_use_sites_for_definition((&invalid_reference).into(), selected),
        Err(MirRewriteError::InvalidReference {
            identity: MirLocalIdentity::Value(value),
            failure: super::super::MirReferenceFailure::Foreign,
            ..
        }) if value == foreign
    ));
}
