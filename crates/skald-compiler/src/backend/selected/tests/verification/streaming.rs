use super::*;

fn select<'p>(
    ctx: &'p SelectionContext<'p>,
    body: &lir::VerifiedCallable<'p>,
    views: &[ViewId],
) -> VerifiedSelectedCallable<'p, Node> {
    let (mut builder, entry, _) = begin(ctx, body);
    ret(&mut builder, entry, vec![], vec![], views);
    checked(builder, Shape::Two)
}

#[test]
fn callable_storage_is_released_before_either_program_closes() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let catalog = lir::TargetDeclarations::new(plan.view()).freeze().unwrap();
    let (resources, views, _) = resources();
    let ctx = context(&catalog, resources, vec![]);
    let mut lowered = lir::ProgramBuilder::new(plan.view());
    let mut selected = SelectedProgramBuilder::new(&ctx);
    for key in [source(0), source(1)] {
        lowered.begin(key).unwrap();
        let body = lower(&plan, key);
        lowered.complete(&body, &body.receipt()).unwrap();
        let product = select(&ctx, &body, &views);
        selected.complete(&product, &product.receipt()).unwrap();
        drop(product);
        drop(body);
    }
    let parent = lowered.finish().unwrap();
    let program = selected.finish(&parent).unwrap();
    assert!(std::ptr::eq(program.parent(), &parent));
    for receipt in program.receipts() {
        parent.require_input(receipt.input().unwrap()).unwrap();
    }
    assert_eq!(program.receipts().len(), 2);
}

#[test]
fn discovery_snapshots_cannot_certify_the_executable_pass() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let catalog = lir::TargetDeclarations::new(plan.view()).freeze().unwrap();
    let (resources, views, _) = resources();
    let ctx = context(&catalog, resources, vec![]);
    let mut lowered = lir::ProgramBuilder::new(plan.view());
    let mut selected = SelectedProgramBuilder::new(&ctx);
    for key in [source(0), source(1)] {
        let discovery = lower(&plan, key);
        let product = select(&ctx, &discovery, &views);
        selected.complete(&product, &product.receipt()).unwrap();
        drop(discovery);
        let executable = lower(&plan, key);
        lowered.begin(key).unwrap();
        lowered
            .complete(&executable, &executable.receipt())
            .unwrap();
    }
    let parent = lowered.finish().unwrap();
    assert!(matches!(
        selected.finish(&parent),
        Err(lir::ProgramError::StaleReceipt)
    ));
}

#[test]
fn replacing_a_lower_body_after_local_selection_requires_fresh_derivation() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let catalog = lir::TargetDeclarations::new(plan.view()).freeze().unwrap();
    let (resources, views, _) = resources();
    let ctx = context(&catalog, resources, vec![]);
    let (parent, [body, other]) = inventory(&plan);
    let product = select(&ctx, &body, &views);
    let mut selected = SelectedProgramBuilder::new(&ctx);
    selected.complete(&product, &product.receipt()).unwrap();
    let other = select(&ctx, &other, &views);
    selected.complete(&other, &other.receipt()).unwrap();
    let (mut lowered, editor) = parent.edit(body).unwrap();
    let replacement = editor.finish().unwrap();
    lowered
        .complete(&replacement, &replacement.receipt())
        .unwrap();
    let parent = lowered.finish().unwrap();
    assert!(matches!(
        selected.finish(&parent),
        Err(lir::ProgramError::StaleReceipt)
    ));
}

#[test]
fn selection_rejects_foreign_inputs_and_closure_rejects_foreign_parents() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let foreign = CheckedPlan::check(facts()).unwrap();
    let catalog = lir::TargetDeclarations::new(plan.view()).freeze().unwrap();
    let (resources, views, _) = resources();
    let ctx = context(&catalog, resources, vec![]);
    let foreign_body = lower(&foreign, source(0));
    assert!(matches!(
        SelectedBuilder::<Node>::new(&ctx, source(0), Some(&foreign_body)),
        Err(lir::ProgramError::Plan(plan::PlanError::WrongContext))
    ));
    let (parent, bodies) = inventory(&plan);
    assert!(matches!(
        SelectedBuilder::<Node>::new(&ctx, source(0), Some(&bodies[1])),
        Err(lir::ProgramError::Plan(plan::PlanError::WrongOwner))
    ));
    let mut selected = SelectedProgramBuilder::new(&ctx);
    for body in &bodies {
        let product = select(&ctx, body, &views);
        selected.complete(&product, &product.receipt()).unwrap();
    }
    let (foreign_parent, _) = inventory(&foreign);
    assert!(matches!(
        selected.finish(&foreign_parent),
        Err(lir::ProgramError::Plan(plan::PlanError::WrongContext))
    ));
    assert_eq!(parent.receipts().len(), 2);
}

#[test]
fn selected_edits_cannot_rebind_to_another_parent_publication() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let catalog = lir::TargetDeclarations::new(plan.view()).freeze().unwrap();
    let (resources, views, _) = resources();
    let ctx = context(&catalog, resources, vec![]);
    let (parent, bodies) = inventory(&plan);
    // Even identical chosen callable witnesses do not replace the bound program.
    let mut another = lir::ProgramBuilder::new(plan.view());
    let mut selected = SelectedProgramBuilder::new(&ctx);
    let mut products = vec![];
    for body in &bodies {
        another.begin(body.receipt().owner().key()).unwrap();
        another.complete(body, &body.receipt()).unwrap();
        let product = select(&ctx, body, &views);
        selected.complete(&product, &product.receipt()).unwrap();
        products.push(product);
    }
    let another = another.finish().unwrap();
    let program = selected.finish(&parent).unwrap();
    let (mut selected, editor) = program.edit(products.remove(0)).unwrap();
    let product = editor
        .finish(&WitnessTarget {
            profile: plan.view().profile(),
            shape: Shape::Two,
            reject: false,
        })
        .unwrap();
    selected.complete(&product, &product.receipt()).unwrap();
    assert!(matches!(
        selected.finish(&another),
        Err(lir::ProgramError::Plan(plan::PlanError::WrongContext))
    ));
}

#[test]
fn catalog_thunks_are_required_even_after_all_source_bodies_complete() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let thunk = plan::LirCallableId::TargetThunk(plan::TargetThunkKey {
        family: 1,
        specialization: 0,
    });
    let mut declarations = lir::TargetDeclarations::new(plan.view());
    declarations
        .declare(plan::ArtifactDeclaration {
            key: ArtifactId::Callable(thunk),
            signature: Some(plan.view().callables().next().unwrap().signature),
            layout: None,
        })
        .unwrap();
    let catalog = declarations.freeze().unwrap();
    let (resources, views, _) = resources();
    let ctx = context(&catalog, resources, vec![]);
    let (parent, bodies) = inventory(&plan);
    let mut selected = SelectedProgramBuilder::new(&ctx);
    for body in &bodies {
        let product = select(&ctx, body, &views);
        selected.complete(&product, &product.receipt()).unwrap();
    }
    assert!(
        matches!(selected.finish(&parent), Err(lir::ProgramError::MissingDefinition(key)) if key == ArtifactId::Callable(thunk))
    );
}

#[test]
fn catalog_profile_trace_policy_and_selection_scope_cannot_be_substituted() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let catalog = lir::TargetDeclarations::new(plan.view()).freeze().unwrap();
    let mut other_facts = facts();
    other_facts.profile.architecture = plan::Architecture::Aarch64;
    other_facts.profile.abi = plan::Abi::Aapcs64;
    let other_target = CheckedPlan::check(other_facts).unwrap();
    assert_eq!(
        catalog.require_plan(other_target.view()),
        Err(lir::ProgramError::Plan(plan::PlanError::WrongTarget))
    );
    let mut other_facts = facts();
    other_facts.runtime_trace = crate::backend::RuntimeTracePolicy::Enabled;
    let other_trace = CheckedPlan::check(other_facts).unwrap();
    assert_eq!(
        catalog.require_plan(other_trace.view()),
        Err(lir::ProgramError::Plan(plan::PlanError::WrongContext))
    );

    let (resources, views, _) = resources();
    let ctx = context(&catalog, resources, vec![]);
    let body = lower(&plan, source(0));
    let product = select(&ctx, &body, &views);
    let another_catalog = lir::TargetDeclarations::new(plan.view()).freeze().unwrap();
    let another_ctx = context(&another_catalog, ResourceCatalog::default(), vec![]);
    let mut selected = SelectedProgramBuilder::new(&another_ctx);
    assert_eq!(
        selected.complete(&product, &product.receipt()),
        Err(lir::ProgramError::Plan(plan::PlanError::WrongContext))
    );
}
