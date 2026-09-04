use crate::{
    identity::CallableId,
    mir::{
        MirBinaryOperation, MirInstruction, MirPlace, MirPlaceBase, MirRvalueKind, MirType,
        StorageId,
    },
    passes::verify_final_mir,
    test_support::lower_source_to_final_mir,
};

use super::*;

fn division_mir() -> crate::mir::MirProgram {
    lower_source_to_final_mir("fn main() -> i64 { return 42 / 2; }")
}

fn raw_counts(program: &crate::mir::MirProgram) -> ScalarSpillProvenanceCounts {
    scalar_spill::analyze_unverified_definition(program.executable_definitions().next().unwrap())
        .unwrap()
}

fn sites<T: Copy + Eq>(counts: &[ScalarSpillCount<T>], key: T) -> u64 {
    counts
        .iter()
        .find(|count| count.key() == key)
        .map_or(0, |count| count.sites())
}

fn make_ordinary_consumers(program: &mut crate::mir::MirProgram) {
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut definition.body.blocks[1].instructions[2] else {
        panic!("expected checked operation assignment");
    };
    let MirRvalueKind::IntegerDivision { dividend, .. } = assignment.rvalue.kind else {
        panic!("expected integer division");
    };
    assignment.rvalue.kind = MirRvalueKind::Binary {
        operation: MirBinaryOperation::AddI64,
        left: dividend,
        right: crate::mir::ValueId::new(CallableId::Function(program.entry_function), 1),
    };
}

#[test]
fn verified_checked_spills_are_classified_without_mutating_mir() {
    let original = division_mir();
    let verified = verify_final_mir(original.clone()).unwrap();
    let first = analyze_scalar_spill_provenance(&verified);
    let second = analyze_scalar_spill_provenance(&verified);
    let counts = first.counts();

    assert_eq!(first, second);
    assert_eq!(verified.program(), &original);
    assert_eq!(counts.inspected(), 3);
    assert_eq!(counts.interesting(), 2);
    assert_eq!(counts.proven(), 0);
    assert_eq!(counts.blocked(), 2);
    assert_eq!(counts.non_candidates(), 1);
    assert_eq!(
        counts.inspected(),
        counts.interesting() + counts.non_candidates()
    );
    assert_eq!(counts.interesting(), counts.proven() + counts.blocked());
    assert_eq!(counts.affected_callables(), 1);
    assert_eq!(
        first.callables()[0].callable(),
        CallableId::Function(original.entry_function)
    );
    assert_eq!(sites(counts.depths(), ScalarSpillDepth::Direct), 2);
    assert_eq!(
        sites(
            counts.primary_blockers(),
            ScalarSpillBlocker::ProtectedMetadataOrUse,
        ),
        2
    );
    assert_eq!(
        sites(counts.barriers(), ScalarSpillBlocker::MissingDominance),
        0,
        "entry-block stores must dominate checked successor loads"
    );
    assert_eq!(
        sites(
            counts.consumers(),
            ScalarSpillConsumer::CheckedIntegerProtocol
        ),
        2
    );
    assert_eq!(
        sites(counts.unlocks(), ScalarSpillUnlock::CheckedFolding),
        0
    );
}

#[test]
fn ordinary_consumers_produce_direct_proven_candidates_and_primitive_unlocks() {
    let mut program = division_mir();
    make_ordinary_consumers(&mut program);
    let counts = raw_counts(&program);
    assert_eq!(counts.proven(), 1);
    assert_eq!(counts.blocked(), 0);
    assert_eq!(sites(counts.depths(), ScalarSpillDepth::Direct), 1);
    assert_eq!(
        sites(counts.consumers(), ScalarSpillConsumer::TotalPrimitive),
        1
    );
    assert_eq!(
        sites(counts.unlocks(), ScalarSpillUnlock::PrimitiveFolding),
        1
    );
    assert_eq!(counts.removable_values_upper_bound(), 1);
    assert_eq!(counts.removable_instructions_upper_bound(), 1);
}

#[test]
fn a_single_virtual_constant_uses_the_existing_checked_evaluator() {
    let mut program = division_mir();
    let callable = CallableId::Function(program.entry_function);
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let MirInstruction::Assign(operation) = &mut definition.body.blocks[1].instructions[2] else {
        panic!("expected checked operation");
    };
    let MirRvalueKind::IntegerDivision { divisor, .. } = &mut operation.rvalue.kind else {
        panic!("expected integer division");
    };
    *divisor = crate::mir::ValueId::new(callable, 1);

    assert_eq!(
        sites(
            raw_counts(&program).unlocks(),
            ScalarSpillUnlock::CheckedFolding,
        ),
        1
    );
}

#[test]
fn loop_epochs_do_not_look_like_ambiguous_dynamic_writes() {
    let verified = verify_final_mir(lower_source_to_final_mir(
        "fn main() -> i64 { var n: i64 = 0; while (n < 2) { n = 42 / 2; } return n; }",
    ))
    .unwrap();
    let observation = analyze_scalar_spill_provenance(&verified);
    let counts = observation.counts();
    assert!(counts.interesting() > 0);
    assert_eq!(
        sites(counts.barriers(), ScalarSpillBlocker::AmbiguousWrites),
        0
    );
    assert_eq!(
        sites(counts.barriers(), ScalarSpillBlocker::MissingDominance),
        0
    );
}

#[test]
fn chained_spills_distinguish_one_hop_and_transitive_provenance() {
    let mut one_hop = division_mir();
    make_ordinary_consumers(&mut one_hop);
    let definition = one_hop
        .definitions
        .get_mut_for_test(one_hop.entry_function)
        .unwrap();
    let MirInstruction::Assign(operation) = &mut definition.body.blocks[1].instructions[2] else {
        panic!("expected ordinary operation");
    };
    let MirRvalueKind::Binary { right, .. } = &mut operation.rvalue.kind else {
        panic!("expected ordinary binary operation");
    };
    *right = crate::mir::ValueId::new(CallableId::Function(one_hop.entry_function), 3);
    let source = match &definition.body.blocks[1].instructions[0] {
        MirInstruction::Assign(assignment) => assignment.result,
        _ => panic!("expected first operand load"),
    };
    let MirInstruction::Store(store) = &mut definition.body.blocks[0].instructions[5] else {
        panic!("expected second operand store");
    };
    store.value = source;
    let counts = raw_counts(&one_hop);
    assert_eq!(sites(counts.depths(), ScalarSpillDepth::OneHop), 1);
    assert!(sites(counts.barriers(), ScalarSpillBlocker::MissingDominance) > 0);

    let mut transitive = one_hop;
    let definition = transitive
        .definitions
        .get_mut_for_test(transitive.entry_function)
        .unwrap();
    let result_reload = match &definition.body.blocks[3].instructions[0] {
        MirInstruction::Assign(assignment) => assignment.result,
        _ => panic!("expected result reload"),
    };
    let MirInstruction::Store(second_store) = &mut definition.body.blocks[0].instructions[5] else {
        panic!("expected second operand store");
    };
    second_store.value = result_reload;
    let first_load = match &definition.body.blocks[1].instructions[0] {
        MirInstruction::Assign(assignment) => assignment.result,
        _ => panic!("expected first operand load"),
    };
    let MirInstruction::Store(result_store) = &mut definition.body.blocks[1].instructions[3] else {
        panic!("expected result store");
    };
    result_store.value = first_load;
    let counts = raw_counts(&transitive);
    assert_eq!(sites(counts.depths(), ScalarSpillDepth::Transitive), 1);
}

#[test]
fn write_dominance_place_type_and_identity_rejections_are_explicit() {
    let mut ambiguous = division_mir();
    make_ordinary_consumers(&mut ambiguous);
    let definition = ambiguous
        .definitions
        .get_mut_for_test(ambiguous.entry_function)
        .unwrap();
    let duplicate = definition.body.blocks[0].instructions[2].clone();
    definition.body.blocks[0].instructions.insert(3, duplicate);
    assert!(
        sites(
            raw_counts(&ambiguous).barriers(),
            ScalarSpillBlocker::AmbiguousWrites
        ) > 0
    );

    let mut nondominating = division_mir();
    make_ordinary_consumers(&mut nondominating);
    let definition = nondominating
        .definitions
        .get_mut_for_test(nondominating.entry_function)
        .unwrap();
    let store = definition.body.blocks[0].instructions.remove(2);
    definition.body.blocks[2].instructions.push(store);
    assert!(
        sites(
            raw_counts(&nondominating).barriers(),
            ScalarSpillBlocker::MissingDominance
        ) > 0
    );

    let mut noncanonical = division_mir();
    make_ordinary_consumers(&mut noncanonical);
    let definition = noncanonical
        .definitions
        .get_mut_for_test(noncanonical.entry_function)
        .unwrap();
    let storage = definition.storage[0].id;
    let MirInstruction::Assign(load) = &mut definition.body.blocks[1].instructions[0] else {
        panic!("expected load");
    };
    load.rvalue.kind = MirRvalueKind::Load(MirPlace {
        base: MirPlaceBase::AliasParameter(storage),
        projections: vec![],
    });
    let counts = raw_counts(&noncanonical);
    assert!(sites(counts.barriers(), ScalarSpillBlocker::NoncanonicalPlace) > 0);
    assert!(sites(counts.barriers(), ScalarSpillBlocker::AliasExposure) > 0);

    let mut wrong_type = division_mir();
    make_ordinary_consumers(&mut wrong_type);
    wrong_type
        .definitions
        .get_mut_for_test(wrong_type.entry_function)
        .unwrap()
        .storage[0]
        .ty = MirType::U64;
    assert!(
        sites(
            raw_counts(&wrong_type).barriers(),
            ScalarSpillBlocker::UnsupportedTypeOrOperation
        ) > 0
    );

    let mut malformed = division_mir();
    make_ordinary_consumers(&mut malformed);
    let definition = malformed
        .definitions
        .get_mut_for_test(malformed.entry_function)
        .unwrap();
    definition.storage[0].id = StorageId::new(CallableId::Function(malformed.entry_function), 99);
    assert!(
        sites(
            raw_counts(&malformed).barriers(),
            ScalarSpillBlocker::MalformedIdentity
        ) > 0
    );
}

#[test]
fn unsupported_and_protected_use_roles_have_closed_classification() {
    use crate::mir::rewrite::{MirCallValueUse, MirScalarValueUse, MirValueUseRole};

    assert_eq!(
        scalar_spill::consumer(MirValueUseRole::OrdinaryScalarRvalue(
            MirScalarValueUse::UnaryOperand,
        )),
        ScalarSpillConsumer::TotalPrimitive
    );
    assert_eq!(
        scalar_spill::consumer(MirValueUseRole::OrdinaryPrimitiveCast),
        ScalarSpillConsumer::PrimitiveCast
    );
    assert_eq!(
        scalar_spill::consumer(MirValueUseRole::OrdinaryCall(MirCallValueUse::Argument(0))),
        ScalarSpillConsumer::Call
    );
    assert_eq!(
        scalar_spill::consumer(MirValueUseRole::OrdinaryStore),
        ScalarSpillConsumer::Store
    );
    assert_eq!(
        scalar_spill::consumer(MirValueUseRole::OrdinaryReturn),
        ScalarSpillConsumer::Return
    );
    assert_eq!(
        scalar_spill::consumer(MirValueUseRole::OrdinaryBranch),
        ScalarSpillConsumer::ConditionalBranch
    );

    let mut barriers = std::collections::BTreeSet::new();
    scalar_spill::add_use_barrier(&mut barriers, MirValueUseRole::ProofMetadata);
    scalar_spill::add_use_barrier(&mut barriers, MirValueUseRole::OwnershipOrLifecycle);
    assert_eq!(
        barriers.into_iter().collect::<Vec<_>>(),
        [
            ScalarSpillBlocker::ProtectedMetadataOrUse,
            ScalarSpillBlocker::LifecycleParticipation,
        ]
    );
}

#[test]
fn programs_without_scalar_spills_produce_an_empty_observation() {
    let verified =
        verify_final_mir(lower_source_to_final_mir("fn main() -> i64 { return 42; }")).unwrap();
    assert_eq!(
        analyze_scalar_spill_provenance(&verified),
        ScalarSpillProvenanceObservation::default()
    );
}
