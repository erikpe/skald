use crate::{
    backend::{
        lir::{CallAttribution, CallTarget, Operation},
        lowering::lower_program,
        plan::{ArtifactId, HelperFamily, LirCallableId},
        planning::plan_program,
        BackendInput,
    },
    identity::CallableId,
    test_support::lower_source_to_complete_final_mir_with_sources,
};

#[test]
fn generated_finalizers_preserve_body_field_base_order_and_close_the_worklist() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "finalizers.ska",
        concat!(
            "class Base { init(){} destroy {} }",
            "class Field { init(){} destroy {} }",
            "class Owner extends Base { child:Field; ",
            "init(){super();self.child=Field();} destroy {} }",
            "fn main()->i64{var owner:Owner=Owner();return 7;}",
        ),
    );
    let planned = plan_program(
        BackendInput::without_runtime_trace(&fixture.mir).with_reachable_artifacts_only(),
    )
    .unwrap();
    let mut finalizers = 0;
    let mut ordered_owner = false;
    let program = lower_program(&planned, |body| {
        let LirCallableId::Helper(key) = body.receipt().owner().key() else {
            return Ok(());
        };
        if key.family != HelperFamily::ClassFinalizer {
            return Ok(());
        }
        finalizers += 1;
        let calls = body
            .draft()
            .blocks()
            .flat_map(|(_, block)| &block.instructions)
            .filter_map(|instruction| match &instruction.operation {
                Operation::Call(call) => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();
        for call in &calls {
            let expected = if matches!(
                call.target,
                CallTarget::Direct(ArtifactId::Callable(LirCallableId::Source(_)))
            ) {
                CallAttribution::SourceBodyFromOmittedHelper {
                    boundary: LirCallableId::Helper(key),
                }
            } else {
                CallAttribution::InheritedOperation {
                    boundary: LirCallableId::Helper(key),
                }
            };
            assert_eq!(call.attribution, expected);
        }
        if calls.len() == 3 {
            assert!(matches!(
                calls[0].target,
                CallTarget::Direct(ArtifactId::Callable(LirCallableId::Source(
                    CallableId::Destructor(_)
                )))
            ));
            assert!(calls[1..].iter().all(|call| matches!(
                call.target,
                CallTarget::Direct(ArtifactId::Callable(LirCallableId::Helper(target)))
                    if target.family == HelperFamily::ClassFinalizer
            )));
            ordered_owner = true;
        }
        Ok(())
    })
    .unwrap();

    assert_eq!(finalizers, 3);
    assert!(ordered_owner);
    assert_eq!(
        program
            .receipts()
            .filter(|(_, receipt)| matches!(
                receipt.owner().key(),
                LirCallableId::Helper(key) if key.family == HelperFamily::ClassFinalizer
            ))
            .count(),
        3
    );
}
