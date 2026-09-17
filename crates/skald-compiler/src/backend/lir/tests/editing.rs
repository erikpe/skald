use super::*;
use crate::backend::graph::EditError;
fn inventory<'p>(p: &'p CheckedPlan, body: &VerifiedCallable<'p>) -> VerifiedProgram<'p> {
    let mut inventory = ProgramBuilder::new(p.view());
    inventory.begin(source(0)).unwrap();
    inventory.complete(body, &body.receipt()).unwrap();
    let mut b = DraftBuilder::new(p.view().callable(source(1)).unwrap()).unwrap();
    let entry = entry_block(&mut b);
    b.terminate(entry, Terminator::Return(vec![])).unwrap();
    let other = verify_callable(b.finish()).unwrap();
    inventory.begin(source(1)).unwrap();
    inventory.complete(&other, &other.receipt()).unwrap();
    inventory.finish().unwrap()
}
fn division(p: &CheckedPlan) -> (VerifiedCallable<'_>, BlockHandle<'_>, ValueHandle<'_>) {
    let mut b = builder(p);
    let entry = entry_block(&mut b);
    let dividend = constant(&mut b, entry, Constant::I64(10));
    let divisor = constant(&mut b, entry, Constant::I64(2));
    let replacement = constant(&mut b, entry, Constant::I64(3));
    b.append(
        entry,
        Operation::Divide {
            result: DivisionResult::Quotient,
            dividend,
            divisor,
            evidence: ScalarDomainEvidence::ExactConstant(divisor),
        },
    )
    .unwrap();
    b.terminate(entry, Terminator::Return(vec![])).unwrap();
    {
        let mut draft = b.finish();
        draft.values.get_mut(dividend).unwrap().origin = Some(super::tracing::origin());
        (verify_callable(draft).unwrap(), entry, replacement)
    }
}
#[test]
fn lowered_split_compaction_republishes_and_reconciles_inventory() {
    let p = CheckedPlan::check(facts()).unwrap();
    let (body, entry, _) = division(&p);
    let old_origin = body
        .draft()
        .values()
        .find(|(_, v)| v.origin.is_some())
        .unwrap()
        .0;
    let old = body.receipt();
    let analysis = body.analysis().unwrap();
    assert!(std::ptr::eq(analysis.owner(), body.draft()));
    drop(analysis);
    let inventory = inventory(&p, &body);
    let (mut inventory, mut edit) = inventory.edit(body).unwrap();
    let entry = edit.block(entry.id()).unwrap();
    let suffix = edit.split_block(entry, 2).unwrap();
    let mut values = edit.values().collect::<Vec<_>>();
    values.reverse();
    let objects = edit.objects().collect::<Vec<_>>();
    assert_eq!(edit.blocks().len(), 2);
    let (edit, map): (LoweredEditor<'_>, LoweredRemap<'_>) =
        edit.rebuild(&values, &[suffix, entry], &objects).unwrap();
    map.require_source(&old).unwrap();
    assert_eq!(map.blocks.get(suffix.id()).unwrap().index(), 0);
    let edited = edit.finish().unwrap();
    assert_eq!(
        edited
            .draft()
            .values
            .get_id(map.values.get(old_origin).unwrap())
            .unwrap()
            .origin,
        Some(super::tracing::origin())
    );
    assert!(!old.matches(&edited));
    assert!(map.require_source(&edited.receipt()).is_err());
    assert!(edited.analysis().unwrap().dominates(1, 0).unwrap());
    assert_eq!(
        inventory.complete(&edited, &old),
        Err(ProgramError::StaleReceipt)
    );
    inventory.complete(&edited, &edited.receipt()).unwrap();
    let program = inventory.finish().unwrap();
    program.require_input(&edited.receipt()).unwrap();
    assert_eq!(program.require_input(&old), Err(ProgramError::StaleReceipt));
}
#[test]
fn substituted_divisor_cannot_reuse_constant_evidence() {
    let p = CheckedPlan::check(facts()).unwrap();
    let (body, entry, replacement) = division(&p);
    let inventory = inventory(&p, &body);
    let (_, mut edit) = inventory.edit(body).unwrap();
    let replacement = edit.value(replacement.id()).unwrap();
    edit.replace_operand(
        InstructionLocation {
            block: entry.id(),
            ordinal: 3,
        },
        1,
        replacement,
    )
    .unwrap();
    match edit.finish() {
        Err(LoweredEditFailure::Verify(errors)) => assert!(errors
            .iter()
            .any(|e| e.reason == VerificationReason::GuardProtection)),
        _ => panic!("stale evidence must fail"),
    }
}
#[test]
fn deleted_and_duplicate_remap_ids_fail_before_publication() {
    let p = CheckedPlan::check(facts()).unwrap();
    for duplicate in [false, true] {
        let (body, entry, _) = division(&p);
        let (_, edit) = inventory(&p, &body).edit(body).unwrap();
        let mut values = edit.values().collect::<Vec<_>>();
        if duplicate {
            values.push(values[0]);
        } else {
            values.remove(0);
        }
        let result = edit.rebuild(&values, &[entry], &[]);
        assert!(
            matches!(result,Err(e) if e==if duplicate {EditError::DuplicateId}else{EditError::MissingId})
        );
    }
}
#[test]
fn reachable_formerly_dead_block_rechecks_dominance() {
    let p = CheckedPlan::check(facts()).unwrap();
    let mut b = builder(&p);
    let entry = entry_block(&mut b);
    let producer = block(&mut b);
    let consumer = block(&mut b);
    let value = constant(&mut b, producer, Constant::I64(1));
    b.append(
        consumer,
        Operation::Unary {
            operation: UnaryOperation::Negate,
            value,
        },
    )
    .unwrap();
    b.terminate(entry, Terminator::Jump(empty_edge(producer)))
        .unwrap();
    b.terminate(producer, Terminator::Return(vec![])).unwrap();
    b.terminate(consumer, Terminator::Return(vec![])).unwrap();
    let body = verify_callable(b.finish()).unwrap();
    assert_eq!(
        body.analysis()
            .unwrap()
            .dominates(producer.id().index(), consumer.id().index()),
        None
    );
    let (_, mut edit) = inventory(&p, &body).edit(body).unwrap();
    edit.redirect_edge(
        EdgeOccurrence {
            predecessor: entry.id(),
            slot: 0,
        },
        consumer,
        &[],
    )
    .unwrap();
    assert!(matches!(edit.finish(), Err(LoweredEditFailure::Verify(_))));
}

#[test]
fn splitting_the_guard_relocates_evidence_before_full_verification() {
    let p = super::verification::input_plan();
    let (draft, entry, success, _) = super::verification::guarded(&p);
    let body = verify_callable(draft).unwrap();
    let (_, mut edit) = inventory(&p, &body).edit(body).unwrap();
    let guard = edit.split_block(entry, 0).unwrap();
    match edit.draft().blocks.get(success).unwrap().instructions[0].operation {
        Operation::Divide {
            evidence: ScalarDomainEvidence::SuccessCheck(id),
            ..
        } => assert_eq!(id, guard.id()),
        _ => panic!("missing relocated evidence"),
    }
    let body = edit.finish().unwrap();
    assert!(body
        .analysis()
        .unwrap()
        .dominates(guard.id().index(), success.id().index())
        .unwrap());
}

#[test]
fn guard_operand_substitution_cannot_protect_the_old_divisor() {
    let p = super::verification::input_plan();
    let (draft, guard, _, _) = super::verification::guarded(&p);
    let body = verify_callable(draft).unwrap();
    let other = body.draft().inputs()[1];
    let (_, mut edit) = inventory(&p, &body).edit(body).unwrap();
    let other = edit.value(other).unwrap();
    edit.replace_terminal_operand(guard, 0, other).unwrap();
    assert!(
        matches!(edit.finish(),Err(LoweredEditFailure::Verify(errors)) if errors.iter().any(|e|e.reason==VerificationReason::GuardProtection))
    );
}

#[test]
fn surviving_symbolic_object_references_cannot_map_to_deleted_storage() {
    let p = CheckedPlan::check(facts()).unwrap();
    let mut b = builder(&p);
    let entry = entry_block(&mut b);
    let object = b
        .declare_object(object(
            LayoutDisposition::Addressable,
            8,
            8,
            ObjectRole::SemanticStorage,
            LifetimeDisposition::WholeCallable,
        ))
        .unwrap();
    b.append(entry, Operation::ObjectAddress(object)).unwrap();
    b.terminate(entry, Terminator::Return(vec![])).unwrap();
    let body = verify_callable(b.finish()).unwrap();
    let (_, edit) = inventory(&p, &body).edit(body).unwrap();
    let values = edit.values().collect::<Vec<_>>();
    let blocks = edit.blocks().collect::<Vec<_>>();
    assert!(matches!(
        edit.rebuild(&values, &blocks, &[]),
        Err(EditError::MissingId)
    ));
}
