use crate::{
    backend::{
        lir::{InventoryState, Operation, ProgramBuilder},
        lowering::{lower_next, LowerError},
        plan::LirCallableId,
        planning::admit,
        BackendInput,
    },
    test_support::lower_source_to_complete_final_mir_with_sources,
};

#[test]
fn source_loop_publishes_with_semantic_memory_and_exact_worklist_receipt() {
    let fixture = lower_source_to_complete_final_mir_with_sources("scalar.ska", "fn main() -> i64 { var sum: i64 = 0; var n: i64 = 0; while (n < 3) { sum = sum + n; n = n + 1; } return sum; }");
    let admitted = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let mut worklist = ProgramBuilder::new(admitted.plan().view());
    let body = lower_next(&admitted, &mut worklist).unwrap().unwrap();
    let draft = body.draft();
    assert_eq!(
        draft.blocks().len(),
        fixture
            .mir
            .program()
            .definitions
            .iter()
            .next()
            .unwrap()
            .body
            .blocks
            .len()
    );
    assert_eq!(draft.objects().len(), 2);
    assert!(draft
        .blocks()
        .flat_map(|(_, b)| &b.instructions)
        .any(|i| matches!(i.operation, Operation::Store { .. })));
    assert_eq!(
        worklist.request(body.receipt().owner().key()).unwrap(),
        InventoryState::Verified
    );
    assert!(body.receipt().matches(&body));
    assert!(body.analysis().is_ok());
    assert_eq!(worklist.next(), Some(LirCallableId::Entry));
}

#[test]
fn primitive_predicates_keep_their_semantic_types_and_parameter_memory() {
    use crate::backend::plan::ScalarType;
    let mut source = String::from("fn main() -> i64 { return 0; }");
    for ty in ["i64", "u64", "u8", "f64"] {
        for (index, predicate) in ["==", "!=", "<", "<=", ">", ">="].iter().enumerate() {
            source.push_str(&format!(
                "fn cmp_{ty}_{index}(a: {ty}, b: {ty}) -> bool {{ return a {predicate} b; }}"
            ));
        }
    }
    source.push_str("fn arithmetic(a: i64, b: i64) -> i64 { return ~(-a) + (a * b - b) & (a | b ^ a); } fn floating(a: f64, b: f64) -> f64 { return -a + a * b - a / b; } fn boolean(a: bool) -> bool { return !a; }");
    let fixture = lower_source_to_complete_final_mir_with_sources("predicates.ska", &source);
    let admitted = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let mut worklist = ProgramBuilder::new(admitted.plan().view());
    let mut comparisons = vec![];
    let mut bodies = 0;
    while worklist.next() != Some(LirCallableId::Entry) {
        let body = lower_next(&admitted, &mut worklist).unwrap().unwrap();
        let draft = body.draft();
        bodies += 1;
        for (_, block) in draft.blocks() {
            assert!(block.parameters.as_ref().unwrap().is_empty());
            for instruction in &block.instructions {
                if let Operation::Compare {
                    predicate, left, ..
                } = instruction.operation
                {
                    comparisons.push((
                        draft.values().find(|(id, _)| *id == left).unwrap().1.ty,
                        predicate,
                    ));
                }
            }
        }
        if draft.inputs().len() == 2 {
            assert_eq!(draft.objects().len(), 2);
            assert!(draft.objects().all(|(_, object)| object.origin.is_some()));
            let entry = draft
                .blocks()
                .find(|(id, _)| Some(*id) == draft.entry())
                .unwrap()
                .1;
            assert_eq!(
                entry
                    .instructions
                    .iter()
                    .filter(|i| matches!(i.operation, Operation::Store { .. }))
                    .count(),
                2
            );
        }
    }
    assert_eq!(bodies, 28);
    for ty in [
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::U8,
        ScalarType::F64,
    ] {
        assert_eq!(
            comparisons
                .iter()
                .filter(|(actual, _)| *actual == ty)
                .count(),
            6
        );
    }
    use crate::primitive_comparison::PrimitiveComparisonPredicate::*;
    for predicate in [
        Equal,
        NotEqual,
        LessThan,
        LessEqual,
        GreaterThan,
        GreaterEqual,
    ] {
        assert_eq!(
            comparisons
                .iter()
                .filter(|(_, actual)| *actual == predicate)
                .count(),
            4
        );
    }
    assert_eq!(comparisons.len(), 24);
}

#[test]
fn sparse_retention_does_not_resurrect_removed_bodies() {
    use crate::mir::retain::{prepare_reachable_definition_retention, MirDefinitionRetention};
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "sparse.ska",
        "fn dead() -> i64 { return 4; } fn main() -> i64 { return 0; }",
    );
    let MirDefinitionRetention::Changed(retention) =
        prepare_reachable_definition_retention(fixture.mir.program(), fixture.mir.reachability())
            .unwrap()
    else {
        panic!("expected sparse retention")
    };
    let sparse =
        crate::passes::verify_final_mir(retention.apply(fixture.mir.program().clone()).program)
            .unwrap();
    let admitted = admit(BackendInput::without_runtime_trace(&sparse)).unwrap();
    let mut worklist = ProgramBuilder::new(admitted.plan().view());
    let body = lower_next(&admitted, &mut worklist).unwrap().unwrap();
    let source = sparse.program().definitions.iter().next().unwrap();
    assert_ne!(source.function.index(), 0);
    assert_eq!(
        body.receipt().owner().key(),
        LirCallableId::Source(crate::identity::CallableId::Function(source.function))
    );
    assert_eq!(worklist.next(), Some(LirCallableId::Entry));
    assert!(
        worklist.finish().is_err(),
        "per-body receipts cannot fake complete program closure"
    );
}

#[test]
fn final_publication_rejects_undefined_return_values_and_unfinished_blocks() {
    use crate::backend::{
        lir::{verify_callable, DraftBuilder, Terminator},
        plan::ScalarType,
    };
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "malformed.ska",
        "fn main() -> i64 { return 0; }",
    );
    let admitted = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let worklist = ProgramBuilder::new(admitted.plan().view());
    let owner = admitted
        .plan()
        .view()
        .callable(worklist.next().unwrap())
        .unwrap();
    for terminate in [false, true] {
        let mut builder = DraftBuilder::new(owner).unwrap();
        let entry = builder.reserve_block().unwrap();
        builder.define_block(entry, &[]).unwrap();
        builder.set_entry(entry).unwrap();
        if terminate {
            let missing = builder.reserve_value(ScalarType::I64, None).unwrap();
            builder
                .terminate(entry, Terminator::Return(vec![missing]))
                .unwrap();
        }
        assert!(verify_callable(builder.finish()).is_err());
    }
}

#[test]
fn duplicate_edges_and_unreachable_retained_blocks_survive_publication() {
    use crate::mir::*;
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "edges.ska",
        "fn main() -> i64 { return 0; }",
    );
    let mut program = fixture.mir.program().clone();
    let function = program.definitions.iter().next().unwrap().function;
    let definition = program.definitions.get_mut_for_test(function).unwrap();
    let span = definition.span;
    let condition = ValueId::new(function, definition.values.len());
    definition.values.push(MirValue {
        id: condition,
        ty: MirType::Bool,
        span,
    });
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::Assign(MirAssignment {
            result: condition,
            rvalue: MirRvalue {
                kind: MirRvalueKind::ConstantBool(true),
                ty: MirType::Bool,
            },
            span,
        }));
    let target = BlockId::new(function, 1);
    definition.body.blocks[0].terminator = Some(MirTerminator::Branch {
        condition,
        true_target: target,
        false_target: target,
        span,
    });
    for index in 1..=2 {
        let value = ValueId::new(function, definition.values.len());
        definition.values.push(MirValue {
            id: value,
            ty: MirType::I64,
            span,
        });
        definition.body.blocks.push(MirBasicBlock {
            id: BlockId::new(function, index),
            span,
            instructions: vec![MirInstruction::Assign(MirAssignment {
                result: value,
                rvalue: MirRvalue {
                    kind: MirRvalueKind::ConstantI64(index as i64),
                    ty: MirType::I64,
                },
                span,
            })],
            terminator: Some(MirTerminator::Return {
                value: Some(value),
                span,
            }),
        });
    }
    let sealed = crate::passes::verify_final_mir(program).unwrap();
    let admitted = admit(BackendInput::without_runtime_trace(&sealed)).unwrap();
    let mut worklist = ProgramBuilder::new(admitted.plan().view());
    let body = lower_next(&admitted, &mut worklist).unwrap().unwrap();
    let graph = body.analysis().unwrap();
    assert_eq!(graph.successors(0), Some([1, 1].as_slice()));
    assert_eq!(graph.predecessors(1), Some([(0, 0), (0, 1)].as_slice()));
    assert_eq!(graph.reachable(2), Some(false));
    assert_eq!(body.draft().blocks().len(), 3);
    assert!(body
        .draft()
        .values()
        .all(|(_, value)| value.origin.is_some()));
}

#[test]
fn callable_addresses_are_canonical_and_float_constants_keep_raw_bits() {
    use crate::{
        backend::{
            lir::Constant,
            plan::{ArtifactId, ScalarType},
        },
        mir::MirRvalueKind,
    };
    let fixture = lower_source_to_complete_final_mir_with_sources("addresses.ska", "fn target(a: i64) -> i64 { return a; } fn choose() -> fn(i64) -> i64 { return target; } fn floating() -> f64 { return 1.5; } fn main() -> i64 { var b: u8 = 2u8; var flag: bool = true; return 0; }");
    let mut program = fixture.mir.program().clone();
    let function = program.definitions.iter().find(|d| d.body.blocks.iter().any(|b| b.instructions.iter().any(|i| matches!(i, crate::mir::MirInstruction::Assign(a) if matches!(a.rvalue.kind, MirRvalueKind::ConstantF64Bits(_)))))).unwrap().function;
    let bits = 0x7ff8_0000_0000_0042;
    for block in &mut program
        .definitions
        .get_mut_for_test(function)
        .unwrap()
        .body
        .blocks
    {
        for instruction in &mut block.instructions {
            if let crate::mir::MirInstruction::Assign(assign) = instruction {
                if matches!(assign.rvalue.kind, MirRvalueKind::ConstantF64Bits(_)) {
                    assign.rvalue.kind = MirRvalueKind::ConstantF64Bits(bits);
                }
            }
        }
    }
    let sealed = crate::passes::verify_final_mir(program).unwrap();
    let admitted = admit(BackendInput::without_runtime_trace(&sealed)).unwrap();
    let mut worklist = ProgramBuilder::new(admitted.plan().view());
    let mut address = false;
    let mut float = false;
    let mut byte_accesses = 0;
    while worklist.next() != Some(LirCallableId::Entry) {
        let body = lower_next(&admitted, &mut worklist).unwrap().unwrap();
        for (_, block) in body.draft().blocks() {
            for instruction in &block.instructions {
                match instruction.operation {
                    Operation::SymbolAddress {
                        symbol: ArtifactId::Callable(target),
                        ty: ScalarType::CodeAddress(signature),
                    } => {
                        assert_eq!(
                            admitted
                                .plan()
                                .view()
                                .callables()
                                .find(|c| c.key == target)
                                .unwrap()
                                .signature,
                            signature
                        );
                        assert!(body
                            .receipt()
                            .references()
                            .contains(&ArtifactId::Callable(target)));
                        address = true;
                    }
                    Operation::Constant(Constant::F64(actual)) => {
                        assert_eq!(actual, bits);
                        float = true;
                    }
                    Operation::Store { representation, .. }
                        if matches!(representation.scalar, ScalarType::U8 | ScalarType::Bool) =>
                    {
                        assert_eq!((representation.bytes, representation.alignment), (1, 1));
                        byte_accesses += 1;
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(address && float);
    assert_eq!(byte_accesses, 2);
}

#[test]
fn foreign_plans_cannot_publish_and_enabled_tracing_publishes() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "context.ska",
        "fn main() -> i64 { return 0; }",
    );
    let admitted = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let other = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let mut foreign = ProgramBuilder::new(other.plan().view());
    assert!(matches!(
        lower_next(&admitted, &mut foreign),
        Err(LowerError::Plan(_))
    ));
    assert!(foreign.finish().is_err());
    let tracing = admit(BackendInput::with_runtime_trace(
        &fixture.mir,
        &fixture.sources,
    ))
    .unwrap();
    let mut worklist = ProgramBuilder::new(tracing.plan().view());
    let body = lower_next(&tracing, &mut worklist).unwrap().unwrap();
    assert_eq!(
        worklist.request(body.receipt().owner().key()).unwrap(),
        InventoryState::Verified
    );
    assert!(body.draft().trace_plan().is_some());
}
