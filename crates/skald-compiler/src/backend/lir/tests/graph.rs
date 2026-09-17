use super::*;
use crate::backend::graph::{check_graph, GraphLocation, GraphReason, GraphSession};
fn valid<'p>(plan: &'p CheckedPlan) -> (CallableDraft<'p>, BlockHandle<'p>, ValueHandle<'p>) {
    let mut b = builder(plan);
    let entry = entry_block(&mut b);
    let v = constant(&mut b, entry, Constant::I64(42));
    b.terminate(entry, Terminator::Return(vec![])).unwrap();
    (b.finish(), entry, v)
}
fn reasons(draft: &CallableDraft<'_>) -> Vec<GraphReason> {
    check_graph(draft)
        .err()
        .expect("invalid draft")
        .iter()
        .map(|e| e.reason)
        .collect()
}
#[test]
fn lowered_analysis_borrows_the_exact_draft_and_handles_nonzero_entry() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut b = builder(&plan);
    let spare = block(&mut b);
    let entry = entry_block(&mut b);
    b.terminate(spare, Terminator::HardTrap).unwrap();
    b.terminate(entry, Terminator::Return(vec![])).unwrap();
    let draft = b.finish();
    let session: GraphSession<'_, CallableDraft<'_>> = check_graph(&draft).unwrap();
    assert!(std::ptr::eq(session.owner(), &draft));
    assert_eq!(session.reachable(entry.id().index()), Some(true));
    assert_eq!(session.reachable(spare.id().index()), Some(false));
}
#[test]
fn malformed_results_and_definition_records_are_not_certified_by_builder_history() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let (mut draft, entry, v) = valid(&plan);
    draft.values.get_mut(v).unwrap().ty = ScalarType::Bool;
    assert!(reasons(&draft).contains(&GraphReason::TypeMismatch));
    draft.values.get_mut(v).unwrap().ty = ScalarType::I64;
    draft.values.get_mut(v).unwrap().definition = None;
    assert!(reasons(&draft).contains(&GraphReason::DefinitionMismatch));
    draft.blocks.get_mut(entry).unwrap().instructions[0]
        .results
        .push(v.id());
    assert_eq!(reasons(&draft), vec![GraphReason::ArityMismatch]);
    draft.blocks.get_mut(entry).unwrap().instructions[0]
        .results
        .pop();
    draft.values.get_mut(v).unwrap().definition = Some(Definition::InstructionResult {
        instruction: InstructionLocation {
            block: entry.id(),
            ordinal: 0,
        },
        ordinal: 0,
    });
    let duplicate = draft.blocks.get(entry).unwrap().instructions[0].clone();
    draft
        .blocks
        .get_mut(entry)
        .unwrap()
        .instructions
        .push(duplicate);
    assert!(reasons(&draft).contains(&GraphReason::DuplicateDefinition));
}
#[test]
fn foreign_value_and_block_ids_fail_before_dense_indexing() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut other = DraftBuilder::new(plan.view().callable(source(1)).unwrap()).unwrap();
    let foreign = entry_block(&mut other);
    let v = constant(&mut other, foreign, Constant::I64(1));
    let (mut draft, entry, _) = valid(&plan);
    draft.blocks.get_mut(entry).unwrap().instructions[0].operation = Operation::Unary {
        operation: UnaryOperation::Negate,
        value: v.id(),
    };
    assert_eq!(reasons(&draft), vec![GraphReason::WrongOwner]);
    draft.blocks.get_mut(entry).unwrap().instructions.clear();
    draft.blocks.get_mut(entry).unwrap().terminator = Some(Terminator::Jump(Edge {
        target: foreign.id(),
        arguments: vec![],
    }));
    assert_eq!(reasons(&draft), vec![GraphReason::WrongOwner]);
}
#[test]
fn dangling_same_owner_ids_are_rejected_without_a_builder_handle() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut other = builder(&plan);
    let spare = block(&mut other);
    let missing = block(&mut other);
    let _ = constant(&mut other, spare, Constant::I64(1));
    let v = constant(&mut other, spare, Constant::I64(2));
    let (mut draft, entry, _) = valid(&plan);
    draft.blocks.get_mut(entry).unwrap().instructions[0].operation = Operation::Unary {
        operation: UnaryOperation::Negate,
        value: v.id(),
    };
    assert_eq!(reasons(&draft), vec![GraphReason::OutOfBounds]);
    draft.blocks.get_mut(entry).unwrap().instructions.clear();
    draft.blocks.get_mut(entry).unwrap().terminator = Some(Terminator::Jump(Edge {
        target: missing.id(),
        arguments: vec![],
    }));
    assert_eq!(reasons(&draft), vec![GraphReason::OutOfBounds]);
}
#[test]
fn lowered_successor_only_and_same_instruction_uses_are_rejected() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut b = builder(&plan);
    let entry = entry_block(&mut b);
    let next = b.reserve_block().unwrap();
    let parameter = b.reserve_value(ScalarType::I64, None).unwrap();
    b.define_block(next, &[parameter]).unwrap();
    b.terminate(
        entry,
        Terminator::Jump(Edge {
            target: next,
            arguments: vec![parameter],
        }),
    )
    .unwrap();
    b.terminate(next, Terminator::Return(vec![])).unwrap();
    assert!(reasons(&b.finish()).contains(&GraphReason::NonDominatingUse));
    let (mut draft, entry, v) = valid(&plan);
    draft.blocks.get_mut(entry).unwrap().instructions[0].operation = Operation::Unary {
        operation: UnaryOperation::Negate,
        value: v.id(),
    };
    let errors = check_graph(&draft).err().unwrap();
    assert_eq!(errors[0].reason, GraphReason::UseBeforeDefinition);
    assert_eq!(
        errors[0].location,
        GraphLocation::Instruction {
            block: 0,
            ordinal: 0,
            operand: 0
        }
    );
}
#[test]
fn stored_call_result_signature_and_missing_terminator_are_checked() {
    use crate::backend::plan::{test_fixtures::runtime_declarations, ArtifactId, RuntimeService};
    let mut supplied = facts();
    let services = runtime_declarations(&mut supplied);
    let plan = CheckedPlan::check(supplied).unwrap();
    let (mut draft, entry, _) = valid(&plan);
    draft.blocks.get_mut(entry).unwrap().instructions[0].operation = Operation::Call(Call {
        target: CallTarget::Direct(ArtifactId::Runtime(RuntimeService::AbiMarker)),
        signature: services[&RuntimeService::AbiMarker],
        arguments: vec![],
        attribution: CallAttribution::HardDefectOnly,
    });
    assert_eq!(reasons(&draft), vec![GraphReason::ArityMismatch]);
    draft.blocks.get_mut(entry).unwrap().instructions.clear();
    draft.blocks.get_mut(entry).unwrap().terminator = None;
    assert!(reasons(&draft).contains(&GraphReason::MissingTerminator));
}

#[test]
fn foreign_live_arena_context_is_rejected_even_with_equal_numeric_ids() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let equal = CheckedPlan::check(facts()).unwrap();
    let (mut draft, _, _) = valid(&plan);
    let (other, _, _) = valid(&equal);
    draft.blocks = other.blocks;
    assert_eq!(reasons(&draft), vec![GraphReason::WrongContext]);
}

#[test]
fn undefined_reservations_and_wrong_entry_input_records_are_rejected() {
    let mut supplied = facts();
    supplied.signatures[0].inputs.push(Component {
        ty: ScalarType::U64,
        role: ComponentRole::Parameter(0),
    });
    let plan = CheckedPlan::check(supplied).unwrap();
    let mut b = builder(&plan);
    let entry = entry_block(&mut b);
    let input = b.inputs().next().unwrap();
    b.terminate(entry, Terminator::Return(vec![])).unwrap();
    let unresolved = b.reserve_value(ScalarType::Bool, None).unwrap();
    b.reserve_block().unwrap();
    let mut draft = b.finish();
    let r = reasons(&draft);
    assert!(r.contains(&GraphReason::UnresolvedValue));
    assert!(r.contains(&GraphReason::UnresolvedBlock));
    assert!(r.contains(&GraphReason::MissingTerminator));
    draft.values.get_mut(input).unwrap().definition = Some(Definition::EntryInput { component: 1 });
    assert!(reasons(&draft).contains(&GraphReason::DefinitionMismatch));
    assert!(draft.value(unresolved).unwrap().definition.is_none());
}
