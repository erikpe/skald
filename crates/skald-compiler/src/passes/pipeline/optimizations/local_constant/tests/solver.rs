use crate::{
    identity::{CallableId, FunctionId},
    mir::{
        MirAssignment, MirBinaryOperation, MirDefinitionRef, MirInstruction, MirRvalue,
        MirRvalueKind, MirTerminationReason, MirTerminator, MirType, MirValue, ValueId,
    },
    source::Span,
    test_support::lower_source_to_final_mir,
};

use super::super::{
    solve::solve_local_constants_with_reversed_seeds, solve_local_constants,
    LocalConstantAnalysisError, LocalConstantIdentity, LocalConstantProvenanceCategory,
    LogicalSelectionKind,
};
use crate::passes::pipeline::optimizations::primitive_evaluation::PrimitiveConstant;

fn entry_definition(program: &crate::mir::MirProgram) -> &crate::mir::MirFunctionDefinition {
    program.definitions.get(program.entry_function).unwrap()
}

fn entry_definition_mut(
    program: &mut crate::mir::MirProgram,
) -> &mut crate::mir::MirFunctionDefinition {
    program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap()
}

fn returned_value(definition: MirDefinitionRef<'_>) -> ValueId {
    definition
        .body()
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::Return {
                value: Some(value), ..
            }) => Some(value),
            _ => None,
        })
        .unwrap()
}

fn logical_definition(program: &crate::mir::MirProgram) -> MirDefinitionRef<'_> {
    program
        .executable_definitions()
        .find(|definition| !definition.logical_expressions().is_empty())
        .unwrap()
}

fn solved_return(source: &str) -> PrimitiveConstant {
    let program = lower_source_to_final_mir(source);
    let definition = entry_definition(&program);
    let solution = solve_local_constants(definition.into()).unwrap();
    solution
        .constant(returned_value(definition.into()))
        .unwrap()
        .unwrap_or_else(|| panic!("return remained unknown; facts: {:?}", solution.facts()))
}

#[test]
fn solves_primitive_cast_comparison_and_checked_protocol_chains() {
    assert_eq!(
        solved_return("fn main() -> i64 { return 23 + (i64) (17u + 12u); }"),
        PrimitiveConstant::I64(52)
    );
    assert_eq!(
        solved_return("fn main() -> i64 { return ((8 / 2) + 2) / 3; }"),
        PrimitiveConstant::I64(2)
    );
    assert_eq!(
        solved_return("fn main() -> i64 { return (1 << 2u) << 1u; }"),
        PrimitiveConstant::I64(8)
    );
    let mixed = lower_source_to_final_mir(
        "fn main() -> i64 { return ((i64) (((8 / 2) < 5) && true) + 3) / 2; }",
    );
    let definition = entry_definition(&mixed);
    let outer_checked_result =
        crate::passes::pipeline::optimizations::checked_integer_topology::observe_checked_integer_topologies(
            definition.into(),
        )
        .unwrap()
        .into_iter()
        .filter_map(|observation| match observation {
            crate::passes::pipeline::optimizations::checked_integer_topology::CheckedIntegerTopologyObservation::Protocol(topology) => {
                Some(topology.result_reload.value)
            }
            crate::passes::pipeline::optimizations::checked_integer_topology::CheckedIntegerTopologyObservation::Rejected { .. } => None,
        })
        .next_back()
        .unwrap();
    assert_eq!(
        solve_local_constants(definition.into())
            .unwrap()
            .constant(outer_checked_result)
            .unwrap(),
        Some(PrimitiveConstant::I64(2))
    );

    let program = lower_source_to_final_mir(concat!(
        "fn compare() -> bool { return ((8 / 2) + 1) < 6; } ",
        "fn main() -> i64 { return 0; }",
    ));
    let definition = program
        .executable_definitions()
        .find(|definition| {
            definition.body().blocks.iter().any(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(instruction, MirInstruction::Assign(assignment) if matches!(assignment.rvalue.kind, MirRvalueKind::PrimitiveComparison { .. }))
                })
            })
        })
        .unwrap();
    let comparison = definition
        .body()
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
    assert_eq!(
        solve_local_constants(definition)
            .unwrap()
            .constant(comparison)
            .unwrap(),
        Some(PrimitiveConstant::Bool(true))
    );
}

#[test]
fn preserves_exact_integer_boundaries_and_records_static_failures() {
    assert_eq!(
        solved_return("fn main() -> i64 { return -7 / 3; }"),
        PrimitiveConstant::I64(-3)
    );
    assert_eq!(
        solved_return("fn main() -> i64 { return -7 % 3; }"),
        PrimitiveConstant::I64(2)
    );
    assert_eq!(
        solved_return("fn main() -> i64 { return -8 >> 1u; }"),
        PrimitiveConstant::I64(-4)
    );
    assert_eq!(
        solved_return("fn main() -> i64 { return (i64) ((255u8 + 2u8) << 1u); }"),
        PrimitiveConstant::I64(2)
    );
    assert_eq!(
        solved_return("fn main() -> i64 { return (i64) ((8u + 1u) / 3u); }"),
        PrimitiveConstant::I64(3)
    );
    assert_eq!(
        solved_return("fn main() -> i64 { return (i64) ((1u + 1u) << 3u); }"),
        PrimitiveConstant::I64(16)
    );

    let program = lower_source_to_final_mir("fn main() -> i64 { return 9 / 0; }");
    let definition = entry_definition(&program);
    let solution = solve_local_constants(definition.into()).unwrap();
    assert_eq!(
        solution
            .constant(returned_value(definition.into()))
            .unwrap(),
        None
    );
    assert_eq!(
        solution.retained_checked_failures().len(),
        1,
        "facts: {:?}",
        solution.facts()
    );
    assert_eq!(
        solution.retained_checked_failures()[0].reason(),
        MirTerminationReason::IntegerDivisionByZero
    );
    assert_eq!(
        solution.retained_checked_failures()[0].result().callable(),
        definition.callable()
    );
    assert_eq!(
        solution.retained_checked_failures()[0]
            .check_block()
            .callable(),
        definition.callable()
    );
}

#[test]
fn selects_all_constant_left_logical_rules_without_requiring_a_skipped_rhs() {
    let cases = [
        (
            "fn dynamic() -> bool { return true; } fn choose() -> bool { return false && dynamic(); } fn main() -> i64 { return 0; }",
            LogicalSelectionKind::Short,
            Some(PrimitiveConstant::Bool(false)),
        ),
        (
            "fn choose() -> bool { return true && false; } fn main() -> i64 { return 0; }",
            LogicalSelectionKind::Right,
            Some(PrimitiveConstant::Bool(false)),
        ),
        (
            "fn dynamic() -> bool { return false; } fn choose() -> bool { return true || dynamic(); } fn main() -> i64 { return 0; }",
            LogicalSelectionKind::Short,
            Some(PrimitiveConstant::Bool(true)),
        ),
        (
            "fn choose() -> bool { return false || true; } fn main() -> i64 { return 0; }",
            LogicalSelectionKind::Right,
            Some(PrimitiveConstant::Bool(true)),
        ),
    ];

    for (source, expected_kind, expected_constant) in cases {
        let program = lower_source_to_final_mir(source);
        let definition = logical_definition(&program);
        let topology = &definition.logical_expressions()[0];
        let solution = solve_local_constants(definition).unwrap();
        let selection = solution.selection(0).unwrap().unwrap();
        assert_eq!(selection.record_index(), 0);
        assert_eq!(selection.kind(), expected_kind);
        assert_eq!(selection.constant(), expected_constant);
        assert_eq!(
            solution.constant(topology.selected_result).unwrap(),
            expected_constant
        );
    }
}

#[test]
fn skipped_checked_failures_do_not_block_short_results_but_selected_failures_do() {
    let skipped = lower_source_to_final_mir(concat!(
        "fn choose() -> bool { return false && (1 / 0 == 0); } ",
        "fn main() -> i64 { return 0; }",
    ));
    let definition = logical_definition(&skipped);
    let selected_result = definition.logical_expressions()[0].selected_result;
    let solution = solve_local_constants(definition).unwrap();
    assert_eq!(
        solution.constant(selected_result).unwrap(),
        Some(PrimitiveConstant::Bool(false))
    );
    assert_eq!(
        solution.retained_checked_failures().len(),
        1,
        "facts: {:?}; carriers: {:?}",
        solution.facts(),
        super::super::carrier::certify_checked_integer_carriers(definition).unwrap()
    );

    let selected = lower_source_to_final_mir(concat!(
        "fn choose() -> bool { return true && (1 / 0 == 0); } ",
        "fn main() -> i64 { return 0; }",
    ));
    let definition = logical_definition(&selected);
    let selected_result = definition.logical_expressions()[0].selected_result;
    let solution = solve_local_constants(definition).unwrap();
    assert_eq!(
        solution.selection(0).unwrap().unwrap().kind(),
        LogicalSelectionKind::Right
    );
    assert_eq!(solution.constant(selected_result).unwrap(), None);
    assert_eq!(solution.retained_checked_failures().len(), 1);
}

#[test]
fn solution_order_and_provenance_are_stable_across_worklist_seed_orders() {
    let program =
        lower_source_to_final_mir("fn main() -> i64 { return (((8 / 2) + 1) << 1u) * 5; }");
    let original = program.clone();
    let definition = entry_definition(&program);
    let forward = solve_local_constants(definition.into()).unwrap();
    let reversed = solve_local_constants_with_reversed_seeds(definition.into()).unwrap();
    assert_eq!(forward, reversed);
    assert!(forward
        .facts()
        .windows(2)
        .all(|facts| facts[0].identity() < facts[1].identity()));

    let result = returned_value(definition.into());
    let fact = forward
        .facts()
        .iter()
        .find(|fact| fact.identity() == LocalConstantIdentity::Value(result))
        .copied()
        .unwrap_or_else(|| panic!("missing {result}; facts: {:?}", forward.facts()));
    assert_eq!(fact.constant(), PrimitiveConstant::I64(50));
    assert_eq!(
        fact.provenance().category(),
        LocalConstantProvenanceCategory::Primitive
    );
    assert!(fact.provenance().crossed_carrier());
    assert!(fact.provenance().crossed_checked());
    assert!(!fact.provenance().crossed_logical());
    assert!(fact.provenance().depth() > 4);
    let carrier = forward
        .facts()
        .iter()
        .find_map(|fact| match fact.identity() {
            LocalConstantIdentity::Carrier(storage) => Some(storage),
            LocalConstantIdentity::Value(_) => None,
        })
        .unwrap();
    assert!(forward.carrier_constant(carrier).unwrap().is_some());
    assert_eq!(program, original);
}

#[test]
fn nested_logical_relations_converge_in_record_order() {
    let program = lower_source_to_final_mir(concat!(
        "fn dynamic() -> bool { return false; } ",
        "fn choose() -> bool { return (false && dynamic()) || true; } ",
        "fn main() -> i64 { return 0; }",
    ));
    let definition = logical_definition(&program);
    let solution = solve_local_constants(definition).unwrap();
    assert_eq!(solution.selections().len(), 2);
    assert!(solution
        .selections()
        .windows(2)
        .all(|pair| pair[0].record_index() < pair[1].record_index()));
    assert_eq!(
        solution
            .constant(definition.logical_expressions()[0].selected_result)
            .unwrap(),
        Some(PrimitiveConstant::Bool(true))
    );
    assert!(solution
        .facts()
        .iter()
        .any(|fact| fact.provenance().crossed_logical()));
}

#[test]
fn iterative_solver_handles_depth_far_beyond_normal_source_nesting() {
    const DEPTH: usize = 2_048;
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let definition = entry_definition_mut(&mut program);
    let callable = definition.callable();
    let span = definition.span;
    let block = definition.body.entry;
    let one = push_assignment(
        definition,
        block,
        span,
        MirRvalueKind::ConstantI64(1),
        MirType::I64,
    );
    let mut result = one;
    for _ in 0..DEPTH {
        result = push_assignment(
            definition,
            block,
            span,
            MirRvalueKind::Binary {
                operation: MirBinaryOperation::AddI64,
                left: result,
                right: one,
            },
            MirType::I64,
        );
    }
    let MirTerminator::Return { value, .. } = definition.body.blocks[block.index()]
        .terminator
        .as_mut()
        .unwrap()
    else {
        panic!("entry fixture must return");
    };
    *value = Some(result);

    let solution = solve_local_constants((&*definition).into()).unwrap();
    assert_eq!(
        solution.constant(result).unwrap(),
        Some(PrimitiveConstant::I64((DEPTH + 1) as i64))
    );
    let provenance = solution
        .facts()
        .iter()
        .find(|fact| fact.identity() == LocalConstantIdentity::Value(result))
        .unwrap()
        .provenance();
    assert_eq!(provenance.depth(), DEPTH);
    assert_eq!(result.callable(), callable);
}

#[test]
fn fan_out_and_fan_in_dependencies_publish_each_fact_once() {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let definition = entry_definition_mut(&mut program);
    let block = definition.body.entry;
    let span = definition.span;
    let two = push_assignment(
        definition,
        block,
        span,
        MirRvalueKind::ConstantI64(2),
        MirType::I64,
    );
    let three = push_assignment(
        definition,
        block,
        span,
        MirRvalueKind::ConstantI64(3),
        MirType::I64,
    );
    let shared = push_assignment(
        definition,
        block,
        span,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left: two,
            right: three,
        },
        MirType::I64,
    );
    let doubled = push_assignment(
        definition,
        block,
        span,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::MultiplyI64,
            left: shared,
            right: two,
        },
        MirType::I64,
    );
    let tripled = push_assignment(
        definition,
        block,
        span,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::MultiplyI64,
            left: shared,
            right: three,
        },
        MirType::I64,
    );
    let combined = push_assignment(
        definition,
        block,
        span,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left: doubled,
            right: tripled,
        },
        MirType::I64,
    );
    let solution = solve_local_constants((&*definition).into()).unwrap();
    assert_eq!(
        solution.constant(combined).unwrap(),
        Some(PrimitiveConstant::I64(25))
    );
    assert_eq!(
        solution
            .facts()
            .iter()
            .filter(|fact| fact.identity() == LocalConstantIdentity::Value(shared))
            .count(),
        1
    );
}

#[test]
fn unsupported_leaves_and_unseeded_cycles_remain_unknown() {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 1 + 2; }");
    let definition = entry_definition_mut(&mut program);
    let result = returned_value((&*definition).into());
    let assignment = definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) if assignment.result == result => Some(assignment),
            _ => None,
        })
        .unwrap();
    let MirRvalueKind::Binary { left, right, .. } = &mut assignment.rvalue.kind else {
        panic!("fixture return must be binary");
    };
    *left = result;
    *right = result;
    let solution = solve_local_constants((&*definition).into()).unwrap();
    assert_eq!(solution.constant(result).unwrap(), None);

    let unsupported = lower_source_to_final_mir(concat!(
        "fn dynamic() -> i64 { return 7; } ",
        "fn main() -> i64 { return dynamic() + 1; }",
    ));
    let definition = entry_definition(&unsupported);
    assert_eq!(
        solve_local_constants(definition.into())
            .unwrap()
            .constant(returned_value(definition.into()))
            .unwrap(),
        None
    );
}

#[test]
fn invalid_dense_identity_and_result_type_are_structured_failures() {
    let mut invalid_identity = lower_source_to_final_mir("fn main() -> i64 { return 1; }");
    let definition = entry_definition_mut(&mut invalid_identity);
    let expected = definition.values[0].id;
    definition.values[0].id = ValueId::new(
        CallableId::Function(FunctionId::new(usize::MAX)),
        expected.index(),
    );
    assert!(matches!(
        solve_local_constants((&*definition).into()),
        Err(LocalConstantAnalysisError::Rewrite(_))
            | Err(LocalConstantAnalysisError::InvalidValueIdentity { .. })
    ));

    let mut invalid_type = lower_source_to_final_mir("fn main() -> i64 { return 1; }");
    let definition = entry_definition_mut(&mut invalid_type);
    let result = returned_value((&*definition).into());
    definition.values[result.index()].ty = MirType::U64;
    assert!(matches!(
        solve_local_constants((&*definition).into()),
        Err(LocalConstantAnalysisError::DeclaredTypeMismatch { .. })
            | Err(LocalConstantAnalysisError::DerivedTypeMismatch { .. })
    ));
}

fn push_assignment(
    definition: &mut crate::mir::MirFunctionDefinition,
    block: crate::mir::BlockId,
    span: Span,
    kind: MirRvalueKind,
    ty: MirType,
) -> ValueId {
    let result = ValueId::new(definition.callable(), definition.values.len());
    definition.values.push(MirValue {
        id: result,
        ty,
        span,
    });
    definition.body.blocks[block.index()]
        .instructions
        .push(MirInstruction::Assign(MirAssignment {
            result,
            rvalue: MirRvalue { kind, ty },
            span,
        }));
    result
}
