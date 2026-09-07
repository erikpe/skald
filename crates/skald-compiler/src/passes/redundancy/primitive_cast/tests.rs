use crate::{
    identity::{CallableId, FunctionId},
    mir::{
        BlockId, MirAssignment, MirBasicBlock, MirInstruction, MirPrimitiveCast,
        MirPrimitiveCastKind, MirPrimitiveType, MirRvalue, MirRvalueKind, MirTerminator, MirType,
        MirValue, ValueId,
    },
    passes::verify_final_mir,
    test_support::lower_source_to_final_mir,
};
use std::collections::BTreeSet;

use super::*;

const TYPES: [MirPrimitiveType; 5] = [
    MirPrimitiveType::I64,
    MirPrimitiveType::U64,
    MirPrimitiveType::U8,
    MirPrimitiveType::F64,
    MirPrimitiveType::Bool,
];

fn sites<T: Copy + Eq>(counts: &[PrimitiveCastCount<T>], key: T) -> u64 {
    counts
        .iter()
        .find(|count| count.key() == key)
        .map_or(0, |count| count.sites())
}

fn analyze(source: &str) -> (crate::mir::MirProgram, PrimitiveCastObservation) {
    let program = lower_source_to_final_mir(source);
    let verified = verify_final_mir(program.clone()).unwrap();
    let observation = analyze_redundant_primitive_casts(&verified);
    assert_eq!(verified.program(), &program);
    (program, observation)
}

#[test]
fn complete_primitive_pair_matrix_has_an_explicit_disposition() {
    let mut ordinary = 0;
    let mut checked = 0;
    for source in TYPES {
        for target in TYPES {
            let operation = MirPrimitiveCast::new(source, target);
            let classified = disposition(operation);
            if operation.may_terminate() {
                checked += 1;
                assert_eq!(
                    classified,
                    PrimitiveCastDisposition::CheckedFloatingToInteger
                );
            } else {
                ordinary += 1;
                assert_ne!(classified, PrimitiveCastDisposition::Unsupported);
            }
        }
    }
    assert_eq!((ordinary, checked), (22, 3));
}

#[test]
fn composition_table_proves_only_complete_domain_equivalences() {
    let cast = MirPrimitiveCast::new;
    assert_eq!(
        compose(
            cast(MirPrimitiveType::I64, MirPrimitiveType::U64),
            cast(MirPrimitiveType::U64, MirPrimitiveType::I64),
        ),
        Composition::OriginalInput
    );
    assert_eq!(
        compose(
            cast(MirPrimitiveType::U8, MirPrimitiveType::I64),
            cast(MirPrimitiveType::I64, MirPrimitiveType::U64),
        ),
        Composition::DirectCast
    );
    assert_eq!(
        compose(
            cast(MirPrimitiveType::I64, MirPrimitiveType::U8),
            cast(MirPrimitiveType::U8, MirPrimitiveType::I64),
        ),
        Composition::RequiredIntegerSequence
    );
    assert_eq!(
        compose(
            cast(MirPrimitiveType::Bool, MirPrimitiveType::U8),
            cast(MirPrimitiveType::U8, MirPrimitiveType::Bool),
        ),
        Composition::OriginalInput
    );
    assert_eq!(
        compose(
            cast(MirPrimitiveType::U64, MirPrimitiveType::Bool),
            cast(MirPrimitiveType::Bool, MirPrimitiveType::U64),
        ),
        Composition::MissingValueDomain
    );
}

#[test]
fn floating_raw_bit_and_checked_compositions_are_explicit_barriers() {
    let numeric = MirPrimitiveCast::new;
    assert_eq!(
        compose(
            numeric(MirPrimitiveType::I64, MirPrimitiveType::F64),
            numeric(MirPrimitiveType::F64, MirPrimitiveType::Bool),
        ),
        Composition::FloatingPayload
    );
    assert_eq!(
        compose(
            MirPrimitiveCast::bit_reinterpretation(MirPrimitiveType::U64, MirPrimitiveType::F64,),
            MirPrimitiveCast::bit_reinterpretation(MirPrimitiveType::F64, MirPrimitiveType::U64,),
        ),
        Composition::FloatingPayload,
        "even a raw-bit round trip stays conservative about NaN payload semantics"
    );
    assert_eq!(
        compose(
            numeric(MirPrimitiveType::I64, MirPrimitiveType::F64),
            numeric(MirPrimitiveType::F64, MirPrimitiveType::I64),
        ),
        Composition::CheckedFailure
    );
}

#[test]
fn identities_and_safe_adjacent_chains_are_proven_and_deterministic() {
    let (program, first) = analyze(
        "fn identity(value: i64) -> i64 { return (i64) value; }\n\
         fn round_trip(value: i64) -> i64 { return (i64) (u64) value; }\n\
         fn main() -> i64 { return identity(round_trip(7)); }",
    );
    let verified = verify_final_mir(program).unwrap();
    let second = analyze_redundant_primitive_casts(&verified);
    assert_eq!(first, second);
    let counts = first.counts();
    assert!(first.examples().iter().any(|example| {
        example.classification() == RedundancySiteClassification::Proven
            && example.reasons().is_empty()
            && example.value().is_some()
    }));
    assert_eq!(
        counts.inspected(),
        counts.interesting() + counts.non_candidates()
    );
    assert_eq!(counts.interesting(), counts.proven() + counts.blocked());
    assert!(counts.proven() >= 2);
    assert!(sites(counts.dispositions(), PrimitiveCastDisposition::Identity) >= 1);
    assert!(
        sites(counts.dispositions(), PrimitiveCastDisposition::Identity) >= 2,
        "both explicit identities and identity-result chains use guarded forwarding"
    );
    assert!(sites(counts.consumers(), PrimitiveCastConsumer::Return) >= 1);
    assert!(counts.removable_instructions_upper_bound() >= 2);
    assert!(counts.eliminated_cast_steps_upper_bound() >= 2);
    assert!(counts.maximum_chain_depth() >= 2);
}

#[test]
fn arbitrary_depth_integer_candidates_report_exact_step_and_support_bounds() {
    let program = lower_source_to_final_mir(
        "fn chain(value: i64) -> u64 {\n\
             return (u64) (u8) (i64) (u8) (u64) value;\n\
         }\n\
         fn main() -> i64 { return 0; }",
    );
    let counts =
        analyze_unverified_definition(program.definitions.get(FunctionId::new(0)).unwrap().into())
            .unwrap();

    assert_eq!(counts.inspected(), 5);
    assert_eq!(counts.interesting(), 4);
    assert_eq!(counts.proven(), 4);
    assert_eq!(counts.blocked(), 0);
    assert_eq!(counts.non_candidates(), 1);
    assert_eq!(counts.supporting_values(), 6);
    assert_eq!(counts.supporting_instructions(), 5);
    assert_eq!(counts.removable_values_upper_bound(), 8);
    assert_eq!(counts.removable_instructions_upper_bound(), 8);
    assert_eq!(counts.eliminated_cast_steps_upper_bound(), 8);
    assert_eq!(counts.maximum_chain_depth(), 5);
    assert!(!counts.saturated());
}

#[test]
fn chain_measurement_merge_saturates_step_totals_and_takes_maximum_depth() {
    let mut total = Accumulator::default();
    total.counts.eliminated_cast_steps_upper_bound = u64::MAX;
    total.counts.maximum_chain_depth = 4;
    let mut next = Accumulator::default();
    next.counts.eliminated_cast_steps_upper_bound = 1;
    next.counts.maximum_chain_depth = 19;

    total.merge(&next);

    assert_eq!(total.counts.eliminated_cast_steps_upper_bound(), u64::MAX);
    assert_eq!(total.counts.maximum_chain_depth(), 19);
    assert!(total.counts.saturated());
}

#[test]
fn integer_chain_boundaries_have_stable_measurement_blockers() {
    let cases = [
        (
            IntegerCastChainBoundary::ControlFlow,
            Some(PrimitiveCastBlocker::ControlFlowBoundary),
        ),
        (
            IntegerCastChainBoundary::InvalidPredecessor,
            Some(PrimitiveCastBlocker::UnsupportedTypeOrOperation),
        ),
        (
            IntegerCastChainBoundary::NonPrecedingDefinition,
            Some(PrimitiveCastBlocker::NonPrecedingProvenance),
        ),
        (
            IntegerCastChainBoundary::RepeatedIdentity,
            Some(PrimitiveCastBlocker::RepeatedProvenance),
        ),
        (IntegerCastChainBoundary::UnsupportedCastFamily, None),
    ];

    for (boundary, expected) in cases {
        let mut barriers = BTreeSet::new();
        add_chain_boundary_barrier(&mut barriers, Some(boundary));
        assert_eq!(barriers.into_iter().next(), expected);
    }
}

#[test]
fn narrowing_widening_is_required_while_boolean_canonicalization_needs_domain_facts() {
    let (_, observation) = analyze(
        "fn narrow(value: i64) -> i64 { return (i64) (u8) value; }\n\
         fn boolean(value: u64) -> u64 { return (u64) (bool) value; }\n\
         fn main() -> i64 { return narrow((i64) boolean(256u)); }",
    );
    let counts = observation.counts();
    assert_eq!(
        sites(
            counts.barriers(),
            PrimitiveCastBlocker::MissingValueDomainFact
        ),
        1
    );
    assert!(
        sites(
            counts.dispositions(),
            PrimitiveCastDisposition::RequiredIntegerWidening
        ) >= 1
    );
    assert!(
        sites(
            counts.dispositions(),
            PrimitiveCastDisposition::BooleanCanonicalization
        ) >= 1
    );
}

#[test]
fn checked_diamonds_are_excluded_from_ordinary_candidates() {
    let (_, observation) = analyze(
        "fn checked(value: f64) -> i64 { return (i64) value; }\n\
         fn main() -> i64 { return checked(1.5); }",
    );
    let counts = observation.counts();
    assert_eq!(counts.excluded_checked_conversions(), 1);
    assert_eq!(counts.excluded_checked_range_checks(), 1);
    assert_eq!(counts.inspected(), 0);
}

#[test]
fn checked_protocol_use_is_a_protected_identity_replacement_barrier() {
    let mut program = lower_source_to_final_mir(
        "fn checked(value: f64) -> i64 { return (i64) value; }\n\
         fn main() -> i64 { return checked(1.5); }",
    );
    let definition = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let (block_index, instruction_index, source) = definition
        .body
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .enumerate()
                .find_map(|(instruction_index, item)| {
                    let MirInstruction::Assign(assignment) = item else {
                        return None;
                    };
                    let MirRvalueKind::CheckedF64ToInteger { operand, .. } = assignment.rvalue.kind
                    else {
                        return None;
                    };
                    Some((block_index, instruction_index, operand))
                })
        })
        .unwrap();
    let callable = CallableId::Function(definition.function);
    let identity = ValueId::new(callable, definition.values.len());
    let span = definition.span;
    definition.values.push(MirValue {
        id: identity,
        ty: MirType::F64,
        span,
    });
    definition.body.blocks[block_index].instructions.insert(
        instruction_index,
        MirInstruction::Assign(MirAssignment {
            result: identity,
            rvalue: MirRvalue {
                kind: MirRvalueKind::PrimitiveCast {
                    operation: MirPrimitiveCast::new(MirPrimitiveType::F64, MirPrimitiveType::F64),
                    operand: source,
                },
                ty: MirType::F64,
            },
            span,
        }),
    );
    let MirInstruction::Assign(checked) =
        &mut definition.body.blocks[block_index].instructions[instruction_index + 1]
    else {
        unreachable!()
    };
    let MirRvalueKind::CheckedF64ToInteger { operand, .. } = &mut checked.rvalue.kind else {
        unreachable!()
    };
    *operand = identity;

    let counts =
        analyze_unverified_definition(program.executable_definitions().next().unwrap()).unwrap();
    assert_eq!(counts.inspected(), 1);
    assert_eq!(counts.blocked(), 1);
    assert_eq!(
        sites(counts.consumers(), PrimitiveCastConsumer::CheckedProtocol),
        1
    );
    assert_eq!(
        sites(
            counts.barriers(),
            PrimitiveCastBlocker::ProtectedMetadataOrUse
        ),
        1
    );
}

#[test]
fn identity_replacement_does_not_cross_a_control_flow_boundary() {
    let mut program = lower_source_to_final_mir(
        "fn identity(value: i64) -> i64 { return (i64) value; }\n\
         fn main() -> i64 { return identity(7); }",
    );
    let definition = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let old_terminator = definition.body.blocks[0].terminator.take().unwrap();
    let target = BlockId::new(CallableId::Function(definition.function), 1);
    let span = definition.span;
    definition.body.blocks[0].terminator = Some(MirTerminator::Goto { target, span });
    definition.body.blocks.push(MirBasicBlock {
        id: target,
        instructions: Vec::new(),
        terminator: Some(old_terminator),
        span,
    });

    let counts =
        analyze_unverified_definition(program.executable_definitions().next().unwrap()).unwrap();
    assert_eq!(counts.blocked(), 1);
    assert_eq!(
        sites(
            counts.primary_blockers(),
            PrimitiveCastBlocker::ControlFlowBoundary
        ),
        1
    );
}

#[test]
fn nonadjacent_chains_and_multiple_intermediate_uses_remain_candidates() {
    let source = "fn chain(value: u8) -> u64 { return (u64) (i64) value; }\n\
                  fn main() -> i64 { return (i64) chain(7u8); }";

    let mut nonadjacent = lower_source_to_final_mir(source);
    let definition = nonadjacent
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let second = definition.body.blocks[0]
        .instructions
        .iter()
        .position(|item| {
            matches!(
                item,
                MirInstruction::Assign(MirAssignment {
                    rvalue: MirRvalue {
                        kind: MirRvalueKind::PrimitiveCast { .. },
                        ..
                    },
                    ..
                })
            )
        })
        .unwrap()
        + 1;
    let extra = ValueId::new(
        CallableId::Function(definition.function),
        definition.values.len(),
    );
    let span = definition.span;
    definition.values.push(MirValue {
        id: extra,
        ty: MirType::I64,
        span,
    });
    definition.body.blocks[0].instructions.insert(
        second,
        MirInstruction::Assign(MirAssignment {
            result: extra,
            rvalue: MirRvalue {
                kind: MirRvalueKind::ConstantI64(0),
                ty: MirType::I64,
            },
            span,
        }),
    );
    let counts =
        analyze_unverified_definition(nonadjacent.executable_definitions().next().unwrap())
            .unwrap();
    assert_eq!(
        sites(
            counts.barriers(),
            PrimitiveCastBlocker::NonAdjacentProvenance
        ),
        0
    );
    assert!(counts.proven() >= 1);

    let mut multiple = lower_source_to_final_mir(source);
    let definition = multiple
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let chains = analyze_integer_cast_chains((&*definition).into()).unwrap();
    let first = chains.entries()[0].site().clone();
    let extra = ValueId::new(
        CallableId::Function(definition.function),
        definition.values.len(),
    );
    let span = definition.span;
    definition.values.push(MirValue {
        id: extra,
        ty: MirType::I64,
        span,
    });
    definition.body.blocks[first.block()]
        .instructions
        .push(MirInstruction::Assign(MirAssignment {
            result: extra,
            rvalue: MirRvalue {
                kind: MirRvalueKind::PrimitiveCast {
                    operation: MirPrimitiveCast::new(MirPrimitiveType::I64, MirPrimitiveType::I64),
                    operand: first.result(),
                },
                ty: MirType::I64,
            },
            span,
        }));
    let counts =
        analyze_unverified_definition(multiple.executable_definitions().next().unwrap()).unwrap();
    assert_eq!(
        sites(counts.barriers(), PrimitiveCastBlocker::MultipleUses),
        0
    );
    assert!(counts.proven() >= 1);
}

#[test]
fn malformed_value_declarations_are_reported_as_blockers() {
    let mut program = lower_source_to_final_mir(
        "fn identity(value: i64) -> i64 { return (i64) value; }\n\
         fn main() -> i64 { return identity(7); }",
    );
    let definition = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let result = analyze_integer_cast_chains((&*definition).into())
        .unwrap()
        .entries()[0]
        .site()
        .result();
    definition.values[result.index()].id = ValueId::new(result.callable(), result.index() + 99);
    let counts =
        analyze_unverified_definition(program.executable_definitions().next().unwrap()).unwrap();
    assert_eq!(
        sites(
            counts.primary_blockers(),
            PrimitiveCastBlocker::MalformedIdentity
        ),
        1
    );
}

#[test]
fn every_ordinary_cast_shape_is_recorded_exactly() {
    let (_, observation) = analyze(
        "fn cast(value: u8) -> u64 { return (u64) value; }\n\
         fn main() -> i64 { return (i64) cast(7u8); }",
    );
    let expected = PrimitiveCastShape::new(
        MirPrimitiveCastKind::IntegerBits,
        MirPrimitiveType::U8,
        MirPrimitiveType::U64,
    );
    assert_eq!(sites(observation.counts().shapes(), expected), 1);
    assert_eq!(observation.callables().len(), 2);
}
