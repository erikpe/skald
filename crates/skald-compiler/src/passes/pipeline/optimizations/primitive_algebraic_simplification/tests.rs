use crate::{
    identity::CallableId,
    mir::{
        MirAssignment, MirBinaryOperation, MirComparisonOperand, MirComparisonPredicate,
        MirInstruction, MirIntegerDivisionKind, MirIntegerDivisionOperation, MirIntegerType,
        MirLogicalExpression, MirLogicalOperation, MirPrimitiveComparison, MirRvalue,
        MirRvalueKind, MirTerminator, MirType, MirValue, PathConditionId, StorageId, ValueId,
    },
    passes::{
        resolve_exact_mir_pass_schedule, run_mir_pipeline_with_occurrences, MirPassMeasurement,
        MirPassOccurrenceOutcome,
    },
    test_support::lower_source_to_final_mir,
};

use super::{
    scan_definition, CONSTANT_RESULTS, FORWARDED_USES, IDENTITY, PROTECTED_REJECTIONS,
    REMOVED_ASSIGNMENTS, REMOVED_VALUES,
};
use crate::passes::pipeline::optimizations::dead_pure_definition_elimination;

fn append_assignment(
    definition: &mut crate::mir::MirFunctionDefinition,
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

fn exact_schedule(identities: &[crate::passes::MirPassIdentity]) -> crate::passes::MirPassSchedule {
    resolve_exact_mir_pass_schedule(identities).unwrap()
}

fn measurements(values: [u64; 5]) -> [MirPassMeasurement; 5] {
    [
        MirPassMeasurement::count(CONSTANT_RESULTS, values[0]),
        MirPassMeasurement::count(FORWARDED_USES, values[1]),
        MirPassMeasurement::count(REMOVED_ASSIGNMENTS, values[2]),
        MirPassMeasurement::count(REMOVED_VALUES, values[3]),
        MirPassMeasurement::count(PROTECTED_REJECTIONS, values[4]),
    ]
}

#[test]
fn operand_identity_forwards_all_safe_uses_and_compacts_dense_values() {
    let mut input = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let entry = input.entry_function;
    let definition = input.definitions.get_mut_for_test(entry).unwrap();
    let source = append_assignment(definition, MirRvalueKind::ConstantI64(7), MirType::I64);
    let zero = append_assignment(definition, MirRvalueKind::ConstantI64(0), MirType::I64);
    let result = append_assignment(
        definition,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left: source,
            right: zero,
        },
        MirType::I64,
    );
    let consumer = append_assignment(
        definition,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left: result,
            right: source,
        },
        MirType::I64,
    );
    definition.body.blocks[0].terminator = Some(MirTerminator::Return {
        value: Some(result),
        span: definition.span,
    });
    let original_values = definition.values.len();

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let output = measured
        .result
        .as_ref()
        .unwrap()
        .program()
        .definitions
        .get(entry)
        .unwrap();
    let record = &measured.occurrences()[0];

    assert_eq!(output.values.len(), original_values - 1);
    assert!(output.body.blocks[0]
        .instructions
        .iter()
        .all(|instruction| !matches!(
            instruction,
            MirInstruction::Assign(assignment)
                if assignment.rvalue.kind
                    == (MirRvalueKind::Binary {
                        operation: MirBinaryOperation::AddI64,
                        left: source,
                        right: zero,
                    })
        )));
    let remapped_consumer = ValueId::new(CallableId::Function(entry), consumer.index() - 1);
    let consumer_assignment = output.body.blocks[0]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) if assignment.result == remapped_consumer => {
                Some(assignment)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        consumer_assignment.rvalue.kind,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left: source,
            right: source,
        }
    );
    assert_eq!(
        output.body.blocks[0].terminator,
        Some(MirTerminator::Return {
            value: Some(source),
            span: output.span,
        })
    );
    assert!(output.body.blocks[0]
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, MirInstruction::Assign(assignment) if assignment.result == zero)));
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(record.removed_mir_entities(), Some(1));
    assert_eq!(record.measurements(), measurements([0, 2, 1, 1, 0]));
}

#[test]
fn constant_identity_preserves_result_assignment_type_position_and_span() {
    let mut input = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let entry = input.entry_function;
    let definition = input.definitions.get_mut_for_test(entry).unwrap();
    let source = append_assignment(definition, MirRvalueKind::ConstantU8(9), MirType::U8);
    let result = append_assignment(
        definition,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::SubtractU8,
            left: source,
            right: source,
        },
        MirType::U8,
    );
    let original = definition.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Assign(assignment) if assignment.result == result))
        .unwrap();
    let original_span = definition.body.blocks[0].instructions[original].span();

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let output = measured
        .result
        .as_ref()
        .unwrap()
        .program()
        .definitions
        .get(entry)
        .unwrap();
    let MirInstruction::Assign(assignment) = &output.body.blocks[0].instructions[original] else {
        panic!("constant-result assignment changed position")
    };
    assert_eq!(assignment.result, result);
    assert_eq!(assignment.rvalue.ty, MirType::U8);
    assert_eq!(assignment.rvalue.kind, MirRvalueKind::ConstantU8(0));
    assert_eq!(assignment.span, original_span);
    assert_eq!(
        measured.occurrences()[0].measurements(),
        measurements([1, 0, 0, 0, 0])
    );
}

#[test]
fn proof_and_checked_uses_are_counted_as_protected_without_mutation() {
    let mut input = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let entry = input.entry_function;
    let definition = input.definitions.get_mut_for_test(entry).unwrap();
    let source = append_assignment(definition, MirRvalueKind::ConstantI64(8), MirType::I64);
    let zero = append_assignment(definition, MirRvalueKind::ConstantI64(0), MirType::I64);
    let result = append_assignment(
        definition,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left: source,
            right: zero,
        },
        MirType::I64,
    );
    append_assignment(
        definition,
        MirRvalueKind::IntegerDivision {
            operation: MirIntegerDivisionOperation {
                kind: MirIntegerDivisionKind::Quotient,
                operand: MirIntegerType::I64,
            },
            dividend: result,
            divisor: source,
        },
        MirType::I64,
    );
    let block = definition.body.entry;
    definition
        .body
        .logical_expressions
        .push(MirLogicalExpression {
            operation: MirLogicalOperation::And,
            condition: PathConditionId::new(result.callable(), 0),
            result: StorageId::new(result.callable(), 0),
            left_result: result,
            split: block,
            selection: block,
            right_entry: block,
            right_exit: block,
            right_result: source,
            short: block,
            join: block,
            selected_result: result,
            span: definition.span,
        });
    definition.body.blocks[0].terminator = Some(MirTerminator::Return {
        value: Some(result),
        span: definition.span,
    });

    let scan = scan_definition((&*definition).into()).unwrap();
    assert_eq!(scan.candidate, None);
    assert_eq!(scan.protected_rejections, 1);
}

#[test]
fn candidate_free_program_keeps_the_verified_seal() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let expected = input.clone();

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let record = &measured.occurrences()[0];
    assert_eq!(measured.result.as_ref().unwrap().program(), &expected);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Unchanged);
    assert_eq!(record.verification_executions(), 0);
    assert_eq!(record.measurements(), measurements([0, 0, 0, 0, 0]));
}

#[test]
fn integer_and_bool_self_comparisons_are_constant_but_float_is_unchanged() {
    let mut input = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let entry = input.entry_function;
    let definition = input.definitions.get_mut_for_test(entry).unwrap();
    let integer = append_assignment(definition, MirRvalueKind::ConstantI64(4), MirType::I64);
    let boolean = append_assignment(definition, MirRvalueKind::ConstantBool(true), MirType::Bool);
    let float = append_assignment(definition, MirRvalueKind::ConstantF64Bits(0), MirType::F64);
    let equal = |operand, value| MirRvalueKind::PrimitiveComparison {
        operation: MirPrimitiveComparison {
            predicate: MirComparisonPredicate::Equal,
            operand,
        },
        left: value,
        right: value,
    };
    let integer_result = append_assignment(
        definition,
        equal(MirComparisonOperand::Integer(MirIntegerType::I64), integer),
        MirType::Bool,
    );
    let bool_result = append_assignment(
        definition,
        equal(MirComparisonOperand::Bool, boolean),
        MirType::Bool,
    );
    let float_result = append_assignment(
        definition,
        equal(MirComparisonOperand::F64, float),
        MirType::Bool,
    );

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let output = measured
        .result
        .as_ref()
        .unwrap()
        .program()
        .definitions
        .get(entry)
        .unwrap();
    let kind = |result| {
        output.body.blocks[0]
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                MirInstruction::Assign(assignment) if assignment.result == result => {
                    Some(&assignment.rvalue.kind)
                }
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(kind(integer_result), &MirRvalueKind::ConstantBool(true));
    assert_eq!(kind(bool_result), &MirRvalueKind::ConstantBool(true));
    assert!(matches!(
        kind(float_result),
        MirRvalueKind::PrimitiveComparison { .. }
    ));
}

#[test]
fn repeated_occurrence_is_stable_and_dead_pure_removes_retained_operands_later() {
    let mut input = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let entry = input.entry_function;
    let definition = input.definitions.get_mut_for_test(entry).unwrap();
    let source = append_assignment(definition, MirRvalueKind::ConstantI64(3), MirType::I64);
    let zero = append_assignment(definition, MirRvalueKind::ConstantI64(0), MirType::I64);
    let result = append_assignment(
        definition,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left: source,
            right: zero,
        },
        MirType::I64,
    );
    definition.body.blocks[0].terminator = Some(MirTerminator::Return {
        value: Some(result),
        span: definition.span,
    });

    let schedule = exact_schedule(&[
        IDENTITY,
        IDENTITY,
        dead_pure_definition_elimination::IDENTITY,
    ]);
    let measured = run_mir_pipeline_with_occurrences(input, &schedule);
    assert_eq!(
        measured
            .occurrences()
            .iter()
            .map(|record| record.outcome())
            .collect::<Vec<_>>(),
        vec![
            MirPassOccurrenceOutcome::Changed,
            MirPassOccurrenceOutcome::Unchanged,
            MirPassOccurrenceOutcome::Changed,
        ]
    );
    let output = measured.result.as_ref().unwrap().program();
    let definition = output.definitions.get(entry).unwrap();
    assert!(definition.body.blocks[0]
        .instructions
        .iter()
        .all(|instruction| !matches!(instruction, MirInstruction::Assign(assignment) if assignment.rvalue.kind == MirRvalueKind::ConstantI64(0))));
}
