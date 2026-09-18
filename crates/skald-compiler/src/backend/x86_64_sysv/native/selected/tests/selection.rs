use super::*;
#[test]
fn ordinary_source_graphs_publish_native_virtual_operands_and_preserve_origins() {
    let mut source="fn main() -> i64 { var n: i64 = 3; var sum: i64 = 0; while (n > 0) { sum = sum + n; n = n - 1; } return sum; } fn integer(a: i64, b: i64) -> i64 { return ~(-a) + (a * b - b) & (a | b ^ a); } fn floating(a: f64, b: f64) -> f64 { return -a + a * b - a / b + 1.5; } fn byte(a: u8, b: u8) -> u8 { return a * b + a - b; } fn boolean(a: bool) -> bool { return !a; } fn reference() -> fn(i64, i64) -> i64 { return integer; }".to_owned();
    for ty in ["i64", "u64", "u8", "f64"] {
        for (index, predicate) in ["==", "!=", "<", "<=", ">", ">="].iter().enumerate() {
            source.push_str(&format!(
                "fn compare_{ty}_{index}(a: {ty}, b: {ty}) -> bool {{ return a {predicate} b; }}"
            ));
        }
    }
    let mut counts = std::collections::HashSet::new();
    let mut comparisons = 0;
    let mut finite_bundles = 0;
    for_sources(&source, |context, lower| {
        let body = select(context, lower).unwrap();
        assert_eq!(
            body.draft().identity().callable,
            lower.receipt().owner().key()
        );
        assert!(body.analysis().is_ok());
        body.visit(|fact| {
            match fact {
                SelectedFact::Origins {
                    values,
                    blocks,
                    objects,
                } => {
                    assert_eq!(values.len(), lower.draft().values().len());
                    assert_eq!(blocks.len(), lower.draft().blocks().len());
                    assert_eq!(objects.len(), lower.draft().objects().len());
                }
                SelectedFact::Instruction { payload, .. } => {
                    counts.insert(std::mem::discriminant(&payload.opcode));
                    if matches!(
                        payload.opcode,
                        Opcode::IntegerCompare { .. } | Opcode::FloatCompare { .. }
                    ) {
                        comparisons += 1;
                    }
                    let desc = payload.describe();
                    assert!(desc.operands.iter().all(|op| matches!(
                        op.constraint,
                        Constraint::Resources { memory: false, .. }
                    )));
                    assert!(matches!(payload.origin.site, Site::Instruction { .. }));
                    if let Some(Bundle::Bounded { steps, scratch }) = &desc.bundle {
                        finite_bundles += 1;
                        assert!(steps.get() <= 4);
                        assert!(!scratch.is_empty());
                    }
                }
                _ => {}
            }
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap();
    });
    assert_eq!(comparisons, 25);
    assert!(finite_bundles >= 7); // Four parity bundles, sign flip, byte multiply, float constant.
    assert!(counts.len() >= 10);
}

#[test]
fn heterogeneous_incoming_slots_publish_in_one_context_and_wrong_shapes_reject() {
    let integer = (0..7)
        .map(|i| format!("a{i}: i64"))
        .collect::<Vec<_>>()
        .join(", ");
    let float = (0..9)
        .map(|i| format!("a{i}: f64"))
        .collect::<Vec<_>>()
        .join(", ");
    let source=format!("fn integers({integer}) -> i64 {{ return a6; }} fn floats({float}) -> f64 {{ return a8; }} fn main() -> i64 {{ return 0; }}");
    let mut slots = vec![];
    for_sources(&source, |context, lower| {
        let body = select(context, lower).unwrap();
        for binding in body.draft().abi().unwrap().inputs() {
            if let selected::AbiLocation::Slot { area, index } = binding.location {
                assert_eq!((area, index), (selected::AbiArea::Incoming, 0));
                slots.push(binding.representation.kind);
                let signature = lower.receipt().owner().signature_id();
                let original = body.draft().abi().unwrap();
                for corruption in 0..3 {
                    let mut inputs = original.inputs().to_vec();
                    let b = inputs
                        .iter_mut()
                        .find(|b| matches!(b.location, selected::AbiLocation::Slot { .. }))
                        .unwrap();
                    match corruption {
                        0 => {
                            b.location = selected::AbiLocation::Slot {
                                area: selected::AbiArea::Results,
                                index: 0,
                            }
                        }
                        1 => {
                            b.location = selected::AbiLocation::Slot {
                                area: selected::AbiArea::Incoming,
                                index: 1,
                            }
                        }
                        2 => {
                            b.representation = selected::Representation::new(
                                selected::RepresentationKind::Bits,
                                32,
                            )
                            .unwrap()
                        }
                        _ => unreachable!(),
                    }
                    assert!(context
                        .abi_bindings(signature, inputs, original.results().to_vec())
                        .is_err());
                }
            }
        }
    });
    assert_eq!(
        slots,
        vec![
            selected::RepresentationKind::Bits,
            selected::RepresentationKind::Float
        ]
    );
}

#[test]
fn duplicate_parameter_edges_get_distinct_forwarders_after_the_atomic_branch() {
    let mut facts = plan::test_fixtures::facts();
    facts.signatures[0].returns = plan::ReturnShape::Scalar(plan::ScalarType::I64);
    facts.signatures[0].results = vec![plan::Component {
        ty: plan::ScalarType::I64,
        role: plan::ComponentRole::Result,
    }];
    let plan = plan::CheckedPlan::check(facts).unwrap();
    let key = plan::test_fixtures::source(0);
    let mut builder = lir::DraftBuilder::new(plan.view().callable(key).unwrap()).unwrap();
    let entry = builder.reserve_block().unwrap();
    let exit = builder.reserve_block().unwrap();
    let parameter = builder.reserve_value(plan::ScalarType::I64, None).unwrap();
    builder.define_block(entry, &[]).unwrap();
    builder.define_block(exit, &[parameter]).unwrap();
    builder.set_entry(entry).unwrap();
    let condition = builder
        .append(entry, lir::Operation::Constant(lir::Constant::Bool(true)))
        .unwrap()[0];
    let first = builder
        .append(entry, lir::Operation::Constant(lir::Constant::I64(11)))
        .unwrap()[0];
    let second = builder
        .append(entry, lir::Operation::Constant(lir::Constant::I64(22)))
        .unwrap()[0];
    builder
        .terminate(
            entry,
            lir::Terminator::Branch {
                condition,
                true_edge: lir::Edge {
                    target: exit,
                    arguments: vec![first],
                },
                false_edge: lir::Edge {
                    target: exit,
                    arguments: vec![second],
                },
            },
        )
        .unwrap();
    builder
        .terminate(exit, lir::Terminator::Return(vec![parameter]))
        .unwrap();
    let lower = lir::verify_callable(builder.finish()).unwrap();
    let catalog = TargetDeclarations::new(plan.view()).freeze().unwrap();
    let context = selection_context(&catalog).unwrap();
    let body = select(&context, &lower).unwrap();
    let graph = body.analysis().unwrap();
    assert_eq!(graph.successors(0), Some([2, 3].as_slice()));
    assert_eq!(graph.predecessors(1), Some([(2, 0), (3, 0)].as_slice()));
    let mut forwarding = vec![];
    body.visit(|fact| {
        if let SelectedFact::Terminal {
            block,
            payload: Some(payload),
            edges,
        } = fact
        {
            if let Site::Edge {
                block: source,
                slot,
            } = payload.origin.site
            {
                assert_eq!(source, entry.id());
                assert_eq!(block.index(), slot + 2);
                assert_eq!(edges.len(), 1);
                assert_eq!(edges[0].0.index(), 1);
                assert_eq!(edges[0].1.len(), 1);
                forwarding.push((slot, edges[0].1[0]));
            }
            if matches!(payload.opcode, Opcode::Branch { .. }) {
                assert!(matches!(payload.describe().bundle, Some(Bundle::Atomic)));
                assert!(edges.iter().all(|(_, args)| args.is_empty()));
            }
        }
        Ok::<_, std::convert::Infallible>(())
    })
    .unwrap();
    assert_eq!(forwarding.len(), 2);
    assert_ne!(forwarding[0].1, forwarding[1].1);
    let verifier = Verifier::new(plan.view().profile()).unwrap();
    let mut editor = body.into_editor();
    let branch = editor.blocks().next().unwrap();
    let target = editor.blocks().nth(1).unwrap();
    for (slot, argument) in forwarding {
        editor
            .redirect_edge(branch, slot, target, &[editor.value(argument).unwrap()])
            .unwrap();
    }
    match editor.finish(&verifier) {
        Err(selected::SelectedEditFailure::Verify(failures)) => {
            assert!(failures.iter().any(|f| matches!(
                f.reason,
                selected::SelectedReason::Target("parameter transfers require forwarding blocks")
            )))
        }
        _ => panic!("joint publication must enforce native parameter transfer normalization"),
    }
}
