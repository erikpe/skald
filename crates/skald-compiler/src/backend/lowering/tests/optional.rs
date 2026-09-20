use crate::{
    backend::{
        lir::{CallTarget, Operation},
        lowering::lower_program,
        plan::{ArtifactId, HelperFamily, LirCallableId},
        planning::plan_program,
        BackendInput,
    },
    test_support::lower_source_to_complete_final_mir_with_sources,
};

#[test]
fn nested_optional_copy_and_cleanup_close_the_lowered_worklist() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "nested-optionals.ska",
        concat!(
            "class Value { marker:i64; init(marker:i64){self.marker=marker;} destroy {} }",
            "fn main()->i64{var deep:i64??=some(some(5));var copy:i64??=deep;copy=deep;",
            "var value:Value??=some(some(Value(7)));var copied:Value??=value;copied=value;",
            "return copy!!+copied!!.marker;}"
        ),
    );
    let planned = plan_program(
        BackendInput::without_runtime_trace(&fixture.mir).with_reachable_artifacts_only(),
    )
    .unwrap();
    let mut source_blocks = 0;
    let mut optional_box_finalizers = 0;
    let program = lower_program(&planned, |body| {
        match body.receipt().owner().key() {
            LirCallableId::Source(_) => source_blocks += body.draft().blocks().count(),
            LirCallableId::Helper(key) if key.family == HelperFamily::OptionalBoxFinalizer => {
                optional_box_finalizers += 1
            }
            _ => {}
        }
        Ok(())
    })
    .unwrap();

    assert!(source_blocks > fixture.mir.executable_definitions().count());
    assert_eq!(optional_box_finalizers, 0);
    assert_eq!(
        program.receipts().count(),
        planned.plan().view().callables().count()
    );
}

#[test]
fn exact_optional_box_finalizer_uses_the_ordinary_recursive_worklist() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "optional-box-finalizer.ska",
        concat!(
            "class Value { init(){} destroy {} }",
            "fn main()->i64{var box:shared Value?=new Value?(Value());return 0;}"
        ),
    );
    let planned = plan_program(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    let mut found = false;
    lower_program(&planned, |body| {
        let LirCallableId::Helper(key) = body.receipt().owner().key() else {
            return Ok(());
        };
        if key.family != HelperFamily::OptionalBoxFinalizer {
            return Ok(());
        }
        found = true;
        assert!(body
            .draft()
            .blocks()
            .flat_map(|(_, block)| &block.instructions)
            .any(|instruction| matches!(
                instruction.operation,
                Operation::Call(crate::backend::lir::Call {
                    target: CallTarget::Direct(ArtifactId::Callable(LirCallableId::Helper(target))),
                    ..
                }) if target.family == HelperFamily::ClassFinalizer
            )));
        Ok(())
    })
    .unwrap();
    assert!(found);
}
