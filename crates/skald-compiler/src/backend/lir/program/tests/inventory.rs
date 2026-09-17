use super::*;

#[test]
fn recursive_helpers_are_reserved_and_completed_without_reentering_a_body() {
    let mut f = facts();
    let [a, b] = helpers(&mut f);
    let plan = CheckedPlan::check(f).unwrap();
    let mut work = ProgramBuilder::new(plan.view());
    work.begin(a).unwrap();
    assert_eq!(work.request(a).unwrap(), InventoryState::Building);
    assert_eq!(error(work.begin(a)), ProgramError::AlreadyBuilding);
    assert_eq!(work.request(b).unwrap(), InventoryState::Declared);
    work.begin(b).unwrap();
    for (key, target) in [(b, a), (a, b)] {
        let body = body(&plan, key, Some(target));
        work.complete(&body, &body.receipt()).unwrap();
        assert_eq!(work.request(key).unwrap(), InventoryState::Verified);
    }
    complete_all(&mut work, &plan);
    let sealed = work.finish().unwrap();
    assert_eq!(sealed.receipts().len(), 4);
}

#[test]
fn canonical_inventory_and_complete_helper_roots_do_not_depend_on_request_arrival() {
    let mut f = facts();
    helpers(&mut f);
    let plan = CheckedPlan::check(f).unwrap();
    let keys = plan.view().callables().map(|c| c.key).collect::<Vec<_>>();
    for order in [
        keys.clone(),
        keys.iter().rev().copied().collect(),
        vec![keys[2], keys[0], keys[3], keys[1]],
    ] {
        let mut work = ProgramBuilder::new(plan.view());
        assert_eq!(work.next(), Some(source(0)));
        for key in order {
            work.begin(key).unwrap();
            let body = body(&plan, key, None);
            work.complete(&body, &body.receipt()).unwrap();
        }
        assert_eq!(work.next(), None);
        assert_eq!(
            work.finish()
                .unwrap()
                .receipts()
                .map(|(key, _)| *key)
                .collect::<Vec<_>>(),
            keys
        );
    }
    let mut missing = ProgramBuilder::new(plan.view());
    for key in [source(0), source(1)] {
        missing.begin(key).unwrap();
        let body = body(&plan, key, None);
        missing.complete(&body, &body.receipt()).unwrap();
    }
    assert_eq!(
        error(missing.finish()),
        ProgramError::MissingDefinition(ArtifactId::Callable(keys[2]))
    );
}

#[test]
fn unfinished_unknown_absent_and_duplicate_definitions_are_rejected() {
    let mut f = facts();
    f.callables[1].body = BodyDisposition::Absent;
    if let LirCallableId::Source(key) = source(1) {
        f.executable_sources.remove(&key);
    }
    let plan = CheckedPlan::check(f).unwrap();
    let mut work = ProgramBuilder::new(plan.view());
    assert_eq!(
        error(work.request(source(1))),
        ProgramError::Plan(PlanError::AbsentBody)
    );
    assert_eq!(
        error(work.request(source(99))),
        ProgramError::Plan(PlanError::UnknownDeclaration)
    );
    let callable = body(&plan, source(0), None);
    assert_eq!(
        error(work.complete(&callable, &callable.receipt())),
        ProgramError::MissingDefinition(ArtifactId::Callable(source(0)))
    );
    work.begin(source(0)).unwrap();
    work.complete(&callable, &callable.receipt()).unwrap();
    assert_eq!(
        error(work.complete(&callable, &callable.receipt())),
        ProgramError::DuplicateDefinition
    );
    assert_eq!(
        error(work.begin(source(0))),
        ProgramError::DuplicateDefinition
    );
    work.finish().unwrap();
    let mut unfinished = ProgramBuilder::new(plan.view());
    unfinished.begin(source(0)).unwrap();
    assert_eq!(
        error(unfinished.finish()),
        ProgramError::MissingDefinition(ArtifactId::Callable(source(0)))
    );
}

#[test]
fn stale_replacement_receipts_and_foreign_live_contexts_do_not_certify_chosen_bodies() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let equal = CheckedPlan::check(facts()).unwrap();
    let old = body(&plan, source(0), None);
    let replacement = body(&plan, source(0), None);
    let foreign = body(&equal, source(0), None);
    let wrong_owner = body(&plan, source(1), None);
    let mut work = ProgramBuilder::new(plan.view());
    work.begin(source(0)).unwrap();
    assert_eq!(
        error(work.complete(&replacement, &old.receipt())),
        ProgramError::StaleReceipt
    );
    assert_eq!(
        error(work.complete(&replacement, &wrong_owner.receipt())),
        ProgramError::StaleReceipt
    );
    assert_eq!(
        error(work.complete(&foreign, &foreign.receipt())),
        ProgramError::Plan(PlanError::WrongContext)
    );
    assert_eq!(
        error(work.complete(&replacement, &foreign.receipt())),
        ProgramError::Plan(PlanError::WrongContext)
    );
    work.complete(&replacement, &replacement.receipt()).unwrap();
    complete_all(&mut work, &plan);
    let sealed = work.finish().unwrap();
    assert_eq!(
        error(sealed.require_input(&old.receipt())),
        ProgramError::StaleReceipt
    );
    assert_eq!(
        error(sealed.require_input(&foreign.receipt())),
        ProgramError::Plan(PlanError::WrongContext)
    );
    sealed.require_input(&replacement.receipt()).unwrap();
    let witness = replacement.receipt();
    drop(replacement);
    sealed.require_input(&witness).unwrap();
}

#[test]
fn snapshot_receipts_reject_foreign_target_profiles_before_completion() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut f = facts();
    f.profile.architecture = crate::backend::plan::Architecture::Aarch64;
    f.profile.abi = crate::backend::plan::Abi::Aapcs64;
    let foreign = CheckedPlan::check(f).unwrap();
    let foreign_body = body(&foreign, source(0), None);
    let mut work = ProgramBuilder::new(plan.view());
    work.begin(source(0)).unwrap();
    assert_eq!(
        error(work.complete(&foreign_body, &foreign_body.receipt())),
        ProgramError::Plan(PlanError::WrongTarget)
    );
}
