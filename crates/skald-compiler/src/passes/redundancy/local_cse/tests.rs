use crate::{
    identity::CallableId,
    mir::{
        test_fixtures::{assign, block, store},
        BlockId, MirBinaryOperation, MirComparisonOperand, MirComparisonPredicate, MirInstruction,
        MirIntegerDivisionKind, MirIntegerDivisionOperation, MirIntegerType, MirPlace,
        MirPrimitiveCast, MirPrimitiveComparison, MirRvalueKind, MirStorage, MirStorageKind,
        MirTerminator, MirType, MirUnaryOperation, MirValue, StorageId, ValueId,
    },
    passes::{verify_final_mir, RedundancySiteClassification},
    test_support::lower_source_to_final_mir,
};

use super::*;

fn sites<T: Copy + Eq>(counts: &[LocalCseCount<T>], key: T) -> u64 {
    counts
        .iter()
        .find(|count| count.key() == key)
        .map_or(0, |count| count.sites())
}

fn empty_program() -> crate::mir::MirProgram {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    definition.storage.clear();
    definition.values.clear();
    definition.body.blocks[0].instructions.clear();
    definition.body.path_conditions.clear();
    definition.body.logical_expressions.clear();
    program
}

fn append(
    definition: &mut crate::mir::MirFunctionDefinition,
    kind: MirRvalueKind,
    ty: MirType,
) -> ValueId {
    let result = ValueId::new(
        CallableId::Function(definition.function),
        definition.values.len(),
    );
    definition.values.push(MirValue {
        id: result,
        ty,
        span: definition.span,
    });
    definition.body.blocks[0]
        .instructions
        .push(assign(result, kind, ty, definition.span));
    result
}

fn exact_repeat_program() -> crate::mir::MirProgram {
    let mut program = empty_program();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let left = append(definition, MirRvalueKind::ConstantI64(7), MirType::I64);
    let right = append(definition, MirRvalueKind::ConstantI64(9), MirType::I64);
    let add = |left, right| MirRvalueKind::Binary {
        operation: MirBinaryOperation::AddI64,
        left,
        right,
    };
    let _first = append(definition, add(left, right), MirType::I64);
    let used_repeat = append(definition, add(left, right), MirType::I64);
    append(definition, add(left, right), MirType::I64);
    let result = append(definition, add(used_repeat, used_repeat), MirType::I64);
    definition.body.blocks[0].terminator = Some(MirTerminator::Return {
        value: Some(result),
        span: definition.span,
    });
    program
}

#[test]
fn exact_repeats_count_replaceable_and_dead_results_without_mutating_mir() {
    let program = exact_repeat_program();
    let verified = verify_final_mir(program.clone()).unwrap();
    let first = analyze_local_primitive_common_subexpressions(&verified);
    let second = analyze_local_primitive_common_subexpressions(&verified);
    let counts = first.counts();

    assert_eq!(first, second);
    assert_eq!(verified.program(), &program);
    assert_eq!(first.examples().len(), 2);
    assert!(first.examples().iter().all(|example| {
        example.classification() == RedundancySiteClassification::Proven
            && example.reasons().is_empty()
            && example.value().is_some()
    }));
    assert_eq!(counts.inspected(), 4);
    assert_eq!(counts.interesting(), 2);
    assert_eq!(counts.proven(), 2);
    assert_eq!(counts.blocked(), 0);
    assert_eq!(counts.non_candidates(), 2);
    assert_eq!(counts.maximum_repetitions_per_key(), 2);
    assert_eq!(counts.replaceable_uses(), 2);
    assert_eq!(counts.removable_values_upper_bound(), 2);
    assert_eq!(counts.removable_instructions_upper_bound(), 2);
    assert_eq!(sites(counts.outcomes(), LocalCseOutcome::Replaceable), 1);
    assert_eq!(sites(counts.outcomes(), LocalCseOutcome::DeadResult), 1);
    assert_eq!(sites(counts.consumers(), LocalCseConsumer::Dead), 1);
    assert_eq!(
        sites(
            counts.operation_families(),
            LocalCseOperationFamily::IntegerBinary
        ),
        4
    );
}

#[test]
fn loop_cfg_blocks_do_not_share_expression_facts() {
    let program = lower_source_to_final_mir(
        "fn main() -> i64 { var n: i64 = 0; while (n < 3) { n = n + 1; } return n; }",
    );
    let verified = verify_final_mir(program.clone()).unwrap();
    let first = analyze_local_primitive_common_subexpressions(&verified);
    let second = analyze_local_primitive_common_subexpressions(&verified);
    assert_eq!(first, second);
    assert_eq!(verified.program(), &program);
    assert_eq!(
        first.counts().inspected(),
        first.counts().interesting() + first.counts().non_candidates()
    );
}

#[test]
fn ordered_operands_operations_and_result_types_are_exact_near_misses() {
    let mut program = empty_program();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let left = append(definition, MirRvalueKind::ConstantI64(7), MirType::I64);
    let right = append(definition, MirRvalueKind::ConstantI64(9), MirType::I64);
    append(
        definition,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left,
            right,
        },
        MirType::I64,
    );
    append(
        definition,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left: right,
            right: left,
        },
        MirType::I64,
    );
    append(
        definition,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::SubtractI64,
            left,
            right,
        },
        MirType::I64,
    );
    append(
        definition,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left,
            right,
        },
        MirType::U64,
    );

    let counts =
        analyze_unverified_definition(program.executable_definitions().next().unwrap()).unwrap();
    assert_eq!(counts.inspected(), 4);
    assert_eq!(counts.interesting(), 0);
    assert_eq!(counts.non_candidates(), 4);
}

#[test]
fn unary_and_comparison_families_use_exact_keys() {
    let mut program = empty_program();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let integer = append(definition, MirRvalueKind::ConstantI64(7), MirType::I64);
    let boolean = append(definition, MirRvalueKind::ConstantBool(true), MirType::Bool);
    for _ in 0..2 {
        append(
            definition,
            MirRvalueKind::Unary {
                operation: MirUnaryOperation::NegateI64,
                operand: integer,
            },
            MirType::I64,
        );
        append(
            definition,
            MirRvalueKind::PrimitiveComparison {
                operation: MirPrimitiveComparison {
                    predicate: MirComparisonPredicate::Equal,
                    operand: MirComparisonOperand::Bool,
                },
                left: boolean,
                right: boolean,
            },
            MirType::Bool,
        );
    }
    let counts =
        analyze_unverified_definition(program.executable_definitions().next().unwrap()).unwrap();
    assert_eq!(counts.interesting(), 2);
    assert_eq!(
        sites(
            counts.operation_families(),
            LocalCseOperationFamily::IntegerUnary
        ),
        2
    );
    assert_eq!(
        sites(
            counts.operation_families(),
            LocalCseOperationFamily::BooleanComparison
        ),
        2
    );
}

#[test]
fn block_boundaries_reset_expression_facts() {
    let mut program = empty_program();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let callable = CallableId::Function(definition.function);
    let left = append(definition, MirRvalueKind::ConstantI64(7), MirType::I64);
    let right = append(definition, MirRvalueKind::ConstantI64(9), MirType::I64);
    let first = append(
        definition,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left,
            right,
        },
        MirType::I64,
    );
    let second = ValueId::new(callable, definition.values.len());
    definition.values.push(MirValue {
        id: second,
        ty: MirType::I64,
        span: definition.span,
    });
    let entry = definition.body.entry;
    let next = BlockId::new(callable, 1);
    definition.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: next,
        span: definition.span,
    });
    definition.body.blocks.push(block(
        next,
        vec![assign(
            second,
            MirRvalueKind::Binary {
                operation: MirBinaryOperation::AddI64,
                left,
                right,
            },
            MirType::I64,
            definition.span,
        )],
        Some(MirTerminator::Return {
            value: Some(second),
            span: definition.span,
        }),
        definition.span,
    ));
    definition.body.entry = entry;

    let counts =
        analyze_unverified_definition(program.executable_definitions().next().unwrap()).unwrap();
    assert_eq!(counts.inspected(), 2);
    assert_eq!(counts.interesting(), 0);
    assert_eq!(counts.non_candidates(), 2);
    assert_ne!(first, second);
}

#[test]
fn protected_consumers_block_replacement_but_multiple_ordinary_uses_do_not() {
    let mut program = exact_repeat_program();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let repeated = match &definition.body.blocks[0].instructions[3] {
        MirInstruction::Assign(assignment) => assignment.result,
        _ => unreachable!(),
    };
    let divisor = match &definition.body.blocks[0].instructions[0] {
        MirInstruction::Assign(assignment) => assignment.result,
        _ => unreachable!(),
    };
    append(
        definition,
        MirRvalueKind::IntegerDivision {
            operation: MirIntegerDivisionOperation {
                kind: MirIntegerDivisionKind::Quotient,
                operand: MirIntegerType::I64,
            },
            dividend: repeated,
            divisor,
        },
        MirType::I64,
    );
    let counts =
        analyze_unverified_definition(program.executable_definitions().next().unwrap()).unwrap();
    assert!(
        sites(
            counts.primary_blockers(),
            LocalCseBlocker::ProtectedMetadataOrUse
        ) >= 1
    );
    assert!(sites(counts.consumers(), LocalCseConsumer::CheckedProtocol) >= 1);
}

#[test]
fn exclusions_are_explicit_and_never_enter_the_key_universe() {
    let value = ValueId::new(CallableId::Function(crate::identity::FunctionId::new(0)), 0);
    assert_eq!(
        excluded_rvalue(&MirRvalueKind::ConstantI64(1)),
        LocalCseExcludedFamily::Constant
    );
    assert_eq!(
        excluded_rvalue(&MirRvalueKind::PrimitiveCast {
            operation: MirPrimitiveCast::new(
                crate::mir::MirPrimitiveType::I64,
                crate::mir::MirPrimitiveType::U64,
            ),
            operand: value,
        }),
        LocalCseExcludedFamily::Cast
    );
    assert_eq!(
        excluded_rvalue(&MirRvalueKind::Unary {
            operation: MirUnaryOperation::NegateF64,
            operand: value,
        }),
        LocalCseExcludedFamily::FloatingOperation
    );
    assert_eq!(
        excluded_rvalue(&MirRvalueKind::IntegerDivision {
            operation: MirIntegerDivisionOperation {
                kind: MirIntegerDivisionKind::Quotient,
                operand: MirIntegerType::I64,
            },
            dividend: value,
            divisor: value,
        }),
        LocalCseExcludedFamily::CheckedProtocol
    );
}

#[test]
fn scalar_spill_constant_equivalence_is_an_overlap_not_a_direct_candidate() {
    let mut program = empty_program();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let callable = CallableId::Function(definition.function);
    let span = definition.span;
    let first_storage = StorageId::new(callable, 0);
    let second_storage = StorageId::new(callable, 1);
    definition.storage = [first_storage, second_storage]
        .into_iter()
        .map(|id| MirStorage {
            id,
            source: None,
            name: format!("spill-{}", id.index()),
            kind: MirStorageKind::ScalarSpill,
            ty: MirType::I64,
            span,
        })
        .collect();
    let constant_one = append(definition, MirRvalueKind::ConstantI64(7), MirType::I64);
    definition.body.blocks[0].instructions.push(store(
        MirPlace::base(first_storage),
        constant_one,
        span,
    ));
    let constant_two = append(definition, MirRvalueKind::ConstantI64(7), MirType::I64);
    definition.body.blocks[0].instructions.push(store(
        MirPlace::base(second_storage),
        constant_two,
        span,
    ));
    let first_load = append(
        definition,
        MirRvalueKind::Load(MirPlace::base(first_storage)),
        MirType::I64,
    );
    let second_load = append(
        definition,
        MirRvalueKind::Load(MirPlace::base(second_storage)),
        MirType::I64,
    );
    let other = append(definition, MirRvalueKind::ConstantI64(9), MirType::I64);
    let operation = |left| MirRvalueKind::Binary {
        operation: MirBinaryOperation::AddI64,
        left,
        right: other,
    };
    append(definition, operation(first_load), MirType::I64);
    let result = append(definition, operation(second_load), MirType::I64);
    definition.body.blocks[0].terminator = Some(MirTerminator::Return {
        value: Some(result),
        span,
    });

    let counts =
        analyze_unverified_definition(program.executable_definitions().next().unwrap()).unwrap();
    assert_eq!(counts.interesting(), 0);
    assert_eq!(counts.scalar_spill_unlocks(), 1);
}

#[test]
fn malformed_repeated_result_is_a_deterministic_primary_blocker() {
    let mut program = exact_repeat_program();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let repeated = match &definition.body.blocks[0].instructions[3] {
        MirInstruction::Assign(assignment) => assignment.result,
        _ => unreachable!(),
    };
    definition.values[repeated.index()].id =
        ValueId::new(repeated.callable(), repeated.index() + 99);
    let counts =
        analyze_unverified_definition(program.executable_definitions().next().unwrap()).unwrap();
    assert!(
        sites(
            counts.primary_blockers(),
            LocalCseBlocker::MalformedIdentity
        ) >= 1
    );
}

#[test]
fn source_observation_has_a_distinct_role_and_blocker() {
    assert_eq!(
        consumer(MirValueUseRole::InputOutput),
        LocalCseConsumer::InputOutput
    );
    assert_eq!(
        use_blocker(MirValueUseRole::InputOutput),
        Some(LocalCseBlocker::SourceObservation)
    );
    assert_eq!(
        use_blocker(MirValueUseRole::ProofMetadata),
        Some(LocalCseBlocker::ProtectedMetadataOrUse)
    );
}
