use crate::{
    backend::{
        lir::{CallTarget, Operation},
        lowering::lower_program,
        plan::{ArtifactId, Coordinator, DataKey, LirCallableId, StaticStorageDisposition},
        planning::admit,
        BackendInput,
    },
    test_support::lower_source_to_complete_final_mir_with_sources,
};

fn lifecycle_source() -> &'static str {
    concat!(
        "class Item { value:i64; init(value:i64){self.value=value;} destroy{} }",
        "class State {",
        "static zero:i64;static item:Item=Item(5);static maybe:Item?=Item(6);",
        "static owner:shared Item=new Item(7);",
        "static maybe_owner:shared? Item=new Item(8);",
        "static values:i64[]=i64[]{9};init(){} }",
        "fn main()->i64{return State.zero+State.item.value+State.maybe!.value+",
        "State.owner->value+State.maybe_owner!->value+State.values[0]+7;}"
    )
}

#[test]
fn coordinators_close_exact_static_dependencies_and_wrap_the_entry() {
    let fixture =
        lower_source_to_complete_final_mir_with_sources("static-lifecycle.ska", lifecycle_source());
    let admitted = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let resources = admitted.plan().view().resources();
    assert!(!resources.activation.is_empty());
    assert_eq!(resources.activation.len(), resources.shutdown.len());

    let mut entry_calls = Vec::new();
    let mut coordinators = 0;
    let program = lower_program(&admitted, |body| {
        match body.receipt().owner().key() {
            LirCallableId::Entry => {
                for (_, block) in body.draft().blocks() {
                    for instruction in &block.instructions {
                        if let Operation::Call(call) = &instruction.operation {
                            if let CallTarget::Direct(target) = call.target {
                                entry_calls.push(target);
                            }
                        }
                    }
                }
            }
            LirCallableId::Coordinator(_) => coordinators += 1,
            _ => {}
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(coordinators, 2);
    assert_eq!(
        entry_calls,
        vec![
            ArtifactId::Runtime(crate::backend::plan::RuntimeService::AbiMarker),
            ArtifactId::Callable(LirCallableId::Coordinator(Coordinator::Initializer)),
            ArtifactId::Callable(LirCallableId::Source(
                fixture.mir.program().entry_function.into()
            )),
            ArtifactId::Callable(LirCallableId::Coordinator(Coordinator::Finalizer)),
        ]
    );
    assert_eq!(program.data().len(), resources.data.len());
}

#[test]
fn complete_mode_materializes_inactive_static_storage_without_lifecycle_work() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "inactive-static.ska",
        concat!(
            "class State {static live:i64=40;static inactive:i64=2;init(){}}",
            "fn dead()->i64{return State.inactive;}",
            "fn main()->i64{return State.live+2;}"
        ),
    );
    let admitted = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let inactive = admitted
        .plan()
        .view()
        .resources()
        .statics
        .iter()
        .find(|fact| fact.disposition == StaticStorageDisposition::RetainedInactive)
        .unwrap();
    assert!(admitted
        .plan()
        .view()
        .resources()
        .activation
        .iter()
        .all(|fact| fact.field != inactive.field));
    assert!(admitted
        .plan()
        .view()
        .resources()
        .shutdown
        .iter()
        .all(|fact| fact.field != inactive.field));

    let program = lower_program(&admitted, |_| Ok(())).unwrap();
    assert!(program
        .data()
        .any(|definition| definition.key == DataKey::Static(inactive.field)));
}
