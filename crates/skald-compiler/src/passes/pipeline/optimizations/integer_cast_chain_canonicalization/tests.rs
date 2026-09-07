use crate::{
    identity::{CallableId, FunctionId},
    mir::{
        dump_mir,
        rewrite::{rewrite_program, MirRewriteError},
        MirAssignment, MirInstruction, MirIoInstruction, MirIoOperation, MirPrimitiveCast,
        MirPrimitiveType, MirRvalue, MirRvalueKind, MirTerminator, MirType, MirValue, ValueId,
    },
    passes::{
        resolve_exact_mir_pass_schedule, run_mir_pipeline_with_occurrences, MirPassMeasurement,
        MirPassOccurrenceOutcome,
    },
    test_support::lower_source_to_final_mir,
};

use super::{
    plan::IntegerCastCanonicalizationPlan, ELIMINATED_STEPS, FORWARDED_ENDPOINTS, FORWARDED_USES,
    IDENTITY, MAXIMUM_DEPTH, PROTECTED_REJECTIONS, REMOVED_ASSIGNMENTS, REMOVED_VALUES,
    RETARGETED_ENDPOINTS,
};

fn source_type(ty: MirPrimitiveType) -> &'static str {
    match ty {
        MirPrimitiveType::I64 => "i64",
        MirPrimitiveType::U64 => "u64",
        MirPrimitiveType::U8 => "u8",
        MirPrimitiveType::F64 => "f64",
        MirPrimitiveType::Bool => "bool",
    }
}

fn definition_with_root(ty: MirPrimitiveType) -> (crate::mir::MirProgram, ValueId, FunctionId) {
    let source = format!(
        "fn chain(value: {0}) -> {0} {{ return value; }}\n\
         fn main() -> i64 {{ return 0; }}",
        source_type(ty)
    );
    let program = lower_source_to_final_mir(&source);
    let function = FunctionId::new(0);
    let definition = program.definitions.get(function).unwrap();
    let root = match definition.body.blocks[0].terminator {
        Some(MirTerminator::Return {
            value: Some(value), ..
        }) => value,
        _ => panic!("fixture must return its parameter value"),
    };
    (program, root, function)
}

fn append_cast(
    definition: &mut crate::mir::MirFunctionDefinition,
    operand: ValueId,
    source: MirPrimitiveType,
    target: MirPrimitiveType,
) -> ValueId {
    let result = ValueId::new(
        CallableId::Function(definition.function),
        definition.values.len(),
    );
    definition.values.push(MirValue {
        id: result,
        ty: target.value_type(),
        span: definition.span,
    });
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::Assign(MirAssignment {
            result,
            rvalue: MirRvalue {
                kind: MirRvalueKind::PrimitiveCast {
                    operation: MirPrimitiveCast::new(source, target),
                    operand,
                },
                ty: target.value_type(),
            },
            span: definition.span,
        }));
    result
}

fn set_return(definition: &mut crate::mir::MirFunctionDefinition, value: ValueId) {
    definition.body.blocks[0].terminator = Some(MirTerminator::Return {
        value: Some(value),
        span: definition.span,
    });
}

fn assignment(definition: &crate::mir::MirFunctionDefinition, result: ValueId) -> &MirAssignment {
    definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) if assignment.result == result => Some(assignment),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing assignment for {result}"))
}

fn measurements(values: [u64; 8]) -> [MirPassMeasurement; 8] {
    [
        MirPassMeasurement::count(RETARGETED_ENDPOINTS, values[0]),
        MirPassMeasurement::count(FORWARDED_ENDPOINTS, values[1]),
        MirPassMeasurement::count(FORWARDED_USES, values[2]),
        MirPassMeasurement::count(REMOVED_ASSIGNMENTS, values[3]),
        MirPassMeasurement::count(REMOVED_VALUES, values[4]),
        MirPassMeasurement::count(ELIMINATED_STEPS, values[5]),
        MirPassMeasurement::count(PROTECTED_REJECTIONS, values[6]),
        MirPassMeasurement::count(MAXIMUM_DEPTH, values[7]),
    ]
}

fn exact_schedule(repetitions: usize) -> crate::passes::MirPassSchedule {
    resolve_exact_mir_pass_schedule(&vec![IDENTITY; repetitions]).unwrap()
}

#[test]
fn direct_recipe_retargets_the_endpoint_without_inserting_or_deleting_mir() {
    let (mut input, root, function) = definition_with_root(MirPrimitiveType::U8);
    let definition = input.definitions.get_mut_for_test(function).unwrap();
    let intermediate = append_cast(
        definition,
        root,
        MirPrimitiveType::U8,
        MirPrimitiveType::I64,
    );
    let endpoint = append_cast(
        definition,
        intermediate,
        MirPrimitiveType::I64,
        MirPrimitiveType::U64,
    );
    let original_span = assignment(definition, endpoint).span;
    let original_values = definition.values.len();
    let original_instructions = definition.body.blocks[0].instructions.len();

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(1));
    let output = measured
        .result
        .as_ref()
        .unwrap()
        .program()
        .definitions
        .get(function)
        .unwrap();
    let endpoint_assignment = assignment(output, endpoint);
    assert_eq!(
        endpoint_assignment.rvalue.kind,
        MirRvalueKind::PrimitiveCast {
            operation: MirPrimitiveCast::new(MirPrimitiveType::U8, MirPrimitiveType::U64),
            operand: root,
        }
    );
    assert_eq!(endpoint_assignment.rvalue.ty, MirType::U64);
    assert_eq!(endpoint_assignment.span, original_span);
    assert_eq!(output.values.len(), original_values);
    assert_eq!(
        output.body.blocks[0].instructions.len(),
        original_instructions
    );
    assert_eq!(
        assignment(output, intermediate).rvalue.kind,
        MirRvalueKind::PrimitiveCast {
            operation: MirPrimitiveCast::new(MirPrimitiveType::U8, MirPrimitiveType::I64),
            operand: root,
        }
    );

    let record = &measured.occurrences()[0];
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(record.changed_callables(), Some(1));
    assert_eq!(record.verification_executions(), 1);
    assert_eq!(
        record.measurements(),
        measurements([1, 0, 0, 0, 0, 1, 0, 2])
    );
}

#[test]
fn two_cast_recipe_reuses_the_first_existing_narrowing_value() {
    let (mut input, root, function) = definition_with_root(MirPrimitiveType::I64);
    let definition = input.definitions.get_mut_for_test(function).unwrap();
    let preserved = append_cast(
        definition,
        root,
        MirPrimitiveType::I64,
        MirPrimitiveType::U64,
    );
    let narrowing = append_cast(
        definition,
        preserved,
        MirPrimitiveType::U64,
        MirPrimitiveType::U8,
    );
    let widened = append_cast(
        definition,
        narrowing,
        MirPrimitiveType::U8,
        MirPrimitiveType::I64,
    );
    let endpoint = append_cast(
        definition,
        widened,
        MirPrimitiveType::I64,
        MirPrimitiveType::U64,
    );
    let original_values = definition.values.len();
    let original_instructions = definition.body.blocks[0].instructions.len();

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(1));
    let output = measured
        .result
        .as_ref()
        .unwrap()
        .program()
        .definitions
        .get(function)
        .unwrap();
    assert_eq!(
        assignment(output, narrowing).rvalue.kind,
        MirRvalueKind::PrimitiveCast {
            operation: MirPrimitiveCast::new(MirPrimitiveType::I64, MirPrimitiveType::U8),
            operand: root,
        }
    );
    assert_eq!(
        assignment(output, endpoint).rvalue.kind,
        MirRvalueKind::PrimitiveCast {
            operation: MirPrimitiveCast::new(MirPrimitiveType::U8, MirPrimitiveType::U64),
            operand: narrowing,
        }
    );
    assert_eq!(output.values.len(), original_values);
    assert_eq!(
        output.body.blocks[0].instructions.len(),
        original_instructions
    );
    assert_eq!(
        measured.occurrences()[0].measurements(),
        measurements([3, 0, 0, 0, 0, 4, 0, 4])
    );
}

#[test]
fn overlapping_identity_endpoints_batch_forwarding_and_remove_only_endpoints() {
    let (mut input, root, function) = definition_with_root(MirPrimitiveType::I64);
    let definition = input.definitions.get_mut_for_test(function).unwrap();
    let first = append_cast(
        definition,
        root,
        MirPrimitiveType::I64,
        MirPrimitiveType::I64,
    );
    let endpoint = append_cast(
        definition,
        first,
        MirPrimitiveType::I64,
        MirPrimitiveType::I64,
    );
    set_return(definition, endpoint);
    let original_values = definition.values.len();

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(2));
    let output = measured
        .result
        .as_ref()
        .unwrap()
        .program()
        .definitions
        .get(function)
        .unwrap();
    assert_eq!(output.values.len(), original_values - 2);
    assert!(output.body.blocks[0]
        .instructions
        .iter()
        .all(|instruction| !matches!(instruction, MirInstruction::Assign(assignment) if assignment.result == first || assignment.result == endpoint)));
    assert!(matches!(
        output.body.blocks[0].terminator,
        Some(MirTerminator::Return {
            value: Some(value), ..
        }) if value == root
    ));
    let records = measured.occurrences();
    assert_eq!(records[0].outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(
        records[0].measurements(),
        measurements([0, 2, 1, 2, 2, 3, 0, 2])
    );
    assert_eq!(records[1].outcome(), MirPassOccurrenceOutcome::Unchanged);
    assert_eq!(records[1].verification_executions(), 0);
    assert_eq!(records[1].measurements(), measurements([0; 8]));
}

#[test]
fn input_output_use_protects_identity_forwarding_and_reuses_the_verified_seal() {
    let (mut input, root, function) = definition_with_root(MirPrimitiveType::U8);
    let definition = input.definitions.get_mut_for_test(function).unwrap();
    let endpoint = append_cast(definition, root, MirPrimitiveType::U8, MirPrimitiveType::U8);
    let result = ValueId::new(
        CallableId::Function(definition.function),
        definition.values.len(),
    );
    definition.values.push(MirValue {
        id: result,
        ty: MirType::I64,
        span: definition.span,
    });
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::Io(MirIoInstruction {
            result,
            operation: MirIoOperation::StandardHandle { stream: endpoint },
            span: definition.span,
        }));
    let expected = input.clone();

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(1));
    assert!(measured.result.is_ok(), "{:?}", measured.result);
    let record = &measured.occurrences()[0];
    assert_eq!(measured.result.as_ref().unwrap().program(), &expected);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Unchanged);
    assert_eq!(record.changed_callables(), Some(0));
    assert_eq!(record.verification_executions(), 0);
    assert_eq!(
        record.measurements(),
        measurements([0, 0, 0, 0, 0, 0, 1, 0])
    );
}

#[test]
fn stale_instruction_and_type_snapshots_fail_atomically() {
    for mutate_type in [false, true] {
        let (mut input, root, function) = definition_with_root(MirPrimitiveType::U8);
        let definition = input.definitions.get_mut_for_test(function).unwrap();
        let first = append_cast(
            definition,
            root,
            MirPrimitiveType::U8,
            MirPrimitiveType::I64,
        );
        let endpoint = append_cast(
            definition,
            first,
            MirPrimitiveType::I64,
            MirPrimitiveType::U64,
        );
        let plan = IntegerCastCanonicalizationPlan::prepare(&input).unwrap();

        let definition = input.definitions.get_mut_for_test(function).unwrap();
        if mutate_type {
            definition.values[endpoint.index()].ty = MirType::I64;
        } else {
            let span = assignment(definition, endpoint).span;
            assignment_mut(definition, endpoint).span =
                crate::source::Span::empty(span.source_id(), span.range().end() + 1);
        }
        let error = rewrite_program(input, |callable, edit| {
            plan.rewrite_callable(callable, edit).map(|_| ())
        })
        .unwrap_err();
        assert!(matches!(
            error,
            MirRewriteError::StaleCallableSnapshot { .. }
        ));
    }
}

#[test]
fn repeated_runs_produce_identical_canonical_dumps() {
    let build = || {
        let (mut input, root, function) = definition_with_root(MirPrimitiveType::U8);
        let definition = input.definitions.get_mut_for_test(function).unwrap();
        let first = append_cast(
            definition,
            root,
            MirPrimitiveType::U8,
            MirPrimitiveType::I64,
        );
        let _endpoint = append_cast(
            definition,
            first,
            MirPrimitiveType::I64,
            MirPrimitiveType::U64,
        );
        input
    };
    let first = run_mir_pipeline_with_occurrences(build(), &exact_schedule(2));
    let second = run_mir_pipeline_with_occurrences(build(), &exact_schedule(2));
    assert_eq!(
        dump_mir(first.result.as_ref().unwrap().program()),
        dump_mir(second.result.as_ref().unwrap().program())
    );
    assert_eq!(
        first
            .occurrences()
            .iter()
            .map(|record| (record.outcome(), record.measurements()))
            .collect::<Vec<_>>(),
        second
            .occurrences()
            .iter()
            .map(|record| (record.outcome(), record.measurements()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn long_chain_rewrite_batches_forwarding_and_removal_without_recursive_work() {
    const DEPTH: usize = 4_096;

    let (mut input, root, function) = definition_with_root(MirPrimitiveType::I64);
    let definition = input.definitions.get_mut_for_test(function).unwrap();
    let mut operand = root;
    let mut source = MirPrimitiveType::I64;
    for index in 0..DEPTH {
        let target = if index % 2 == 0 {
            MirPrimitiveType::U64
        } else {
            MirPrimitiveType::I64
        };
        operand = append_cast(definition, operand, source, target);
        source = target;
    }
    set_return(definition, operand);

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(1));
    let output = measured.result.as_ref().unwrap().program();
    let analysis = crate::passes::integer_cast::analyze_integer_cast_chains(
        output.definitions.get(function).unwrap().into(),
    )
    .unwrap();
    assert_eq!(analysis.candidates().count(), 0);
    assert_eq!(
        measured.occurrences()[0]
            .measurements()
            .iter()
            .find(|measurement| measurement.name() == MAXIMUM_DEPTH)
            .unwrap()
            .value(),
        DEPTH as u64
    );
}

fn assignment_mut(
    definition: &mut crate::mir::MirFunctionDefinition,
    result: ValueId,
) -> &mut MirAssignment {
    definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) if assignment.result == result => Some(assignment),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing assignment for {result}"))
}
