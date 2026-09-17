use super::*;
use std::fmt::{self, Write};
impl InspectPayload for Node {
    fn fmt_opcode(&self, out: &mut dyn Write) -> fmt::Result {
        // Structural operands are printed independently; preserve opcode distinctions.
        let opcode = match &self.op {
            Op::Load { .. } => "load",
            Op::Address { .. } => "address",
            Op::Divide { .. } => "divide-fixed",
            Op::Add {
                destructive: true, ..
            } => "add-destructive",
            Op::Add {
                destructive: false, ..
            } => "add-three-address",
            Op::Call { .. } => "call",
            Op::Branch { .. } => "flag-branch",
            Op::Jump => "jump",
            Op::Return { .. } => "return",
            Op::Trap => "trap",
            Op::Trace => "trace",
        };
        out.write_str(opcode)
    }
}
pub(super) fn assert_dump(body: &VerifiedSelectedCallable<'_, Node>) {
    let mut text = String::new();
    body.dump(&mut text).unwrap();
    assert!(text.starts_with("skald-lir schema=1 stage=selected status=verified callable="));
    assert!(text.contains("profile TargetProfile"));
    assert!(text.contains("resource ViewId"));
    assert!(!text.contains("PhantomData"));
    let mut terminals = 0;
    let mut resources = 0;
    let mut entries = 0;
    let mut origins = 0;
    body.visit(|fact| {
        match fact {
            SelectedFact::Terminal { .. } => terminals += 1,
            SelectedFact::Resource(_, _) => resources += 1,
            SelectedFact::Entry { entry, inputs, .. } => {
                entries += 1;
                assert_eq!(entry, body.draft().entry);
                assert_eq!(inputs, body.draft().inputs);
            }
            SelectedFact::Origins {
                values,
                blocks,
                objects,
            } => {
                origins += 1;
                assert_eq!(values, &body.draft().origins);
                assert_eq!(blocks, &body.draft().block_origins);
                assert_eq!(objects, &body.draft().object_origins);
            }
            _ => {}
        }
        Ok::<_, std::convert::Infallible>(())
    })
    .unwrap();
    assert_eq!(terminals, body.draft().blocks.iter().len());
    assert_eq!(resources, body.draft().context.resources.views().len());
    assert_eq!(entries, 1);
    assert_eq!(origins, 1);
    if std::env::var_os("SKALD_LIR_DUMP_CHILD").is_some() {
        println!("LIR-DUMP-BEGIN\n{text}LIR-DUMP-END");
    }
}
#[test]
fn malformed_selected_drafts_and_writer_failures_are_visible() {
    let p = CheckedPlan::check(supplied(Shape::Two)).unwrap();
    let (program, bodies) = inventory(&p);
    let extension = lir::TargetDeclarations::new(program.parent())
        .freeze()
        .unwrap();
    let (r, views, _) = resources();
    let ctx = context(&extension, r, vec![repr(); 2]);
    let (b, _, _) = begin(&ctx, &bodies[0]);
    let draft = b.finish();
    let mut text = String::new();
    draft.dump_draft(&mut text).unwrap();
    assert!(text.contains("status=unverified-draft"));
    assert!(text.contains("terminal <unresolved>"));
    struct FailingWriter;
    impl Write for FailingWriter {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }
    assert!(draft.dump_draft(&mut FailingWriter).is_err());
    let (mut b, entry, _) = begin(&ctx, &bodies[0]);
    ret(&mut b, entry, vec![], vec![], &views);
    let body = checked(b, Shape::Two);
    assert!(body.dump(&mut FailingWriter).is_err());
    let mut builder = SelectedProgramBuilder::new(&ctx);
    builder.complete(&body, &body.receipt()).unwrap();
    let (mut b, entry, args) = begin(&ctx, &bodies[1]);
    let binding = AbiBinding {
        component: p
            .view()
            .signature(p.view().signature_id(1).unwrap())
            .unwrap()
            .results[0],
        representation: repr(),
        location: AbiLocation::Slot {
            area: AbiArea::Results,
            index: 0,
        },
    };
    ret(
        &mut b,
        entry,
        vec![vf(args[0], repr())],
        vec![binding],
        &views,
    );
    let body = checked(b, Shape::Two);
    builder.complete(&body, &body.receipt()).unwrap();
    let mut text = String::new();
    builder
        .finish(&program)
        .unwrap()
        .dump_inventory(&mut text)
        .unwrap();
    assert!(text.contains("stage=selected-inventory status=verified"));
    assert!(text.contains("lower-input=Some(Source"));
}
#[test]
fn worked_case_dumps_are_deterministic_in_independent_processes() {
    const CHILD: &str = "SKALD_LIR_DUMP_CHILD";
    if std::env::var_os(CHILD).is_some() {
        witnesses::destructive_and_three_address_targets_preserve_live_input_flow();
        witnesses::division_selection_exposes_guard_correction_blocks_and_join_arguments();
        witnesses::loop_swaps_parallel_successors_and_critical_edges_remain_simultaneous();
        witnesses::hidden_destination_receiver_and_mixed_banks_keep_exact_components();
        witnesses::release_uses_original_header_after_finalizer_and_rejects_omitted_trace();
        witnesses::resource_extensions_and_thunk_receipts_are_context_and_snapshot_bound();
        tracing::enabled_trace_dependencies_and_inherited_helper_attribution_survive_publication();
        return;
    }
    fn child() -> Vec<String> {
        crate::backend::inspection::test_child_dumps("backend::selected::tests::verification::inspection::worked_case_dumps_are_deterministic_in_independent_processes")
    }
    let first = child();
    assert!(first.len() >= 6);
    let combined = first.concat();
    for required in [
        "add-destructive",
        "add-three-address",
        "divide-fixed",
        "edge",
        "Fixed",
        "Float",
        "TraceState",
        "InheritedOperation",
        "TargetThunk",
    ] {
        assert!(combined.contains(required), "missing {required}");
    }
    assert_eq!(first, child());
}

#[test]
fn equivalent_live_selection_scopes_have_identical_dumps() {
    let p = CheckedPlan::check(supplied(Shape::Two)).unwrap();
    let (program, bodies) = inventory(&p);
    let extension = lir::TargetDeclarations::new(program.parent())
        .freeze()
        .unwrap();
    let (r, first_views, _) = resources();
    let first_ctx = context(&extension, r, vec![repr(); 2]);
    let (r, equal_views, _) = resources();
    let equal_ctx = context(&extension, r, vec![repr(); 2]);
    fn build<'p>(
        ctx: &'p SelectionContext<'p>,
        body: &lir::VerifiedCallable<'p>,
        views: &[ViewId],
    ) -> VerifiedSelectedCallable<'p, Node> {
        let (mut b, entry, args) = begin(ctx, body);
        let sum = b.value(repr(), Some(origin())).unwrap();
        b.append(
            entry,
            Node::new(
                Op::Add {
                    a: vf(args[0], repr()),
                    b: vf(args[1], repr()),
                    out: vf(sum, repr()),
                    destructive: true,
                },
                views,
            ),
        )
        .unwrap();
        ret(&mut b, entry, vec![], vec![], views);
        checked(b, Shape::Two)
    }
    let first = build(&first_ctx, &bodies[0], &first_views);
    let equal = build(&equal_ctx, &bodies[0], &equal_views);
    assert!(!first.receipt().same_snapshot(&equal.receipt()));
    let mut first_text = String::new();
    let mut equal_text = String::new();
    first.dump(&mut first_text).unwrap();
    equal.dump(&mut equal_text).unwrap();
    assert_eq!(first_text, equal_text);
    assert!(first_text.contains("add-destructive"));
    assert!(first_text.contains("ties=[Tie { input: 0, output: 2 }]"));
}
