use super::*;

#[test]
fn entry_inputs_and_forward_results_have_exact_single_definition_sites() {
    let mut supplied = facts();
    supplied.signatures[0].inputs = vec![Component {
        ty: ScalarType::I64,
        role: ComponentRole::Parameter(7),
    }];
    supplied.signatures[0].results = vec![Component {
        ty: ScalarType::I64,
        role: ComponentRole::Result,
    }];
    supplied.signatures[0].returns = ReturnShape::Scalar(ScalarType::I64);
    let plan = CheckedPlan::check(supplied).unwrap();
    let mut builder = builder(&plan);
    let spare = builder.reserve_block().unwrap();
    let entry = entry_block(&mut builder);
    assert_eq!(entry.id().index(), 1);
    let input = builder.inputs().next().unwrap();
    let result = builder.reserve_value(ScalarType::I64, None).unwrap();
    builder
        .append_into(
            entry,
            Operation::Unary {
                operation: UnaryOperation::Negate,
                value: input,
            },
            &[result],
        )
        .unwrap();
    assert_eq!(
        err(builder.append_into(entry, Operation::Constant(Constant::I64(3)), &[result])),
        BuildError::DuplicateDefinition
    );
    builder
        .terminate(entry, Terminator::Return(vec![result]))
        .unwrap();
    builder.define_block(spare, &[]).unwrap();
    builder.terminate(spare, Terminator::HardTrap).unwrap();
    let draft = builder.finish();
    crate::backend::graph::check_graph(&draft).unwrap();
    assert_eq!(draft.owner().key(), source(0));
    assert_eq!(draft.entry(), Some(entry.id()));
    assert_eq!(draft.inputs(), &[input.id()]);
    assert_eq!(
        draft.value(input).unwrap().definition,
        Some(Definition::EntryInput { component: 0 })
    );
    assert_eq!(
        draft.value(result).unwrap().definition,
        Some(Definition::InstructionResult {
            instruction: InstructionLocation {
                block: entry.id(),
                ordinal: 0
            },
            ordinal: 0
        })
    );
    assert_eq!(
        draft.block(entry).unwrap().instructions[0].results,
        [result.id()]
    );
    assert_eq!(draft.values().len(), 2);
    assert_eq!(draft.blocks().len(), 2);
}

#[test]
fn reservations_and_rejected_writes_leave_existing_definitions_intact() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut builder = builder(&plan);
    let reserved = builder.reserve_block().unwrap();
    assert_eq!(
        err(builder.append(reserved, Operation::Constant(Constant::Bool(true)))),
        BuildError::UndefinedBlock
    );
    let first = builder.reserve_value(ScalarType::I64, None).unwrap();
    let second = builder.reserve_value(ScalarType::Bool, None).unwrap();
    assert_eq!(
        err(builder.define_block(reserved, &[first, first])),
        BuildError::DuplicateDefinition
    );
    builder.define_block(reserved, &[first, second]).unwrap();
    assert_eq!(
        err(builder.define_block(reserved, &[])),
        BuildError::DuplicateDefinition
    );
    assert_eq!(err(builder.set_entry(reserved)), BuildError::InvalidEntry);
    let entry = entry_block(&mut builder);
    assert_eq!(err(builder.set_entry(entry)), BuildError::InvalidEntry);
    let output = builder.reserve_value(ScalarType::Bool, None).unwrap();
    assert_eq!(
        err(builder.append_into(entry, Operation::Constant(Constant::I64(1)), &[output])),
        BuildError::ResultMismatch
    );
    assert_eq!(
        err(builder.append_into(entry, Operation::Constant(Constant::I64(1)), &[])),
        BuildError::ResultMismatch
    );
    builder
        .append_into(entry, Operation::Constant(Constant::Bool(true)), &[output])
        .unwrap();
    assert_eq!(
        err(builder.terminate(entry, Terminator::Jump(empty_edge(entry)))),
        BuildError::InvalidEntry
    );
    assert_eq!(
        err(builder.terminate(entry, Terminator::Return(vec![output]))),
        BuildError::InvalidReturn
    );
    builder
        .terminate(entry, Terminator::Return(vec![]))
        .unwrap();
    assert_eq!(
        err(builder.terminate(entry, Terminator::HardTrap)),
        BuildError::AlreadyTerminated
    );
    assert_eq!(
        err(builder.append(entry, Operation::Constant(Constant::Bool(false)))),
        BuildError::AlreadyTerminated
    );
    let draft = builder.finish();
    assert_eq!(draft.block(entry).unwrap().instructions.len(), 1);
    assert!(draft.value(output).unwrap().definition.is_some());
    assert!(draft.value(first).unwrap().origin.is_none());
}

#[test]
pub(super) fn loop_parameters_transfer_simultaneously_and_parallel_edges_keep_occurrences() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    let header = builder.reserve_block().unwrap();
    let a = builder.reserve_value(ScalarType::U64, None).unwrap();
    let b = builder.reserve_value(ScalarType::U64, None).unwrap();
    let one = constant(&mut builder, entry, Constant::U64(1));
    let two = constant(&mut builder, entry, Constant::U64(2));
    let condition = constant(&mut builder, entry, Constant::Bool(true));
    // Both edges precede definition of their forward target.
    builder
        .terminate(
            entry,
            Terminator::Branch {
                condition,
                true_edge: Edge {
                    target: header,
                    arguments: vec![one, two],
                },
                false_edge: Edge {
                    target: header,
                    arguments: vec![two, one],
                },
            },
        )
        .unwrap();
    builder.define_block(header, &[a, b]).unwrap();
    builder
        .terminate(
            header,
            Terminator::Jump(Edge {
                target: header,
                arguments: vec![b, a],
            }),
        )
        .unwrap();
    let draft = builder.finish();
    crate::backend::graph::check_graph(&draft).unwrap();
    let terminator = draft.block(entry).unwrap().terminator.as_ref().unwrap();
    let edges = terminator.edges(entry.id()).collect::<Vec<_>>();
    assert_eq!(edges.len(), 2);
    assert_ne!(edges[0].0, edges[1].0);
    assert_eq!(
        edges[0].0,
        EdgeOccurrence {
            predecessor: entry.id(),
            slot: 0
        }
    );
    assert_eq!(edges[1].0.slot, 1);
    assert_eq!(edges[0].1.target, edges[1].1.target);
    assert_eq!(edges[0].1.arguments, [one.id(), two.id()]);
    assert_eq!(edges[1].1.arguments, [two.id(), one.id()]);
    assert_eq!(
        draft.value(a).unwrap().definition,
        Some(Definition::BlockParameter {
            block: header.id(),
            ordinal: 0
        })
    );
    assert_eq!(
        draft.block(header).unwrap().parameters.as_ref().unwrap(),
        &[a.id(), b.id()]
    );
    match draft.block(header).unwrap().terminator.as_ref().unwrap() {
        Terminator::Jump(edge) => assert_eq!(edge.arguments, [b.id(), a.id()]),
        _ => panic!("expected back edge"),
    }
    inspection::assert_dump(&verify_callable(draft).unwrap());
}

#[test]
fn diamond_edges_match_defined_parameters_and_return_components() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    let left = block(&mut builder);
    let right = block(&mut builder);
    let merge = builder.reserve_block().unwrap();
    let parameter = builder.reserve_value(ScalarType::U8, None).unwrap();
    builder.define_block(merge, &[parameter]).unwrap();
    let condition = constant(&mut builder, entry, Constant::Bool(false));
    builder
        .terminate(
            entry,
            Terminator::Branch {
                condition,
                true_edge: empty_edge(left),
                false_edge: empty_edge(right),
            },
        )
        .unwrap();
    let byte = constant(&mut builder, left, Constant::U8(255));
    assert_eq!(
        err(builder.terminate(left, Terminator::Jump(empty_edge(merge)))),
        BuildError::EdgeMismatch
    );
    let wrong = constant(&mut builder, left, Constant::U64(255));
    assert_eq!(
        err(builder.terminate(
            left,
            Terminator::Jump(Edge {
                target: merge,
                arguments: vec![wrong]
            })
        )),
        BuildError::EdgeMismatch
    );
    builder
        .terminate(
            left,
            Terminator::Jump(Edge {
                target: merge,
                arguments: vec![byte],
            }),
        )
        .unwrap();
    let byte = constant(&mut builder, right, Constant::U8(0));
    builder
        .terminate(
            right,
            Terminator::Jump(Edge {
                target: merge,
                arguments: vec![byte],
            }),
        )
        .unwrap();
    builder
        .terminate(merge, Terminator::Return(vec![]))
        .unwrap();
    let draft = builder.finish();
    crate::backend::graph::check_graph(&draft).unwrap();
    assert_eq!(draft.blocks().len(), 4);
}

#[test]
fn builder_rejects_foreign_live_contexts_and_callable_owners() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let equal_plan = CheckedPlan::check(facts()).unwrap();
    let mut local = builder(&plan);
    let entry = entry_block(&mut local);
    let mut foreign = builder(&equal_plan);
    let foreign_entry = entry_block(&mut foreign);
    let foreign_value = constant(&mut foreign, foreign_entry, Constant::I64(42));
    assert_eq!(
        err(local.append(
            entry,
            Operation::Unary {
                operation: UnaryOperation::Negate,
                value: foreign_value
            }
        )),
        BuildError::Plan(PlanError::WrongContext)
    );
    assert_eq!(
        err(local.terminate(entry, Terminator::Jump(empty_edge(foreign_entry)))),
        BuildError::Plan(PlanError::WrongContext)
    );
    let mut other = DraftBuilder::new(plan.view().callable(source(1)).unwrap()).unwrap();
    let other_entry = entry_block(&mut other);
    let other_value = constant(&mut other, other_entry, Constant::I64(42));
    assert_eq!(
        err(local.append(
            entry,
            Operation::Unary {
                operation: UnaryOperation::Negate,
                value: other_value
            }
        )),
        BuildError::Plan(PlanError::WrongOwner)
    );
    assert_eq!(local.finish().values().len(), 0);
}

#[test]
fn owner_private_fixtures_can_challenge_verification_without_a_builder_success_path() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    let reserved = builder.reserve_value(ScalarType::Bool, None).unwrap();
    let mut draft: CallableDraft<'_> = builder.finish();
    // Owner-private storage deliberately permits malformed verifier fixtures;
    // no public constructor or unchecked verified product is exposed.
    draft
        .blocks
        .get_mut(entry)
        .unwrap()
        .instructions
        .push(Instruction {
            operation: Operation::Constant(Constant::I64(42)),
            results: vec![reserved.id(), reserved.id()],
            effects: crate::backend::effects::Effects::default(),
        });
    draft.values.get_mut(reserved).unwrap().definition = Some(Definition::InstructionResult {
        instruction: InstructionLocation {
            block: entry.id(),
            ordinal: usize::MAX,
        },
        ordinal: 0,
    });
    let value: &Value = draft.value(reserved).unwrap();
    assert_eq!(value.ty, ScalarType::Bool);
    assert_eq!(
        draft.block(entry).unwrap().instructions[0].results,
        [reserved.id(), reserved.id()]
    );
    assert!(draft.block(entry).unwrap().terminator.is_none());
}

#[test]
fn profile_capabilities_and_foreign_target_operands_fail_before_mutation() {
    use crate::backend::plan::{Abi, Architecture};
    let mut no_float = facts();
    no_float.profile.capabilities.binary64 = false;
    let plan = CheckedPlan::check(no_float).unwrap();
    let mut local = builder(&plan);
    let entry = entry_block(&mut local);
    assert_eq!(
        err(local.reserve_value(ScalarType::F64, None)),
        BuildError::Plan(PlanError::UnsupportedCapability)
    );
    assert_eq!(
        err(local.append(entry, Operation::Constant(Constant::F64(0)))),
        BuildError::Plan(PlanError::UnsupportedCapability)
    );
    let mut aarch = facts();
    aarch.profile.architecture = Architecture::Aarch64;
    aarch.profile.abi = Abi::Aapcs64;
    let other_plan = CheckedPlan::check(aarch).unwrap();
    let mut other = builder(&other_plan);
    let other_entry = entry_block(&mut other);
    let other_value = constant(&mut other, other_entry, Constant::I64(1));
    assert_eq!(
        err(local.append(
            entry,
            Operation::Unary {
                operation: UnaryOperation::Negate,
                value: other_value
            }
        )),
        BuildError::Plan(PlanError::WrongTarget)
    );
    let other_object = other
        .declare_object(object(
            LayoutDisposition::Addressable,
            8,
            8,
            ObjectRole::SemanticStorage,
            LifetimeDisposition::WholeCallable,
        ))
        .unwrap();
    assert_eq!(
        err(local.append(entry, Operation::ObjectAddress(other_object))),
        BuildError::Plan(PlanError::WrongTarget)
    );
    let mut no_indirect = facts();
    no_indirect.profile.capabilities.indirect_calls = false;
    let signature = no_indirect.callables[0].signature;
    let no_indirect_plan = CheckedPlan::check(no_indirect).unwrap();
    let mut no_indirect_builder = builder(&no_indirect_plan);
    assert_eq!(
        err(no_indirect_builder.reserve_value(ScalarType::CodeAddress(signature), None)),
        BuildError::Plan(PlanError::UnsupportedCapability)
    );
    let draft = local.finish();
    assert_eq!(draft.values().len(), 0);
    assert!(draft.block(entry).unwrap().instructions.is_empty());
}

#[test]
fn reserved_value_and_object_origins_preserve_source_metadata_without_lookup() {
    use crate::source::{SourceDatabase, Span};
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    let mut sources = SourceDatabase::new();
    let source = sources.add("fixture.ska", "");
    let origin = Span::empty(source, 42);
    let value = builder.reserve_value(ScalarType::U8, Some(origin)).unwrap();
    builder
        .append_into(entry, Operation::Constant(Constant::U8(255)), &[value])
        .unwrap();
    let mut declaration = object(
        LayoutDisposition::Addressable,
        1,
        1,
        ObjectRole::SemanticStorage,
        LifetimeDisposition::WholeCallable,
    );
    declaration.origin = Some(origin);
    let object = builder.declare_object(declaration).unwrap();
    let draft = builder.finish();
    assert_eq!(draft.value(value).unwrap().origin, Some(origin));
    assert_eq!(draft.object(object).unwrap().origin, Some(origin));
}
