use crate::{
    backend::{
        failure::FailureMessage,
        lir::{CallAttribution, CallTarget, Constant, Operation, Terminator},
        lowering::lower_program,
        plan::{ArtifactId, DataKey, HelperFamily, LirCallableId, RuntimeService},
        planning::admit,
        BackendInput,
    },
    test_support::lower_source_to_complete_final_mir_with_sources,
};

#[test]
fn generated_owner_helpers_cover_boundaries_and_preserve_the_original_allocation() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "owner-helpers.ska",
        concat!(
            "class Leaf { value:i64; init(value:i64){self.value=value;} destroy {} }",
            "class Holder { edge:shared Leaf; init(edge:shared Leaf){self.edge=edge;} }",
            "fn main()->i64{var leaf:shared Leaf=new Leaf(7);",
            "var copy:shared Leaf=leaf;var holder:Holder=Holder(copy);",
            "holder=holder;return holder.edge->value;}",
        ),
    );
    let admitted =
        admit(BackendInput::without_runtime_trace(&fixture.mir).with_reachable_artifacts_only())
            .unwrap();
    let mut retain_seen = false;
    let mut release_seen = false;
    let mut retain_before_release = false;

    let program = lower_program(&admitted, |body| {
        let owner = body.receipt().owner().key();
        if let LirCallableId::Helper(key) = owner {
            match key.family {
                HelperFamily::Retain => {
                    retain_seen = true;
                    let mut constants = Vec::new();
                    let mut overflow = false;
                    let mut hard_trap = false;
                    for (_, block) in body.draft().blocks() {
                        for instruction in &block.instructions {
                            if let Operation::Constant(constant) = instruction.operation {
                                constants.push(constant);
                            }
                            assert!(matches!(
                                instruction.operation,
                                Operation::Call(crate::backend::lir::Call {
                                    attribution: CallAttribution::InheritedOperation { boundary },
                                    ..
                                }) if boundary == owner
                            ) || !matches!(instruction.operation, Operation::Call(_)));
                        }
                        match &block.terminator {
                            Some(Terminator::ReportFailure { reason, .. }) => {
                                overflow |= *reason == FailureMessage::OwnershipCountOverflow
                            }
                            Some(Terminator::HardTrap) => hard_trap = true,
                            _ => {}
                        }
                    }
                    assert!(constants.contains(&Constant::Null(
                        crate::backend::plan::ScalarType::DataAddress
                    )));
                    for boundary in [0, 1, u64::MAX - 1, u64::MAX] {
                        assert!(constants.contains(&Constant::U64(boundary)));
                    }
                    assert!(overflow);
                    assert!(hard_trap);
                    assert_eq!(
                        body.receipt().references(),
                        &[
                            ArtifactId::Callable(owner),
                            ArtifactId::Runtime(RuntimeService::Panic),
                            ArtifactId::Data(DataKey::FailureMessage(
                                FailureMessage::OwnershipCountOverflow,
                            )),
                        ]
                        .into_iter()
                        .collect()
                    );
                }
                HelperFamily::Release => {
                    release_seen = true;
                    let original = body.draft().inputs()[0];
                    let mut finalizer_before_free = false;
                    for (_, block) in body.draft().blocks() {
                        let calls = block
                            .instructions
                            .iter()
                            .filter_map(|instruction| match &instruction.operation {
                                Operation::Call(call) => Some(call),
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        for pair in calls.windows(2) {
                            if matches!(pair[0].target, CallTarget::Indirect(_))
                                && matches!(
                                    pair[1].target,
                                    CallTarget::Direct(ArtifactId::Runtime(RuntimeService::Free))
                                )
                            {
                                assert_eq!(pair[1].arguments[0].value, original);
                                assert_eq!(pair[1].attribution, CallAttribution::HardDefectOnly);
                                finalizer_before_free = true;
                            }
                        }
                    }
                    assert!(finalizer_before_free);
                    assert_eq!(
                        body.receipt().references(),
                        &[
                            ArtifactId::Callable(owner),
                            ArtifactId::Runtime(RuntimeService::Free),
                        ]
                        .into_iter()
                        .collect()
                    );
                }
                _ => {}
            }
        } else if matches!(owner, LirCallableId::Source(_)) {
            let families = body
                .draft()
                .blocks()
                .flat_map(|(_, block)| &block.instructions)
                .filter_map(|instruction| match &instruction.operation {
                    Operation::Call(call) => match call.target {
                        CallTarget::Direct(ArtifactId::Callable(LirCallableId::Helper(key))) => {
                            Some(key.family)
                        }
                        _ => None,
                    },
                    _ => None,
                })
                .collect::<Vec<_>>();
            retain_before_release |= families.windows(2).any(|pair| {
                pair == [HelperFamily::Retain, HelperFamily::Release]
            });
        }
        Ok(())
    })
    .unwrap();

    assert!(retain_seen);
    assert!(release_seen);
    assert!(retain_before_release);
    assert!(program.receipts().any(|(_, receipt)| matches!(
        receipt.owner().key(),
        LirCallableId::Helper(key) if key.family == HelperFamily::ClassFinalizer
    )));
}
