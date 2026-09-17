use super::*;
use crate::backend::plan::test_fixtures::{facts, source};
struct Fixture {
    graph: GraphDescription<u8>,
    stage: GraphStage,
}
impl GraphView for Fixture {
    type Type = u8;
    fn identity(&self) -> GraphIdentity {
        GraphIdentity {
            stage: self.stage,
            target: facts().profile,
            callable: source(0),
        }
    }
    fn describe(&self) -> Result<GraphDescription<u8>, GraphFailure> {
        // Independent stage fixture: the common owner must check its tables itself.
        Ok(GraphDescription {
            entry: self.graph.entry,
            inputs: self.graph.inputs.clone(),
            values: self
                .graph
                .values
                .iter()
                .map(|v| ValueDescription {
                    ty: v.ty,
                    definition: v.definition,
                    origin: v.origin,
                })
                .collect(),
            blocks: self
                .graph
                .blocks
                .iter()
                .map(|b| BlockDescription {
                    parameters: b.parameters.clone(),
                    instructions: b
                        .instructions
                        .iter()
                        .map(|i| InstructionDescription {
                            uses: i.uses.iter().map(copy_use).collect(),
                            results: i.results.clone(),
                        })
                        .collect(),
                    terminal: b.terminal.as_ref().map(|(uses, edges)| {
                        (
                            uses.iter().map(copy_use).collect(),
                            edges
                                .iter()
                                .map(|e| EdgeDescription {
                                    target: e.target,
                                    arguments: e.arguments.iter().map(copy_use).collect(),
                                })
                                .collect(),
                        )
                    }),
                })
                .collect(),
        })
    }
}
fn copy_use(v: &TypedUse<u8>) -> TypedUse<u8> {
    TypedUse {
        value: v.value,
        expected: v.expected,
    }
}
fn block() -> BlockDescription<u8> {
    BlockDescription {
        parameters: Some(vec![]),
        instructions: vec![],
        terminal: Some((vec![], vec![])),
    }
}
fn fixture() -> Fixture {
    Fixture {
        graph: GraphDescription {
            entry: Some(0),
            inputs: vec![],
            values: vec![],
            blocks: vec![block()],
        },
        stage: GraphStage::Selected,
    }
}
fn edge(target: usize, values: &[usize]) -> EdgeDescription<u8> {
    EdgeDescription {
        target,
        arguments: values
            .iter()
            .map(|value| TypedUse {
                value: *value,
                expected: None,
            })
            .collect(),
    }
}
fn result(f: &mut Fixture, block: usize) -> usize {
    let value = f.graph.values.len();
    let instruction = f.graph.blocks[block].instructions.len();
    f.graph.values.push(ValueDescription {
        ty: 64,
        definition: Some(DefinitionSite::Result {
            block,
            instruction,
            ordinal: 0,
        }),
        origin: None,
    });
    f.graph.blocks[block]
        .instructions
        .push(InstructionDescription {
            uses: vec![],
            results: vec![(value, 64)],
        });
    value
}
fn reasons(f: &Fixture) -> Vec<GraphReason> {
    check_graph(f)
        .err()
        .expect("invalid structure")
        .iter()
        .map(|e| e.reason)
        .collect()
}
#[test]
fn selected_structural_fixture_uses_shared_session_and_canonical_failures() {
    let mut f = fixture();
    let v = result(&mut f, 0);
    let session = check_graph(&f).unwrap();
    assert!(std::ptr::eq(session.owner(), &f));
    assert_eq!(session.reachable(0), Some(true));
    assert_eq!(session.reachable(99), None);
    assert_eq!(session.dominates(99, 0), None);
    assert_eq!(session.predecessors(0), Some([].as_slice()));
    f.graph.blocks[0].instructions[0].results.push((v, 8));
    let errors = check_graph(&f).err().unwrap();
    assert_eq!(errors[0].identity.stage, GraphStage::Selected);
    assert_eq!(errors[0].identity.target, facts().profile);
    assert!(errors
        .iter()
        .any(|e| e.reason == GraphReason::DuplicateDefinition));
    assert!(errors.iter().any(|e| e.reason == GraphReason::TypeMismatch));
    assert_eq!(errors, check_graph(&f).err().unwrap());
}
#[test]
fn unresolved_and_dangling_storage_is_rejected_before_analysis() {
    let mut f = fixture();
    f.graph.entry = Some(99);
    f.graph.blocks[0].parameters = None;
    f.graph.blocks[0].terminal = None;
    f.graph.values.push(ValueDescription {
        ty: 64,
        definition: None,
        origin: None,
    });
    f.graph.blocks[0].instructions.push(InstructionDescription {
        uses: vec![TypedUse {
            value: usize::MAX,
            expected: None,
        }],
        results: vec![(usize::MAX, 64)],
    });
    let r = reasons(&f);
    for expected in [
        GraphReason::InvalidEntry,
        GraphReason::UnresolvedBlock,
        GraphReason::MissingTerminator,
        GraphReason::UnresolvedValue,
        GraphReason::OutOfBounds,
    ] {
        assert!(r.contains(&expected));
    }
}
#[test]
fn definition_site_and_result_type_are_independently_checked() {
    let mut f = fixture();
    let v = result(&mut f, 0);
    f.graph.values[v].definition = Some(DefinitionSite::Result {
        block: 0,
        instruction: usize::MAX,
        ordinal: 0,
    });
    assert!(reasons(&f).contains(&GraphReason::DefinitionMismatch));
    f.graph.values[v].definition = Some(DefinitionSite::Result {
        block: 0,
        instruction: 0,
        ordinal: 0,
    });
    f.graph.values[v].ty = 8;
    assert!(reasons(&f).contains(&GraphReason::TypeMismatch));
}
#[test]
fn successor_parameter_is_not_available_on_predecessor_edge() {
    let mut f = fixture();
    f.graph.blocks.push(block());
    f.graph.values.push(ValueDescription {
        ty: 64,
        definition: Some(DefinitionSite::Parameter {
            block: 1,
            ordinal: 0,
        }),
        origin: None,
    });
    f.graph.blocks[1].parameters = Some(vec![0]);
    f.graph.blocks[0].terminal = Some((vec![], vec![edge(1, &[0])]));
    let e = check_graph(&f).err().unwrap();
    assert_eq!(e[0].reason, GraphReason::NonDominatingUse);
    assert_eq!(
        e[0].location,
        GraphLocation::Edge {
            block: 0,
            slot: 0,
            operand: 0
        }
    );
}
#[test]
fn unreachable_cross_block_dominance_is_unknown_but_new_reachability_rechecks_uses() {
    let mut f = fixture();
    f.graph.blocks.extend([block(), block()]);
    let v = result(&mut f, 1);
    f.graph.blocks[2].terminal = Some((
        vec![TypedUse {
            value: v,
            expected: Some(64),
        }],
        vec![],
    ));
    let s = check_graph(&f).unwrap();
    assert_eq!(s.reachable(2), Some(false));
    assert_eq!(s.dominates(1, 2), None);
    assert_eq!(s.dominates(1, 1), Some(true));
    f.graph.blocks[0].terminal = Some((vec![], vec![edge(2, &[])]));
    assert!(reasons(&f).contains(&GraphReason::NonDominatingUse));
}
#[test]
fn unreachable_local_order_and_edge_signatures_still_apply() {
    let mut f = fixture();
    f.graph.blocks.push(block());
    let v = result(&mut f, 1);
    f.graph.blocks[1].instructions[0].uses.push(TypedUse {
        value: v,
        expected: None,
    });
    assert!(reasons(&f).contains(&GraphReason::UseBeforeDefinition));
    f.graph.blocks[1].instructions[0].uses.clear();
    f.graph.blocks[1].terminal = Some((vec![], vec![edge(1, &[v])]));
    assert!(reasons(&f).contains(&GraphReason::ArityMismatch));
}
#[test]
fn parallel_critical_edges_and_simultaneous_loop_swaps_keep_occurrences() {
    let mut f = fixture();
    f.graph.blocks.extend([block(), block()]);
    let a = result(&mut f, 0);
    let b = result(&mut f, 0);
    for ordinal in 0..2 {
        f.graph.values.push(ValueDescription {
            ty: 64,
            definition: Some(DefinitionSite::Parameter { block: 2, ordinal }),
            origin: None,
        });
    }
    f.graph.blocks[2].parameters = Some(vec![2, 3]);
    f.graph.blocks[0].terminal = Some((vec![], vec![edge(2, &[a, b]), edge(1, &[])]));
    f.graph.blocks[1].terminal = Some((vec![], vec![edge(2, &[b, a]), edge(2, &[a, b])]));
    f.graph.blocks[2].terminal = Some((vec![], vec![edge(2, &[3, 2])]));
    let s = check_graph(&f).unwrap();
    assert_eq!(
        s.predecessors(2),
        Some([(0, 0), (1, 0), (1, 1), (2, 0)].as_slice())
    );
    assert_eq!(s.dominates(0, 2), Some(true));
    assert_eq!(s.dominates(1, 2), Some(false));
}
#[test]
fn entry_predecessor_and_wrong_parameter_types_are_rejected() {
    let mut f = fixture();
    f.graph.blocks[0].terminal = Some((vec![], vec![edge(0, &[])]));
    assert!(reasons(&f).contains(&GraphReason::EntryPredecessor));
    f.graph.blocks.push(block());
    let v = result(&mut f, 0);
    f.graph.values.push(ValueDescription {
        ty: 8,
        definition: Some(DefinitionSite::Parameter {
            block: 1,
            ordinal: 0,
        }),
        origin: None,
    });
    f.graph.blocks[1].parameters = Some(vec![1]);
    f.graph.blocks[0].terminal = Some((vec![], vec![edge(1, &[v])]));
    assert!(reasons(&f).contains(&GraphReason::TypeMismatch));
}

#[test]
fn reachable_diamond_rejects_a_sibling_definition_at_the_join() {
    let mut f = fixture();
    f.graph.blocks.extend([block(), block(), block()]);
    let v = result(&mut f, 1);
    f.graph.blocks[0].terminal = Some((vec![], vec![edge(1, &[]), edge(2, &[])]));
    f.graph.blocks[1].terminal = Some((vec![], vec![edge(3, &[])]));
    f.graph.blocks[2].terminal = Some((vec![], vec![edge(3, &[])]));
    f.graph.blocks[3].terminal = Some((
        vec![TypedUse {
            value: v,
            expected: Some(64),
        }],
        vec![],
    ));
    assert_eq!(reasons(&f), vec![GraphReason::NonDominatingUse]);
}
