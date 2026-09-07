use crate::{
    identity::{CallableId, FunctionId},
    mir::{
        BlockId, MirAssignment, MirBasicBlock, MirInstruction, MirPrimitiveCast,
        MirPrimitiveCastKind, MirPrimitiveType, MirRvalue, MirRvalueKind, MirTerminator, MirType,
        MirValue, ValueId,
    },
    test_support::lower_source_to_final_mir,
};

use super::*;

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

fn append_i64_constant(definition: &mut crate::mir::MirFunctionDefinition, value: i64) -> ValueId {
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
        .push(MirInstruction::Assign(MirAssignment {
            result,
            rvalue: MirRvalue {
                kind: MirRvalueKind::ConstantI64(value),
                ty: MirType::I64,
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

fn analysis(program: &crate::mir::MirProgram, function: FunctionId) -> IntegerCastChainAnalysis {
    analyze_integer_cast_chains(program.definitions.get(function).unwrap().into()).unwrap()
}

#[test]
fn arbitrary_depth_chain_records_exact_sites_root_recipe_and_reusable_narrowing() {
    let (mut program, root, function) = definition_with_root(MirPrimitiveType::I64);
    let definition = program.definitions.get_mut_for_test(function).unwrap();
    let first = append_cast(
        definition,
        root,
        MirPrimitiveType::I64,
        MirPrimitiveType::U64,
    );
    let narrowed = append_cast(
        definition,
        first,
        MirPrimitiveType::U64,
        MirPrimitiveType::U8,
    );
    let widened = append_cast(
        definition,
        narrowed,
        MirPrimitiveType::U8,
        MirPrimitiveType::I64,
    );
    let renarrowed = append_cast(
        definition,
        widened,
        MirPrimitiveType::I64,
        MirPrimitiveType::U8,
    );
    let endpoint = append_cast(
        definition,
        renarrowed,
        MirPrimitiveType::U8,
        MirPrimitiveType::U64,
    );
    let span = definition.span;
    set_return(definition, endpoint);

    let analysis = analysis(&program, function);
    assert_eq!(analysis.candidates().count(), 4);
    let entry = analysis.entry_for_result(endpoint).unwrap();
    let chain = entry.chain().unwrap();
    assert_eq!(chain.root(), root);
    assert_eq!(chain.original_length(), 5);
    assert_eq!(chain.canonical_length(), 2);
    assert_eq!(chain.eliminated_steps(), 3);
    assert!(chain.is_candidate());
    assert_eq!(chain.reusable_narrowing(), Some(narrowed));
    assert_eq!(chain.boundary(), None);
    assert_eq!(chain.endpoint().result(), endpoint);
    assert_eq!(chain.endpoint().result_type(), MirType::U64);
    assert_eq!(chain.endpoint().span(), span);
    assert!(matches!(
        chain.endpoint().expected(),
        MirInstruction::Assign(assignment) if assignment.result == endpoint
    ));
    assert_eq!(
        analysis
            .sites(chain)
            .map(IntegerCastSite::result)
            .collect::<Vec<_>>(),
        vec![endpoint, renarrowed, widened, narrowed, first]
    );
}

#[test]
fn every_root_and_result_type_pair_gets_its_shortest_preserving_recipe() {
    let types = [
        MirPrimitiveType::I64,
        MirPrimitiveType::U64,
        MirPrimitiveType::U8,
    ];
    for source in types {
        for target in types {
            let (mut program, root, function) = definition_with_root(source);
            let definition = program.definitions.get_mut_for_test(function).unwrap();
            let middle = if source == MirPrimitiveType::U8 {
                MirPrimitiveType::I64
            } else if source == MirPrimitiveType::I64 {
                MirPrimitiveType::U64
            } else {
                MirPrimitiveType::I64
            };
            let first = append_cast(definition, root, source, middle);
            let second = append_cast(definition, first, middle, source);
            let endpoint = append_cast(definition, second, source, target);
            set_return(definition, endpoint);

            let analysis = analysis(&program, function);
            let chain = analysis
                .entry_for_result(endpoint)
                .unwrap()
                .chain()
                .unwrap();
            assert_eq!(chain.original_length(), 3, "{source:?} -> {target:?}");
            assert_eq!(
                chain.canonical_length(),
                usize::from(source != target),
                "{source:?} -> {target:?}"
            );
            assert!(chain.is_candidate());
            assert_eq!(chain.reusable_narrowing(), None);
        }
    }
}

#[test]
fn unrelated_instructions_and_shared_intermediate_uses_do_not_break_provenance() {
    let (mut program, root, function) = definition_with_root(MirPrimitiveType::U8);
    let definition = program.definitions.get_mut_for_test(function).unwrap();
    let first = append_cast(
        definition,
        root,
        MirPrimitiveType::U8,
        MirPrimitiveType::I64,
    );
    append_i64_constant(definition, 17);
    let endpoint = append_cast(
        definition,
        first,
        MirPrimitiveType::I64,
        MirPrimitiveType::U64,
    );
    let shared_use = append_cast(
        definition,
        first,
        MirPrimitiveType::I64,
        MirPrimitiveType::I64,
    );
    set_return(definition, endpoint);

    let analysis = analysis(&program, function);
    let endpoint_chain = analysis
        .entry_for_result(endpoint)
        .unwrap()
        .chain()
        .unwrap();
    assert_eq!(endpoint_chain.original_length(), 2);
    assert_eq!(endpoint_chain.canonical_length(), 1);
    assert!(endpoint_chain.is_candidate());
    assert!(analysis
        .entry_for_result(shared_use)
        .unwrap()
        .chain()
        .unwrap()
        .is_candidate());
}

#[test]
fn unsupported_cast_families_and_non_cast_definitions_bound_integer_suffixes() {
    let cases = [
        (MirPrimitiveType::Bool, MirPrimitiveType::I64),
        (MirPrimitiveType::F64, MirPrimitiveType::I64),
    ];
    for (root_type, integer_type) in cases {
        let (mut program, root, function) = definition_with_root(root_type);
        let definition = program.definitions.get_mut_for_test(function).unwrap();
        let boundary = append_cast(definition, root, root_type, integer_type);
        let endpoint = append_cast(definition, boundary, integer_type, MirPrimitiveType::U64);
        set_return(definition, endpoint);

        let analysis = analysis(&program, function);
        let entry = analysis.entry_for_result(endpoint).unwrap();
        assert_eq!(
            entry.boundary(),
            Some(IntegerCastChainBoundary::UnsupportedCastFamily)
        );
        let chain = entry.chain().unwrap();
        assert_eq!(chain.root(), boundary);
        assert_eq!(chain.original_length(), 1);
    }

    let (mut program, root, function) = definition_with_root(MirPrimitiveType::I64);
    let definition = program.definitions.get_mut_for_test(function).unwrap();
    let constant = append_i64_constant(definition, 11);
    let endpoint = append_cast(
        definition,
        constant,
        MirPrimitiveType::I64,
        MirPrimitiveType::U64,
    );
    set_return(definition, endpoint);
    let analysis = analysis(&program, function);
    let entry = analysis.entry_for_result(endpoint).unwrap();
    assert_eq!(entry.boundary(), None);
    assert_eq!(entry.chain().unwrap().root(), constant);
    assert_ne!(root, constant);
}

#[test]
fn raw_bit_and_checked_results_are_not_absorbed_into_integer_chains() {
    let (mut program, root, function) = definition_with_root(MirPrimitiveType::U64);
    let definition = program.definitions.get_mut_for_test(function).unwrap();
    let bits_as_float = append_cast(
        definition,
        root,
        MirPrimitiveType::U64,
        MirPrimitiveType::F64,
    );
    let boundary_result = append_cast(
        definition,
        bits_as_float,
        MirPrimitiveType::F64,
        MirPrimitiveType::U64,
    );
    for result in [bits_as_float, boundary_result] {
        let assignment = definition.body.blocks[0]
            .instructions
            .iter_mut()
            .find_map(|instruction| match instruction {
                MirInstruction::Assign(assignment) if assignment.result == result => {
                    Some(assignment)
                }
                _ => None,
            })
            .unwrap();
        let MirRvalueKind::PrimitiveCast { operation, .. } = &mut assignment.rvalue.kind else {
            unreachable!()
        };
        *operation = MirPrimitiveCast::bit_reinterpretation(operation.source, operation.target);
    }
    let endpoint = append_cast(
        definition,
        boundary_result,
        MirPrimitiveType::U64,
        MirPrimitiveType::I64,
    );
    set_return(definition, endpoint);

    let raw_bit_analysis = analysis(&program, function);
    assert_eq!(
        raw_bit_analysis
            .entry_for_result(endpoint)
            .unwrap()
            .boundary(),
        Some(IntegerCastChainBoundary::UnsupportedCastFamily)
    );

    let source = "fn checked(value: f64) -> u64 { return (u64) (i64) value; }\n\
                  fn main() -> i64 { return 0; }";
    let program = lower_source_to_final_mir(source);
    let checked_analysis = analysis(&program, FunctionId::new(0));
    let integer_entry = checked_analysis
        .entries()
        .iter()
        .find(|entry| entry.site().operation().source == MirPrimitiveType::I64)
        .unwrap();
    assert_eq!(integer_entry.boundary(), None);
    assert_eq!(integer_entry.chain().unwrap().original_length(), 1);
}

#[test]
fn control_flow_boundary_starts_a_new_same_block_suffix() {
    let (mut program, root, function) = definition_with_root(MirPrimitiveType::I64);
    let definition = program.definitions.get_mut_for_test(function).unwrap();
    let first = append_cast(
        definition,
        root,
        MirPrimitiveType::I64,
        MirPrimitiveType::U64,
    );
    let endpoint = append_cast(
        definition,
        first,
        MirPrimitiveType::U64,
        MirPrimitiveType::I64,
    );
    let second_instruction = definition.body.blocks[0].instructions.pop().unwrap();
    let old_terminator = definition.body.blocks[0].terminator.take().unwrap();
    let second_block = BlockId::new(CallableId::Function(function), 1);
    definition.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: second_block,
        span: definition.span,
    });
    definition.body.blocks.push(MirBasicBlock {
        id: second_block,
        instructions: vec![second_instruction],
        terminator: Some(old_terminator),
        span: definition.span,
    });

    let analysis = analysis(&program, function);
    let entry = analysis.entry_for_result(endpoint).unwrap();
    assert_eq!(
        entry.boundary(),
        Some(IntegerCastChainBoundary::ControlFlow)
    );
    let chain = entry.chain().unwrap();
    assert_eq!(chain.root(), first);
    assert_eq!(chain.original_length(), 1);
    assert!(!chain.is_candidate());
}

#[test]
fn malformed_later_and_cyclic_provenance_have_explicit_outcomes() {
    let (mut malformed, root, function) = definition_with_root(MirPrimitiveType::I64);
    let definition = malformed.definitions.get_mut_for_test(function).unwrap();
    let malformed_result = append_cast(
        definition,
        root,
        MirPrimitiveType::I64,
        MirPrimitiveType::U64,
    );
    let MirInstruction::Assign(assignment) = definition.body.blocks[0]
        .instructions
        .iter_mut()
        .find(|instruction| {
            matches!(instruction, MirInstruction::Assign(value) if value.result == malformed_result)
        })
        .unwrap()
    else {
        unreachable!()
    };
    let MirRvalueKind::PrimitiveCast { operation, .. } = &mut assignment.rvalue.kind else {
        unreachable!()
    };
    operation.set_kind_for_test(MirPrimitiveCastKind::ToBool);
    let malformed_analysis = analysis(&malformed, function);
    assert_eq!(
        malformed_analysis
            .entry_for_result(malformed_result)
            .unwrap()
            .invalidities(),
        &[IntegerCastSiteInvalidity::UnsupportedTypeOrOperation]
    );

    let (mut later, root, function) = definition_with_root(MirPrimitiveType::U64);
    let definition = later.definitions.get_mut_for_test(function).unwrap();
    let early = append_cast(
        definition,
        root,
        MirPrimitiveType::I64,
        MirPrimitiveType::U64,
    );
    let late = append_cast(
        definition,
        root,
        MirPrimitiveType::U64,
        MirPrimitiveType::I64,
    );
    let MirInstruction::Assign(assignment) = definition.body.blocks[0]
        .instructions
        .iter_mut()
        .find(|instruction| matches!(instruction, MirInstruction::Assign(value) if value.result == early))
        .unwrap()
    else {
        unreachable!()
    };
    let MirRvalueKind::PrimitiveCast { operand, .. } = &mut assignment.rvalue.kind else {
        unreachable!()
    };
    *operand = late;
    let later_analysis = analysis(&later, function);
    assert_eq!(
        later_analysis.entry_for_result(early).unwrap().boundary(),
        Some(IntegerCastChainBoundary::NonPrecedingDefinition)
    );

    let (mut cyclic, root, function) = definition_with_root(MirPrimitiveType::I64);
    let definition = cyclic.definitions.get_mut_for_test(function).unwrap();
    let first = append_cast(
        definition,
        root,
        MirPrimitiveType::I64,
        MirPrimitiveType::U64,
    );
    let second = append_cast(
        definition,
        first,
        MirPrimitiveType::U64,
        MirPrimitiveType::I64,
    );
    let MirInstruction::Assign(assignment) = definition.body.blocks[0]
        .instructions
        .iter_mut()
        .find(|instruction| matches!(instruction, MirInstruction::Assign(value) if value.result == first))
        .unwrap()
    else {
        unreachable!()
    };
    let MirRvalueKind::PrimitiveCast { operand, .. } = &mut assignment.rvalue.kind else {
        unreachable!()
    };
    *operand = second;
    let analysis = analysis(&cyclic, function);
    for result in [first, second] {
        assert_eq!(
            analysis.entry_for_result(result).unwrap().boundary(),
            Some(IntegerCastChainBoundary::RepeatedIdentity)
        );
    }
}

#[test]
fn duplicate_cast_definitions_are_structured_failures() {
    let (mut program, root, function) = definition_with_root(MirPrimitiveType::I64);
    let definition = program.definitions.get_mut_for_test(function).unwrap();
    let first = append_cast(
        definition,
        root,
        MirPrimitiveType::I64,
        MirPrimitiveType::U64,
    );
    let second = append_cast(
        definition,
        first,
        MirPrimitiveType::U64,
        MirPrimitiveType::I64,
    );
    let MirInstruction::Assign(assignment) = definition.body.blocks[0]
        .instructions
        .iter_mut()
        .find(|instruction| matches!(instruction, MirInstruction::Assign(value) if value.result == second))
        .unwrap()
    else {
        unreachable!()
    };
    assignment.result = first;

    assert!(matches!(
        analyze_integer_cast_chains(program.definitions.get(function).unwrap().into()),
        Err(MirRewriteError::DuplicateValueDefinition { value, .. }) if value == first
    ));
}

#[test]
fn analysis_is_deterministic_and_long_chains_do_not_use_recursive_discovery() {
    const DEPTH: usize = 16_384;

    let (mut program, root, function) = definition_with_root(MirPrimitiveType::I64);
    let definition = program.definitions.get_mut_for_test(function).unwrap();
    let mut current = root;
    let mut current_type = MirPrimitiveType::I64;
    for index in 0..DEPTH {
        let target = match index % 3 {
            0 => MirPrimitiveType::U64,
            1 => MirPrimitiveType::U8,
            _ => MirPrimitiveType::I64,
        };
        current = append_cast(definition, current, current_type, target);
        current_type = target;
    }
    set_return(definition, current);

    let first = analysis(&program, function);
    let second = analysis(&program, function);
    assert_eq!(first, second);
    let chain = first.entry_for_result(current).unwrap().chain().unwrap();
    assert_eq!(first.entries().len(), DEPTH);
    assert_eq!(chain.original_length(), DEPTH);
    assert_eq!(first.sites(chain).len(), DEPTH);
    assert_eq!(first.sites(chain).next().unwrap().result(), current);
    assert!(chain.is_candidate());
}
