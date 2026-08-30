use crate::{
    identity::CallableId,
    mir::{
        dump_mir, MirAssignment, MirBinaryOperation, MirBody, MirInstruction, MirRvalue,
        MirRvalueKind, MirType, MirUnaryOperation, MirValue, ValueId,
    },
    passes::{
        resolve_exact_mir_pass_schedule, run_mir_pipeline, run_mir_pipeline_with_occurrences,
        MirPassMeasurement, MirPassOccurrenceOutcome,
    },
    source::Span,
    test_support::lower_source_to_final_mir,
};

use super::{removable_rvalue, IDENTITY, REMOVED_ASSIGNMENTS, REMOVED_VALUE_DECLARATIONS};

fn exact_schedule() -> super::super::super::policy::MirPassSchedule {
    resolve_exact_mir_pass_schedule(&[IDENTITY]).unwrap()
}

fn dead_tree_program() -> crate::mir::MirProgram {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let callable = CallableId::Function(function.function);
    let span = function.span;
    let first = ValueId::new(callable, function.values.len());
    let second = ValueId::new(callable, function.values.len() + 1);
    let third = ValueId::new(callable, function.values.len() + 2);
    function
        .values
        .extend([first, second, third].map(|id| MirValue {
            id,
            ty: MirType::I64,
            span,
        }));
    function.body.blocks[0].instructions.extend([
        MirInstruction::Assign(MirAssignment {
            result: first,
            rvalue: MirRvalue {
                kind: MirRvalueKind::ConstantI64(4),
                ty: MirType::I64,
            },
            span,
        }),
        MirInstruction::Assign(MirAssignment {
            result: second,
            rvalue: MirRvalue {
                kind: MirRvalueKind::Unary {
                    operation: MirUnaryOperation::NegateI64,
                    operand: first,
                },
                ty: MirType::I64,
            },
            span,
        }),
        MirInstruction::Assign(MirAssignment {
            result: third,
            rvalue: MirRvalue {
                kind: MirRvalueKind::Binary {
                    operation: MirBinaryOperation::AddI64,
                    left: second,
                    right: first,
                },
                ty: MirType::I64,
            },
            span,
        }),
    ]);
    program
}

fn append_value(
    callable: CallableId,
    values: &mut Vec<MirValue>,
    instructions: &mut Vec<MirInstruction>,
    kind: MirRvalueKind,
    ty: MirType,
    span: Span,
) -> ValueId {
    let result = ValueId::new(callable, values.len());
    values.push(MirValue {
        id: result,
        ty,
        span,
    });
    instructions.push(MirInstruction::Assign(MirAssignment {
        result,
        rvalue: MirRvalue { kind, ty },
        span,
    }));
    result
}

fn inject_dead_constant(
    callable: CallableId,
    values: &mut Vec<MirValue>,
    body: &mut MirBody,
    span: Span,
) {
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
                kind: MirRvalueKind::ConstantI64(91),
                ty: MirType::I64,
            },
            span,
        }));
}

#[test]
fn scalar_whitelist_is_explicit_and_rejects_checked_casts() {
    use crate::mir::{MirPrimitiveCast, MirPrimitiveType};

    let callable = CallableId::Function(crate::identity::FunctionId::new(0));
    let value = ValueId::new(callable, 0);
    let eligible = [
        MirRvalueKind::ConstantI64(0),
        MirRvalueKind::ConstantU64(0),
        MirRvalueKind::ConstantU8(0),
        MirRvalueKind::ConstantF64Bits(0),
        MirRvalueKind::ConstantBool(false),
        MirRvalueKind::Unary {
            operation: MirUnaryOperation::NegateI64,
            operand: value,
        },
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left: value,
            right: value,
        },
        MirRvalueKind::PrimitiveComparison {
            operation: crate::mir::MirPrimitiveComparison {
                predicate: crate::mir::MirComparisonPredicate::Equal,
                operand: crate::mir::MirComparisonOperand::Bool,
            },
            left: value,
            right: value,
        },
        MirRvalueKind::PrimitiveCast {
            operation: MirPrimitiveCast::new(MirPrimitiveType::U8, MirPrimitiveType::U64),
            operand: value,
        },
    ];

    assert!(eligible.iter().all(removable_rvalue));
    assert!(!removable_rvalue(&MirRvalueKind::PrimitiveCast {
        operation: MirPrimitiveCast::new(MirPrimitiveType::F64, MirPrimitiveType::I64),
        operand: value,
    }));
}

#[test]
fn every_current_excluded_rvalue_family_is_rejected() {
    use crate::{
        identity::{ArrayTypeId, ClassId, FunctionId, FunctionTypeId, OptionalBoxTypeId},
        mir::{
            MirAliasAccess, MirCallableAddress, MirF64ToIntegerRange, MirIntegerDivisionKind,
            MirIntegerDivisionOperation, MirIntegerType, MirObjectOrigin, MirObjectView,
            MirPathConditionValue, MirPlace, MirPresenceTestKind, MirShiftDirection,
            MirShiftOperation, MirViewProvenance, MirViewTarget, PathConditionId, StorageId,
        },
    };

    let program = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let definition = program.definitions.get(program.entry_function).unwrap();
    let callable = CallableId::Function(definition.function);
    let value = ValueId::new(callable, 0);
    let storage = StorageId::new(callable, 0);
    let class = ClassId::new(0);
    let view = MirObjectView {
        source: MirPlace::base(storage),
        origin: Box::new(MirObjectOrigin::Exact {
            complete: MirPlace::base(storage),
            dynamic_class: class,
        }),
        target: MirViewTarget::Class(class),
        access: MirAliasAccess::ReadOnly,
        provenance: MirViewProvenance::Ordinary,
        span: definition.span,
    };
    let excluded = [
        MirRvalueKind::CallableAddress(MirCallableAddress {
            target: CallableId::Function(FunctionId::new(0)),
            function_type: FunctionTypeId::new(0),
        }),
        MirRvalueKind::PathCondition(MirPathConditionValue {
            condition: PathConditionId::new(callable, 0),
            activation: storage,
        }),
        MirRvalueKind::Load(MirPlace::base(storage)),
        MirRvalueKind::IntegerDivision {
            operation: MirIntegerDivisionOperation {
                kind: MirIntegerDivisionKind::Quotient,
                operand: MirIntegerType::I64,
            },
            dividend: value,
            divisor: value,
        },
        MirRvalueKind::Shift {
            operation: MirShiftOperation {
                direction: MirShiftDirection::Left,
                left: MirIntegerType::I64,
            },
            left: value,
            count: value,
        },
        MirRvalueKind::CheckedF64ToInteger {
            relation: MirF64ToIntegerRange {
                target: MirIntegerType::I64,
            },
            operand: value,
        },
        MirRvalueKind::TypeTest {
            source: view,
            target: MirViewTarget::Class(class),
        },
        MirRvalueKind::OptionalPresence {
            source: MirPlace::base(storage),
            kind: MirPresenceTestKind::Some,
        },
        MirRvalueKind::OptionalBoxPresence {
            owner: storage,
            target: OptionalBoxTypeId::new(0),
            layer: 0,
            kind: MirPresenceTestKind::Some,
        },
        MirRvalueKind::ArrayLength {
            source: MirPlace::base(storage),
            array: ArrayTypeId::new(0),
        },
    ];

    assert!(excluded.iter().all(|kind| !removable_rvalue(kind)));
}

#[test]
fn every_eligible_family_is_removed_when_unused_and_retained_when_used() {
    use crate::mir::{
        MirComparisonOperand, MirComparisonPredicate, MirPrimitiveCast, MirPrimitiveComparison,
        MirPrimitiveType,
    };

    let expected = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let mut input = expected.clone();
    let definition = input
        .definitions
        .get_mut_for_test(input.entry_function)
        .unwrap();
    let callable = CallableId::Function(definition.function);
    let span = definition.span;
    let values = &mut definition.values;
    let instructions = &mut definition.body.blocks[0].instructions;
    let signed = append_value(
        callable,
        values,
        instructions,
        MirRvalueKind::ConstantI64(1),
        MirType::I64,
        span,
    );
    append_value(
        callable,
        values,
        instructions,
        MirRvalueKind::ConstantU64(2),
        MirType::U64,
        span,
    );
    let byte = append_value(
        callable,
        values,
        instructions,
        MirRvalueKind::ConstantU8(3),
        MirType::U8,
        span,
    );
    append_value(
        callable,
        values,
        instructions,
        MirRvalueKind::ConstantF64Bits(4.0f64.to_bits()),
        MirType::F64,
        span,
    );
    let boolean = append_value(
        callable,
        values,
        instructions,
        MirRvalueKind::ConstantBool(true),
        MirType::Bool,
        span,
    );
    append_value(
        callable,
        values,
        instructions,
        MirRvalueKind::Unary {
            operation: MirUnaryOperation::NegateI64,
            operand: signed,
        },
        MirType::I64,
        span,
    );
    append_value(
        callable,
        values,
        instructions,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left: signed,
            right: signed,
        },
        MirType::I64,
        span,
    );
    append_value(
        callable,
        values,
        instructions,
        MirRvalueKind::PrimitiveComparison {
            operation: MirPrimitiveComparison {
                predicate: MirComparisonPredicate::Equal,
                operand: MirComparisonOperand::Bool,
            },
            left: boolean,
            right: boolean,
        },
        MirType::Bool,
        span,
    );
    append_value(
        callable,
        values,
        instructions,
        MirRvalueKind::PrimitiveCast {
            operation: MirPrimitiveCast::new(MirPrimitiveType::U8, MirPrimitiveType::U64),
            operand: byte,
        },
        MirType::U64,
        span,
    );

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule());
    assert_eq!(measured.result.as_ref().unwrap().program(), &expected);
    assert_eq!(
        measured.occurrences()[0].measurements(),
        [
            MirPassMeasurement::count(REMOVED_ASSIGNMENTS, 9),
            MirPassMeasurement::count(REMOVED_VALUE_DECLARATIONS, 9),
        ]
    );

    let used = lower_source_to_final_mir(concat!(
        "fn floating(value: f64) -> f64 { return -value + 1.0; }\n",
        "fn truth() -> bool { return !false; }\n",
        "fn unsigned() -> u64 { return (u64) 1u8 + 2u; }\n",
        "fn main() -> i64 { if (truth() && 1 < 2) { return (i64) unsigned(); } return (i64) floating(3.0); }\n",
    ));
    let used_dump = dump_mir(&used);
    let measured = run_mir_pipeline_with_occurrences(used, &exact_schedule());

    assert_eq!(
        dump_mir(measured.result.as_ref().unwrap().program()),
        used_dump
    );
    assert_eq!(
        measured.occurrences()[0].outcome(),
        MirPassOccurrenceOutcome::Unchanged
    );
}

#[test]
fn cascading_dead_tree_reaches_a_fixed_point_and_reports_exact_work() {
    let input = dead_tree_program();
    let original_value_count = input
        .definitions
        .get(input.entry_function)
        .unwrap()
        .values
        .len();
    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule());
    let output = measured.result.as_ref().unwrap();
    let record = &measured.occurrences()[0];

    assert_eq!(
        output
            .definitions
            .get(output.entry_function)
            .unwrap()
            .values
            .len(),
        original_value_count - 3
    );
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(record.processed_callables(), Some(1));
    assert_eq!(record.changed_callables(), Some(1));
    assert_eq!(record.removed_mir_entities(), Some(3));
    assert_eq!(record.verification_executions(), 1);
    assert_eq!(
        record.measurements(),
        [
            MirPassMeasurement::count(REMOVED_ASSIGNMENTS, 3),
            MirPassMeasurement::count(REMOVED_VALUE_DECLARATIONS, 3),
        ]
    );
}

#[test]
fn no_op_exact_run_preserves_the_seal_and_exact_mir() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let expected = dump_mir(&input);
    let processed = input.executable_definitions().count() as u64;
    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule());
    let record = &measured.occurrences()[0];

    assert_eq!(
        dump_mir(measured.result.as_ref().unwrap().program()),
        expected
    );
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Unchanged);
    assert_eq!(record.processed_callables(), Some(processed));
    assert_eq!(record.changed_callables(), Some(0));
    assert_eq!(record.verification_executions(), 0);
    assert_eq!(
        record.measurements(),
        [
            MirPassMeasurement::count(REMOVED_ASSIGNMENTS, 0),
            MirPassMeasurement::count(REMOVED_VALUE_DECLARATIONS, 0),
        ]
    );
}

#[test]
fn registered_canary_remains_inactive_in_the_default_profile() {
    let input = dead_tree_program();
    let expected = input.clone();

    assert_eq!(run_mir_pipeline(input).unwrap().program(), &expected);
}

#[test]
fn retained_instructions_keep_their_relative_order_after_dense_remapping() {
    let input = dead_tree_program();
    let expected = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let output = run_mir_pipeline_with_occurrences(input, &exact_schedule())
        .result
        .unwrap();
    let definition = output.definitions.get(output.entry_function).unwrap();

    assert_eq!(output.program(), &expected);
    for (index, value) in definition.values.iter().enumerate() {
        assert_eq!(value.id.index(), index);
    }
}

#[test]
fn whole_program_rewrite_covers_every_executable_callable_kind_once() {
    let original = lower_source_to_final_mir(concat!(
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
    let mut input = original.clone();
    let function_ids = input
        .definitions
        .iter()
        .map(|definition| definition.function)
        .collect::<Vec<_>>();
    for function in function_ids {
        let definition = input.definitions.get_mut_for_test(function).unwrap();
        inject_dead_constant(
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
        inject_dead_constant(
            callable,
            &mut definition.values,
            &mut definition.body,
            definition.span,
        );
    }
    if let Some(coordinator) = &mut input.static_lifecycle {
        for definition in coordinator.initializers_mut_for_test() {
            inject_dead_constant(
                CallableId::StaticInitializer(definition.id),
                &mut definition.values,
                &mut definition.body,
                definition.span,
            );
        }
    }
    let executable_count = input.executable_definitions().count() as u64;

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule());
    let record = &measured.occurrences()[0];

    assert_eq!(measured.result.as_ref().unwrap().program(), &original);
    assert_eq!(record.processed_callables(), Some(executable_count));
    assert_eq!(record.changed_callables(), Some(executable_count));
    assert_eq!(
        record.measurements(),
        [
            MirPassMeasurement::count(REMOVED_ASSIGNMENTS, executable_count),
            MirPassMeasurement::count(REMOVED_VALUE_DECLARATIONS, executable_count),
        ]
    );
    assert_eq!(record.verification_executions(), 1);
}

#[test]
fn unused_call_and_io_results_are_not_treated_as_pure_assignments() {
    let mut call_program = lower_source_to_final_mir(concat!(
        "fn produce() -> i64 { return 7; }\n",
        "fn main() -> i64 { return produce(); }\n",
    ));
    let entry = call_program.entry_function;
    let call_definition = call_program.definitions.get_mut_for_test(entry).unwrap();
    let (block_index, instruction_index, mut duplicate, original_result) =
        call_definition
            .body
            .blocks
            .iter()
            .enumerate()
            .find_map(|(block_index, block)| {
                block.instructions.iter().enumerate().find_map(
                    |(instruction_index, instruction)| match instruction {
                        MirInstruction::Call(call) => call
                            .result
                            .map(|result| (block_index, instruction_index, call.clone(), result)),
                        _ => None,
                    },
                )
            })
            .unwrap();
    let call_result = ValueId::new(CallableId::Function(entry), call_definition.values.len());
    duplicate.result = Some(call_result);
    let result_type = call_definition.value(original_result).unwrap().ty;
    call_definition.values.push(MirValue {
        id: call_result,
        ty: result_type,
        span: duplicate.span,
    });
    call_definition.body.blocks[block_index]
        .instructions
        .insert(instruction_index + 1, MirInstruction::Call(duplicate));

    let call_output = run_mir_pipeline_with_occurrences(call_program, &exact_schedule())
        .result
        .unwrap();
    let call_definition = call_output.definitions.get(entry).unwrap();
    assert!(call_definition.value(call_result).is_some());
    assert!(call_definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|instruction| matches!(instruction, MirInstruction::Call(call) if call.result == Some(call_result))));

    let mut io_program = crate::mir::test_fixtures::io_program();
    let (function_id, block_index, instruction_index, mut duplicate, original_result) = io_program
        .definitions
        .iter()
        .find_map(|definition| {
            definition
                .body
                .blocks
                .iter()
                .enumerate()
                .find_map(|(block_index, block)| {
                    block.instructions.iter().enumerate().find_map(
                        |(instruction_index, instruction)| match instruction {
                            MirInstruction::Io(io) => Some((
                                definition.function,
                                block_index,
                                instruction_index,
                                io.clone(),
                                io.result,
                            )),
                            _ => None,
                        },
                    )
                })
        })
        .unwrap();
    let io_definition = io_program
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let io_result = ValueId::new(
        CallableId::Function(function_id),
        io_definition.values.len(),
    );
    duplicate.result = io_result;
    let result_type = io_definition.value(original_result).unwrap().ty;
    io_definition.values.push(MirValue {
        id: io_result,
        ty: result_type,
        span: duplicate.span,
    });
    io_definition.body.blocks[block_index]
        .instructions
        .insert(instruction_index + 1, MirInstruction::Io(duplicate));

    let io_output = run_mir_pipeline_with_occurrences(io_program, &exact_schedule())
        .result
        .unwrap();
    let io_definition = io_output.definitions.get(function_id).unwrap();
    assert!(io_definition.value(io_result).is_some());
    assert!(io_definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(
            |instruction| matches!(instruction, MirInstruction::Io(io) if io.result == io_result)
        ));
}

#[test]
fn excluded_feature_fixtures_remain_valid_through_the_exact_pass() {
    let sources = [
        concat!(
            "fn add(left: i64, right: i64) -> i64 { return left + right; }\n",
            "fn choose() -> fn(i64, i64) -> i64 { return add; }\n",
            "fn invoke(callback: fn(i64, i64) -> i64) -> i64 { return callback(40, 2); }\n",
            "fn main() -> i64 { return invoke(choose()); }\n",
        ),
        concat!(
            "fn checked(divisor: i64, count: u64, value: f64) -> i64 {\n",
            "  return ((1 / divisor) << count) + (i64) value;\n",
            "}\n",
            "fn main() -> i64 { return checked(2, 1u, 3.0); }\n",
        ),
        concat!(
            "class Item { value: i64; init(value: i64) { self.value = value; }\n",
            "  fn read() -> i64 { return self.value; } destroy {} }\n",
            "fn main() -> i64 {\n",
            "  var maybe: i64? = 1;\n",
            "  var values: i64[] = i64[]{2, 3};\n",
            "  var owner: shared Item = new Item(4);\n",
            "  if (maybe is some) { return maybe! + (i64) values.len() + owner->read(); }\n",
            "  return 0;\n",
            "}\n",
        ),
        concat!(
            "class State { static value: i64 = initialize(); init() {} }\n",
            "fn initialize() -> i64 { return 7; }\n",
            "fn main() -> i64 { return State.value; }\n",
        ),
    ];

    for source in sources {
        let program = lower_source_to_final_mir(source);
        let measured = run_mir_pipeline_with_occurrences(program, &exact_schedule());
        assert!(measured.result.is_ok(), "{source}");
        assert_eq!(measured.occurrences().len(), 1);
    }
}
