use super::*;

pub(super) fn assert_dump(body: &VerifiedCallable<'_>) {
    let mut text = String::new();
    body.dump(&mut text).unwrap();
    assert!(text.starts_with("skald-lir schema=1 stage=lowered status=verified callable="));
    assert!(text.contains("profile TargetProfile"));
    assert!(!text.contains("PhantomData"));
    let mut counts = [0; 3];
    body.visit(|fact| {
        match fact {
            LoweredFact::Value(_, _) => counts[0] += 1,
            LoweredFact::Object(_, _) => counts[1] += 1,
            LoweredFact::Block(_, _) => counts[2] += 1,
        }
        Ok::<_, std::convert::Infallible>(())
    })
    .unwrap();
    assert_eq!(
        counts,
        [
            body.draft().values().len(),
            body.draft().objects().len(),
            body.draft().blocks().len()
        ]
    );
    if std::env::var_os("SKALD_LIR_DUMP_CHILD").is_some() {
        println!("LIR-DUMP-BEGIN\n{text}LIR-DUMP-END");
    }
}
fn float_dump(plan: &CheckedPlan) -> String {
    let mut b = builder(plan);
    let entry = entry_block(&mut b);
    for bits in [
        0,
        1 << 63,
        1,
        0x7ff0000000000000,
        0xfff0000000000000,
        0x7ff8000000001234,
        0x7ff0000000001234,
    ] {
        constant(&mut b, entry, Constant::F64(bits));
    }
    b.terminate(entry, Terminator::Return(vec![])).unwrap();
    let body = verify_callable(b.finish()).unwrap();
    assert_dump(&body);
    let mut text = String::new();
    body.dump(&mut text).unwrap();
    text
}
#[test]
fn float_bits_and_distinct_live_contexts_have_identical_text() {
    let first_plan = CheckedPlan::check(facts()).unwrap();
    let equal_plan = CheckedPlan::check(facts()).unwrap();
    let first = float_dump(&first_plan);
    assert_eq!(first, float_dump(&equal_plan));
    for bits in [
        "0000000000000000",
        "8000000000000000",
        "0000000000000001",
        "7ff0000000000000",
        "fff0000000000000",
        "7ff8000000001234",
        "7ff0000000001234",
    ] {
        assert!(first.contains(&format!("F64 bits=0x{bits}")));
    }
}
#[test]
fn malformed_drafts_are_explicitly_unverified_and_do_not_follow_ids() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut b = builder(&plan);
    b.reserve_block().unwrap();
    let draft = b.finish();
    let mut text = String::new();
    draft.dump_draft(&mut text).unwrap();
    assert!(text.contains("status=unverified-draft"));
    assert!(text.contains("entry <unresolved>"));
    assert!(text.contains("parameters=<unresolved>"));
    assert!(text.contains("terminal <unresolved>"));
    assert!(!text.contains("status=verified"));
}
#[test]
fn generated_inventory_order_is_independent_of_construction_order() {
    fn dump(reverse: bool) -> String {
        let mut f = facts();
        let helper = plan_helper(&mut f);
        if reverse {
            f.callables.reverse();
            f.artifacts.reverse();
        }
        let p = CheckedPlan::check(f).unwrap();
        let mut program = ProgramBuilder::new(p.view());
        let mut keys = vec![source(0), source(1), helper];
        if reverse {
            keys.reverse();
        }
        for key in keys {
            let owner = program.begin(key).unwrap();
            let mut b = DraftBuilder::new(owner).unwrap();
            let entry = entry_block(&mut b);
            b.terminate(entry, Terminator::Return(vec![])).unwrap();
            let body = verify_callable(b.finish()).unwrap();
            program.complete(&body, &body.receipt()).unwrap();
        }
        let mut text = String::new();
        program.finish().unwrap().dump_inventory(&mut text).unwrap();
        text
    }
    assert_eq!(dump(false), dump(true));
    fn plan_helper(f: &mut crate::backend::plan::PlanFacts) -> crate::backend::plan::LirCallableId {
        use crate::backend::plan::*;
        let key = LirCallableId::Helper(HelperKey {
            family: HelperFamily::Retain,
            layout: f
                .add_layout(LayoutFact {
                    size: 0,
                    alignment: 1,
                    disposition: LayoutDisposition::Addressable,
                })
                .unwrap(),
            signature: f.callables[0].signature,
        });
        f.callables.push(CallableDeclaration {
            key,
            signature: f.callables[0].signature,
            body: BodyDisposition::Required,
        });
        key
    }
}

#[test]
fn lowered_worked_cases_are_deterministic_in_independent_processes() {
    if std::env::var_os("SKALD_LIR_DUMP_CHILD").is_some() {
        float_dump(&CheckedPlan::check(facts()).unwrap());
        release::shared_release_uses_explicit_memory_branches_finalizer_and_original_header_after_call();
        tracing::enabled_trace_actions_remain_ordered_and_associated_with_explicit_call_locations();
        tracing::reported_failure_and_nonreturning_service_calls_are_explicit_terminals_without_cleanup_edges();
        drafts::loop_parameters_transfer_simultaneously_and_parallel_edges_keep_occurrences();
        return;
    }
    let first = crate::backend::inspection::test_child_dumps("backend::lir::tests::inspection::lowered_worked_cases_are_deterministic_in_independent_processes");
    assert!(first.len() >= 5);
    let combined = first.concat();
    for fact in [
        "F64 bits=",
        "edge slot=1",
        "Trace",
        "ReportFailure",
        "NonReturningCall",
        "Read",
        "Free",
    ] {
        assert!(combined.contains(fact), "missing {fact}");
    }
    assert_eq!(first, crate::backend::inspection::test_child_dumps("backend::lir::tests::inspection::lowered_worked_cases_are_deterministic_in_independent_processes"));
}
