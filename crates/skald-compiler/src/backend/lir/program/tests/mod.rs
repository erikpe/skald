use crate::backend::lir::{
    verify_callable, Call, CallAttribution, CallTarget, DataDefinition, DataInitializer,
    DraftBuilder, InventoryState, Operation, ProgramBuilder, ProgramError, TargetCatalog,
    TargetDeclarations, Terminator, VerifiedCallable, VerifiedProgram,
};
use crate::backend::plan::{
    test_fixtures::{facts, runtime_declarations, source},
    ArtifactCategory, ArtifactDeclaration, ArtifactId, BodyDisposition, CallableDeclaration,
    CheckedPlan, Convention, DataKey, DispatchSlot, HelperFamily, HelperKey, LirCallableId,
    PlanError, PlanFacts, TargetThunkKey,
};

fn body(
    plan: &CheckedPlan,
    key: LirCallableId,
    target: Option<LirCallableId>,
) -> VerifiedCallable<'_> {
    let mut builder = DraftBuilder::new(plan.view().callable(key).unwrap()).unwrap();
    let entry = builder.reserve_block().unwrap();
    builder.define_block(entry, &[]).unwrap();
    builder.set_entry(entry).unwrap();
    if let Some(target) = target {
        builder
            .append(
                entry,
                Operation::Call(Call {
                    target: CallTarget::Direct(ArtifactId::Callable(target)),
                    signature: plan
                        .view()
                        .callables()
                        .find(|c| c.key == target)
                        .unwrap()
                        .signature,
                    arguments: vec![],
                    attribution: CallAttribution::ProcessBoundary,
                }),
            )
            .unwrap();
    }
    builder
        .terminate(entry, Terminator::Return(vec![]))
        .unwrap();
    verify_callable(builder.finish()).unwrap()
}
fn complete_all<'p>(builder: &mut ProgramBuilder<'p>, plan: &'p CheckedPlan) {
    while let Some(key) = builder.next() {
        builder.begin(key).unwrap();
        let body = body(plan, key, None);
        builder.complete(&body, &body.receipt()).unwrap();
    }
}
fn program(plan: &CheckedPlan) -> VerifiedProgram<'_> {
    let mut builder = ProgramBuilder::new(plan.view());
    complete_all(&mut builder, plan);
    builder.finish().unwrap()
}
fn helpers(facts: &mut PlanFacts) -> [LirCallableId; 2] {
    let signature = facts.callables[0].signature;
    let layout = facts.add_layout(facts.layouts[0]).unwrap();
    [HelperFamily::Retain, HelperFamily::Release].map(|family| {
        let key = LirCallableId::Helper(HelperKey {
            family,
            layout,
            signature,
        });
        facts.callables.push(CallableDeclaration {
            key,
            signature,
            body: BodyDisposition::Required,
        });
        key
    })
}
fn error<T>(result: Result<T, ProgramError>) -> ProgramError {
    result.err().expect("must reject")
}

fn data_plan() -> CheckedPlan {
    let mut f = facts();
    let layout = f.add_layout(f.layouts[0]).unwrap();
    for key in [DataKey::Table(0), DataKey::Table(1)] {
        f.artifacts.push(ArtifactDeclaration {
            key: ArtifactId::Data(key),
            signature: None,
            layout: Some(layout),
        });
    }
    CheckedPlan::check(f).unwrap()
}
fn definition(key: DataKey, initializer: DataInitializer) -> DataDefinition {
    DataDefinition {
        key,
        initializers: vec![initializer],
    }
}

fn thunk() -> ArtifactId {
    ArtifactId::Callable(LirCallableId::TargetThunk(TargetThunkKey {
        family: 0,
        specialization: 0,
    }))
}

mod data;
mod inventory;
mod target;
