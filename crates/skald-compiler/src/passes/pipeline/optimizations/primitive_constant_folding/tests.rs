use crate::{
    identity::CallableId,
    mir::{
        MirAssignment, MirBinaryOperation, MirBody, MirComparisonOperand, MirComparisonPredicate,
        MirFunctionDefinition, MirInstruction, MirPrimitiveCast, MirPrimitiveComparison,
        MirPrimitiveType, MirRvalue, MirRvalueKind, MirTerminator, MirType, MirUnaryOperation,
        MirValue, ValueId,
    },
    passes::{
        resolve_exact_mir_pass_schedule, run_mir_pipeline, run_mir_pipeline_with_occurrences,
        MirPassMeasurement, MirPassOccurrenceOutcome,
    },
    source::Span,
    test_support::lower_source_to_final_mir,
};

use super::{
    FoldCounts, FOLDED_BINARY, FOLDED_CASTS, FOLDED_COMPARISONS, FOLDED_UNARY,
    FOLDS_CROSSING_CARRIERS, FOLDS_CROSSING_CHECKED, FOLDS_CROSSING_LOGICAL, IDENTITY,
    MAXIMUM_DEPENDENCY_DEPTH,
};
use crate::passes::pipeline::optimizations::{
    dead_pure_definition_elimination, whole_world_reachability,
};

fn exact_schedule(
    identities: &[crate::passes::MirPassIdentity],
) -> super::super::super::policy::MirPassSchedule {
    resolve_exact_mir_pass_schedule(identities).unwrap()
}

fn append_assignment(
    definition: &mut MirFunctionDefinition,
    kind: MirRvalueKind,
    ty: MirType,
) -> ValueId {
    let callable = CallableId::Function(definition.function);
    let result = ValueId::new(callable, definition.values.len());
    definition.values.push(MirValue {
        id: result,
        ty,
        span: definition.span,
    });
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::Assign(MirAssignment {
            result,
            rvalue: MirRvalue { kind, ty },
            span: definition.span,
        }));
    result
}

fn append_fold_candidate(
    callable: CallableId,
    values: &mut Vec<MirValue>,
    body: &mut MirBody,
    span: Span,
) -> ValueId {
    let operand = ValueId::new(callable, values.len());
    values.push(MirValue {
        id: operand,
        ty: MirType::I64,
        span,
    });
    body.blocks[0]
        .instructions
        .push(MirInstruction::Assign(MirAssignment {
            result: operand,
            rvalue: MirRvalue {
                kind: MirRvalueKind::ConstantI64(1),
                ty: MirType::I64,
            },
            span,
        }));

    let result = ValueId::new(callable, values.len());
    values.push(MirValue {
        id: result,
        ty: MirType::I64,
        span,
    });
    body.blocks[0]
        .instructions
        .push(MirInstruction::Assign(MirAssignment {
            result,
            rvalue: MirRvalue {
                kind: MirRvalueKind::Unary {
                    operation: MirUnaryOperation::NegateI64,
                    operand,
                },
                ty: MirType::I64,
            },
            span,
        }));
    result
}

fn assignment_mut(definition: &mut MirFunctionDefinition, result: ValueId) -> &mut MirAssignment {
    definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) if assignment.result == result => Some(assignment),
            _ => None,
        })
        .unwrap()
}

fn returned_assignment(definition: &MirFunctionDefinition) -> (&MirAssignment, &MirRvalueKind) {
    let returned = definition
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::Return {
                value: Some(value), ..
            }) => Some(value),
            _ => None,
        })
        .expect("fixture returns one scalar value");
    let assignment = definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) if assignment.result == returned => Some(assignment),
            _ => None,
        })
        .expect("returned value has an assignment definition");
    (assignment, &assignment.rvalue.kind)
}

struct Chain {
    unary: ValueId,
    first_binary: ValueId,
    comparison: ValueId,
    cast: ValueId,
    second_binary: ValueId,
}

fn append_chain(definition: &mut MirFunctionDefinition, return_result: bool) -> Chain {
    let forty = append_assignment(definition, MirRvalueKind::ConstantI64(40), MirType::I64);
    let two = append_assignment(definition, MirRvalueKind::ConstantI64(2), MirType::I64);
    let first_binary = append_assignment(
        definition,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left: forty,
            right: two,
        },
        MirType::I64,
    );
    let unary = append_assignment(
        definition,
        MirRvalueKind::Unary {
            operation: MirUnaryOperation::NegateI64,
            operand: first_binary,
        },
        MirType::I64,
    );
    let comparison = append_assignment(
        definition,
        MirRvalueKind::PrimitiveComparison {
            operation: MirPrimitiveComparison {
                predicate: MirComparisonPredicate::LessThan,
                operand: MirComparisonOperand::Integer(crate::mir::MirIntegerType::I64),
            },
            left: unary,
            right: forty,
        },
        MirType::Bool,
    );
    let cast = append_assignment(
        definition,
        MirRvalueKind::PrimitiveCast {
            operation: MirPrimitiveCast::new(MirPrimitiveType::Bool, MirPrimitiveType::I64),
            operand: comparison,
        },
        MirType::I64,
    );
    let second_binary = append_assignment(
        definition,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::MultiplyI64,
            left: first_binary,
            right: cast,
        },
        MirType::I64,
    );

    if return_result {
        definition.body.blocks[0].terminator = Some(MirTerminator::Return {
            value: Some(second_binary),
            span: definition.span,
        });
    }

    Chain {
        unary,
        first_binary,
        comparison,
        cast,
        second_binary,
    }
}

fn expected_chain(definition: &mut MirFunctionDefinition, chain: &Chain) {
    assignment_mut(definition, chain.first_binary).rvalue.kind = MirRvalueKind::ConstantI64(42);
    assignment_mut(definition, chain.unary).rvalue.kind = MirRvalueKind::ConstantI64(-42);
    assignment_mut(definition, chain.comparison).rvalue.kind = MirRvalueKind::ConstantBool(true);
    assignment_mut(definition, chain.cast).rvalue.kind = MirRvalueKind::ConstantI64(1);
    assignment_mut(definition, chain.second_binary).rvalue.kind = MirRvalueKind::ConstantI64(42);
}

fn expected_measurements(counts: FoldCounts) -> [MirPassMeasurement; 8] {
    [
        MirPassMeasurement::count(FOLDED_UNARY, counts.unary as u64),
        MirPassMeasurement::count(FOLDED_BINARY, counts.binary as u64),
        MirPassMeasurement::count(FOLDED_COMPARISONS, counts.comparisons as u64),
        MirPassMeasurement::count(FOLDED_CASTS, counts.casts as u64),
        MirPassMeasurement::count(FOLDS_CROSSING_CARRIERS, counts.crossing_carriers as u64),
        MirPassMeasurement::count(FOLDS_CROSSING_CHECKED, counts.crossing_checked as u64),
        MirPassMeasurement::count(FOLDS_CROSSING_LOGICAL, counts.crossing_logical as u64),
        MirPassMeasurement::count(
            MAXIMUM_DEPENDENCY_DEPTH,
            counts.maximum_dependency_depth as u64,
        ),
    ]
}

#[test]
fn straight_line_chain_folds_every_supported_assignment_kind_in_place() {
    let mut input = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let entry = input.entry_function;
    let chain = append_chain(input.definitions.get_mut_for_test(entry).unwrap(), true);
    let mut expected = input.clone();
    expected_chain(
        expected.definitions.get_mut_for_test(entry).unwrap(),
        &chain,
    );

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let output = measured.result.as_ref().unwrap().program();
    let record = &measured.occurrences()[0];

    assert_eq!(output, &expected);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(record.processed_callables(), Some(1));
    assert_eq!(record.changed_callables(), Some(1));
    assert_eq!(record.inserted_mir_entities(), Some(0));
    assert_eq!(record.removed_mir_entities(), Some(0));
    assert_eq!(record.verification_executions(), 1);
    assert_eq!(
        record.measurements(),
        expected_measurements(FoldCounts {
            unary: 1,
            binary: 2,
            comparisons: 1,
            casts: 1,
            maximum_dependency_depth: 5,
            ..FoldCounts::default()
        })
    );
}

#[test]
fn candidate_free_occurrence_preserves_the_verified_product_without_reverification() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let expected = input.clone();
    let processed = input.executable_definitions().count() as u64;
    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let record = &measured.occurrences()[0];

    assert_eq!(measured.result.as_ref().unwrap().program(), &expected);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Unchanged);
    assert_eq!(record.processed_callables(), Some(processed));
    assert_eq!(record.changed_callables(), Some(0));
    assert_eq!(record.verification_executions(), 0);
    assert_eq!(
        record.measurements(),
        expected_measurements(FoldCounts::default())
    );
}

#[test]
fn unsupported_checked_floating_load_and_call_structure_remains_exact() {
    let mut input = lower_source_to_final_mir(concat!(
        "fn checked(dividend: i64, divisor: i64, count: u64) -> i64 {\n",
        "  return dividend / divisor + (dividend << count);\n",
        "}\n",
        "fn floating(value: f64) -> f64 { return -value + 1.0; }\n",
        "fn main() -> i64 { return checked(8, 2, 1u) + (i64) floating(2.0); }\n",
    ));
    let entry = input.entry_function;
    let definition = input.definitions.get_mut_for_test(entry).unwrap();
    let result = append_fold_candidate(
        CallableId::Function(entry),
        &mut definition.values,
        &mut definition.body,
        definition.span,
    );
    let mut expected = input.clone();
    assignment_mut(
        expected.definitions.get_mut_for_test(entry).unwrap(),
        result,
    )
    .rvalue
    .kind = MirRvalueKind::ConstantI64(-1);

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    assert_eq!(measured.result.as_ref().unwrap().program(), &expected);
    assert_eq!(
        measured.occurrences()[0].measurements(),
        expected_measurements(FoldCounts {
            unary: 1,
            maximum_dependency_depth: 1,
            ..FoldCounts::default()
        })
    );
}

#[test]
fn repeated_occurrences_are_changed_then_idempotently_unchanged() {
    let mut input = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let entry = input.entry_function;
    append_chain(input.definitions.get_mut_for_test(entry).unwrap(), true);
    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY, IDENTITY]));
    let records = measured.occurrences();

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].occurrence(), 0);
    assert_eq!(records[0].outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(records[0].verification_executions(), 1);
    assert_eq!(records[1].occurrence(), 1);
    assert_eq!(records[1].outcome(), MirPassOccurrenceOutcome::Unchanged);
    assert_eq!(records[1].verification_executions(), 0);
    assert_eq!(
        records[1].measurements(),
        expected_measurements(FoldCounts::default())
    );
}

#[test]
fn one_occurrence_folds_arbitrarily_deep_primitive_chains() {
    const DEPTH: usize = 512;
    let mut input = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let entry = input.entry_function;
    let definition = input.definitions.get_mut_for_test(entry).unwrap();
    let one = append_assignment(definition, MirRvalueKind::ConstantI64(1), MirType::I64);
    let mut result = one;
    for _ in 0..DEPTH {
        result = append_assignment(
            definition,
            MirRvalueKind::Binary {
                operation: MirBinaryOperation::AddI64,
                left: result,
                right: one,
            },
            MirType::I64,
        );
    }
    definition.body.blocks[0].terminator = Some(MirTerminator::Return {
        value: Some(result),
        span: definition.span,
    });

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let definition = measured
        .result
        .as_ref()
        .unwrap()
        .definitions
        .get(entry)
        .unwrap();
    assert_eq!(
        returned_assignment(definition).1,
        &MirRvalueKind::ConstantI64((DEPTH + 1) as i64)
    );
    assert_eq!(
        measured.occurrences()[0].measurements(),
        expected_measurements(FoldCounts {
            binary: DEPTH,
            maximum_dependency_depth: DEPTH,
            ..FoldCounts::default()
        })
    );
}

#[test]
fn propagated_checked_fact_folds_ordinary_consumer_without_rewriting_protocol() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return (8 / 2) + 3; }");
    let entry = input.entry_function;
    let original_checks = input
        .definitions
        .get(entry)
        .unwrap()
        .body
        .blocks
        .iter()
        .filter(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::IntegerDivisorCheck { .. })
            )
        })
        .count();

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let definition = measured
        .result
        .as_ref()
        .unwrap()
        .definitions
        .get(entry)
        .unwrap();
    assert_eq!(
        returned_assignment(definition).1,
        &MirRvalueKind::ConstantI64(7)
    );
    assert_eq!(
        definition
            .body
            .blocks
            .iter()
            .filter(|block| matches!(
                block.terminator,
                Some(MirTerminator::IntegerDivisorCheck { .. })
            ))
            .count(),
        original_checks
    );
    let counts = FoldCounts {
        binary: 1,
        crossing_carriers: 1,
        crossing_checked: 1,
        maximum_dependency_depth: 6,
        ..FoldCounts::default()
    };
    assert_eq!(
        measured.occurrences()[0].measurements(),
        expected_measurements(counts)
    );
}

#[test]
fn propagated_logical_result_folds_ordinary_consumer_but_retains_rhs_region() {
    let input = lower_source_to_final_mir(concat!(
        "fn effect() -> bool { return true; }\n",
        "fn choose() -> bool { return (false && effect()) == false; }\n",
        "fn main() -> i64 { return (i64) choose(); }\n",
    ));
    let choose = input
        .definitions
        .iter()
        .find(|definition| !definition.body.logical_expressions.is_empty())
        .map(|definition| definition.function)
        .unwrap();
    let input_definition = input.definitions.get(choose).unwrap();
    let input_blocks = input_definition.body.blocks.len();
    let comparison = input_definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(
                    assignment.rvalue.kind,
                    MirRvalueKind::PrimitiveComparison { .. }
                ) =>
            {
                Some(assignment.result)
            }
            _ => None,
        })
        .unwrap();

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let definition = measured
        .result
        .as_ref()
        .unwrap()
        .definitions
        .get(choose)
        .unwrap();
    let folded = definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) if assignment.result == comparison => {
                Some(&assignment.rvalue.kind)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(folded, &MirRvalueKind::ConstantBool(true));
    assert_eq!(definition.body.blocks.len(), input_blocks);
    assert_eq!(
        measured.occurrences()[0].measurements(),
        expected_measurements(FoldCounts {
            comparisons: 1,
            crossing_logical: 1,
            maximum_dependency_depth: 2,
            ..FoldCounts::default()
        })
    );
}

#[test]
fn statically_failing_checked_result_remains_a_propagation_barrier() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return (9 / 0) + 1; }");
    let entry = input.entry_function;
    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let definition = measured
        .result
        .as_ref()
        .unwrap()
        .definitions
        .get(entry)
        .unwrap();

    assert!(matches!(
        returned_assignment(definition).1,
        MirRvalueKind::Binary { .. }
    ));
    assert_eq!(
        measured.occurrences()[0].outcome(),
        MirPassOccurrenceOutcome::Unchanged
    );
}

#[test]
fn stale_immutable_plan_aborts_before_publishing_a_partial_program() {
    let mut input = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let entry = input.entry_function;
    let definition = input.definitions.get_mut_for_test(entry).unwrap();
    let result = append_fold_candidate(
        CallableId::Function(entry),
        &mut definition.values,
        &mut definition.body,
        definition.span,
    );
    let plan = super::plan::PrimitiveFoldPlan::prepare(&input).unwrap();
    assignment_mut(input.definitions.get_mut_for_test(entry).unwrap(), result)
        .rvalue
        .kind = MirRvalueKind::ConstantI64(99);

    let error = crate::mir::rewrite::rewrite_program(input, |callable, edit| {
        plan.rewrite_callable(callable, edit)
    })
    .unwrap_err();
    assert!(matches!(
        error,
        crate::mir::rewrite::MirRewriteError::StaleCallableSnapshot {
            callable: CallableId::Function(function),
            subject: "instruction",
        } if function == entry
    ));
}

#[test]
fn default_profile_selects_constant_folding_and_later_cleanup() {
    let mut input = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let entry = input.entry_function;
    append_chain(input.definitions.get_mut_for_test(entry).unwrap(), true);
    let output = run_mir_pipeline(input).unwrap();
    let definition = output.definitions.get(entry).unwrap();
    let Some(MirTerminator::Return {
        value: Some(returned),
        ..
    }) = &definition.body.blocks[0].terminator
    else {
        panic!("fixture must retain its scalar return")
    };
    let returned = definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) if assignment.result == *returned => {
                Some(&assignment.rvalue.kind)
            }
            _ => None,
        })
        .unwrap();

    assert_eq!(returned, &MirRvalueKind::ConstantI64(42));
}

#[test]
fn folding_composes_with_dead_pure_cleanup_and_whole_world_retention() {
    let mut input = lower_source_to_final_mir(concat!(
        "fn unused() -> i64 { return 9; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let entry = input.entry_function;
    append_chain(input.definitions.get_mut_for_test(entry).unwrap(), false);
    let schedule = exact_schedule(&[
        IDENTITY,
        dead_pure_definition_elimination::IDENTITY,
        whole_world_reachability::IDENTITY,
    ]);
    let measured = run_mir_pipeline_with_occurrences(input, &schedule);
    let output = measured.result.as_ref().unwrap().program();

    assert_eq!(output.executable_definitions().count(), 1);
    assert_eq!(
        measured
            .occurrences()
            .iter()
            .map(|record| (record.name(), record.outcome()))
            .collect::<Vec<_>>(),
        [
            (
                "primitive-constant-folding",
                MirPassOccurrenceOutcome::Changed
            ),
            (
                "dead-pure-definition-elimination",
                MirPassOccurrenceOutcome::Changed,
            ),
            (
                "whole-world-reachability",
                MirPassOccurrenceOutcome::Changed
            ),
        ]
    );
}

#[test]
fn pass_processes_every_executable_callable_kind_and_counts_only_changed_callables() {
    let mut input = lower_source_to_final_mir(concat!(
        "class Item {\n",
        "  static seed: i64 = 1;\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Item) { self.value = other.value; }\n",
        "  assign(ref other: Item) { self.value = other.value; }\n",
        "  destroy {}\n",
        "  fn read() -> i64 { return self.value; }\n",
        "  static fn identity(value: i64) -> i64 { return value; }\n",
        "}\n",
        "fn main() -> i64 { var item: Item = Item(2); return Item.identity(item.read()); }\n",
    ));
    let function_ids = input
        .definitions
        .iter()
        .map(|definition| definition.function)
        .collect::<Vec<_>>();
    for function in function_ids {
        let definition = input.definitions.get_mut_for_test(function).unwrap();
        append_fold_candidate(
            CallableId::Function(function),
            &mut definition.values,
            &mut definition.body,
            definition.span,
        );
    }
    let member_ids = input
        .member_definitions
        .iter()
        .map(|definition| definition.callable)
        .collect::<Vec<_>>();
    for callable in member_ids {
        let definition = input.member_definitions.get_mut_for_test(callable).unwrap();
        append_fold_candidate(
            callable,
            &mut definition.values,
            &mut definition.body,
            definition.span,
        );
    }
    if let Some(coordinator) = &mut input.static_lifecycle {
        for definition in coordinator.initializers_mut_for_test() {
            append_fold_candidate(
                CallableId::StaticInitializer(definition.id),
                &mut definition.values,
                &mut definition.body,
                definition.span,
            );
        }
    }
    let executable_count = input.executable_definitions().count() as u64;

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let record = &measured.occurrences()[0];
    assert_eq!(record.processed_callables(), Some(executable_count));
    assert_eq!(record.changed_callables(), Some(executable_count));
    assert_eq!(
        record.measurements(),
        expected_measurements(FoldCounts {
            unary: executable_count as usize,
            maximum_dependency_depth: 1,
            ..FoldCounts::default()
        })
    );
}
