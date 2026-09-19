use crate::{
    backend::{
        failure::FailureMessage,
        lir::{Operation, Terminator},
        lowering::lower_program,
        plan::{HelperFamily, LirCallableId},
        planning::admit,
        BackendInput,
    },
    test_support::lower_source_to_complete_final_mir_with_sources,
};

#[test]
fn primitive_array_storage_positions_and_loops_lower_as_checked_lir() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "primitive-arrays.ska",
        concat!(
            "fn sum()->u64{var values:u64[]=u64[](3u);",
            "values[0]=4u;values[-1]=8u;var total:u64=0u;",
            "for(value in values){total=total+value;}",
            "return total+values.len();}",
            "fn main()->i64{if(sum()==15u){return 0;}return 1;}"
        ),
    );
    let admitted =
        admit(BackendInput::without_runtime_trace(&fixture.mir).with_reachable_artifacts_only())
            .unwrap();
    let mut addressed_element = false;
    let mut checked_control_flow = false;
    let mut corrupt_count_traps = false;
    let program = lower_program(&admitted, |body| {
        for (_, block) in body.draft().blocks() {
            addressed_element |= block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction.operation, Operation::ByteOffset { .. }));
            checked_control_flow |= matches!(block.terminator, Some(Terminator::Branch { .. }));
            corrupt_count_traps |= matches!(block.terminator, Some(Terminator::HardTrap));
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(
        program.receipts().count(),
        admitted.plan().view().callables().count()
    );
    assert!(addressed_element && checked_control_flow && corrupt_count_traps);
}

#[test]
fn shared_and_optional_shared_primitive_arrays_close_the_owner_worklist() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "shared-primitive-arrays.ska",
        concat!(
            "fn main()->i64{",
            "var owner:shared i64[]=new i64[]{4,8};",
            "var copy:shared i64[]=owner;",
            "var maybe:shared? i64[]=copy;",
            "if(maybe is none){return 1;}",
            "var recovered:shared i64[]=maybe!;",
            "return recovered->[0]+recovered->[1];}"
        ),
    );
    let admitted =
        admit(BackendInput::without_runtime_trace(&fixture.mir).with_reachable_artifacts_only())
            .unwrap();
    let program = lower_program(&admitted, |_| Ok(())).unwrap();
    assert_eq!(
        program.receipts().count(),
        admitted.plan().view().callables().count()
    );
    let families = program
        .receipts()
        .filter_map(|(_, receipt)| match receipt.owner().key() {
            LirCallableId::Helper(key) => Some(key.family),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    for family in [
        HelperFamily::ArrayElementInitializer,
        HelperFamily::ArrayElementCopier,
        HelperFamily::ArrayClone,
        HelperFamily::ArrayElementDestroyer,
        HelperFamily::ArrayRelease,
        HelperFamily::ArraySharedFinalizer,
    ] {
        assert!(families.contains(&family));
    }
}

#[test]
fn empty_and_oversized_arrays_retain_checked_failure_control_flow() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "array-boundaries.ska",
        concat!(
            "fn build(length:u64)->u64{var values:i64[]=i64[](length);",
            "return values.len();}",
            "fn main()->i64{var ignored:u64=build(18446744073709551615u);return 0;}"
        ),
    );
    let admitted =
        admit(BackendInput::without_runtime_trace(&fixture.mir).with_reachable_artifacts_only())
            .unwrap();
    let mut allocation_failure = false;
    lower_program(&admitted, |body| {
        allocation_failure |= body.draft().blocks().any(|(_, block)| {
            matches!(
                block.terminator,
                Some(Terminator::ReportFailure {
                    reason: FailureMessage::ArrayAllocationFailure,
                    ..
                })
            )
        });
        Ok(())
    })
    .unwrap();
    assert!(allocation_failure);
}

#[test]
fn nontrivial_element_lifecycle_closes_generated_array_and_class_helpers() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "nontrivial-array-lifecycle.ska",
        concat!(
            "class Item{value:i64;init(){self.value=0;}",
            "copy(ref other:Item){self.value=other.value;}",
            "assign(ref other:Item){self.value=other.value;}destroy{}}",
            "fn main()->i64{var source:Item[]=Item[](2u);source[0].value=7;",
            "var copied:Item[]=source;copied[1]=source[0];",
            "var optional:Item?[]=Item?[](2u);optional[0]=Item();",
            "var optional_copy:Item?[]=optional;",
            "var nested:Item[][]=Item[][](2u);nested[0]=source;",
            "var nested_copy:Item[][]=nested;return copied[1].value;}"
        ),
    );
    for input in [
        BackendInput::without_runtime_trace(&fixture.mir),
        BackendInput::without_runtime_trace(&fixture.mir).with_reachable_artifacts_only(),
    ] {
        let admitted =
            admit(input).expect("all concrete inline array lifecycle cells are admitted");
        let program = lower_program(&admitted, |_| Ok(())).unwrap();
        assert_eq!(
            program.receipts().count(),
            admitted.plan().view().callables().count()
        );
        assert!(program.receipts().any(|(_, receipt)| matches!(
            receipt.owner().key(),
            LirCallableId::Helper(key) if key.family == HelperFamily::RawClassCopy
        )));
    }
}

#[test]
fn shared_and_optional_shared_elements_use_checked_retain_release_helpers() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "owned-array-lifecycle.ska",
        concat!(
            "class Item{value:i64;init(value:i64){self.value=value;}destroy{}}",
            "fn main()->i64{var item:shared Item=new Item(7);",
            "var values:(shared Item)[]=(shared Item)[]{item,item};",
            "var copy:(shared Item)[]=values;",
            "var optional:(shared? Item)[]=(shared? Item)[]{none,item};",
            "var optional_copy:(shared? Item)[]=optional;return copy[0]->value;}"
        ),
    );
    let admitted =
        admit(BackendInput::without_runtime_trace(&fixture.mir).with_reachable_artifacts_only())
            .expect("shared array lifecycle is admitted");
    let program = lower_program(&admitted, |_| Ok(())).unwrap();
    let families = program
        .receipts()
        .filter_map(|(_, receipt)| match receipt.owner().key() {
            LirCallableId::Helper(key) => Some(key.family),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert!(families.contains(&HelperFamily::Retain));
    assert!(families.contains(&HelperFamily::Release));
}

#[test]
fn optional_array_elements_close_recursive_clone_and_release_edges() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "optional-array-elements.ska",
        concat!(
            "fn main()->i64{var row:i64[]=i64[]{4,8};",
            "var maybe:i64[]?=row;var values:i64[]?[]=i64[]?[]{none,maybe};",
            "var copied:i64[]?[]=values;",
            "if(copied[1] is none){return 1;}return copied[1]![0];}"
        ),
    );
    let admitted =
        admit(BackendInput::without_runtime_trace(&fixture.mir).with_reachable_artifacts_only())
            .expect("inline-array optional lifecycle is admitted");
    let program = lower_program(&admitted, |_| Ok(())).unwrap();
    assert_eq!(
        program.receipts().count(),
        admitted.plan().view().callables().count()
    );
}
