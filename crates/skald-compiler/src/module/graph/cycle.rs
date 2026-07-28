use crate::{identity::ModuleId, source::Span};

use super::model::ModuleImportEdge;

pub(super) struct ImportCycle {
    pub edges: Vec<CycleEdge>,
}

pub(super) struct CycleEdge {
    pub source: ModuleId,
    pub target: ModuleId,
    pub span: Span,
}

/// Finds the first cycle in canonical module and edge order without recursion.
pub(super) fn find_cycle(imports: &[Vec<ModuleImportEdge>]) -> Option<ImportCycle> {
    let mut states = vec![VisitState::Unvisited; imports.len()];
    let mut stack = Vec::<VisitFrame>::new();

    for start in 0..imports.len() {
        if states[start] != VisitState::Unvisited {
            continue;
        }
        states[start] = VisitState::Visiting;
        stack.push(VisitFrame::new(ModuleId::new(start)));

        while let Some(frame) = stack.last_mut() {
            let source = frame.module;
            let Some(edge) = imports[source.index()].get(frame.next_edge) else {
                states[source.index()] = VisitState::Visited;
                stack.pop();
                continue;
            };
            frame.next_edge += 1;

            match states[edge.target().index()] {
                VisitState::Unvisited => {
                    states[edge.target().index()] = VisitState::Visiting;
                    stack.push(VisitFrame::new(edge.target()));
                }
                VisitState::Visiting => {
                    let cycle_start = stack
                        .iter()
                        .position(|frame| frame.module == edge.target())
                        .expect("a visiting module is present on the DFS stack");
                    let mut cycle_edges = stack[cycle_start..]
                        .windows(2)
                        .map(|frames| edge_between(frames[0].module, frames[1].module, imports))
                        .collect::<Vec<_>>();
                    cycle_edges.push(CycleEdge {
                        source,
                        target: edge.target(),
                        span: edge.first_evidence_span(),
                    });
                    return Some(ImportCycle { edges: cycle_edges });
                }
                VisitState::Visited => {}
            }
        }
    }
    None
}

fn edge_between(
    source: ModuleId,
    target: ModuleId,
    imports: &[Vec<ModuleImportEdge>],
) -> CycleEdge {
    let edge = imports[source.index()]
        .iter()
        .find(|edge| edge.target() == target)
        .expect("the DFS stack follows direct import edges");
    CycleEdge {
        source,
        target,
        span: edge.first_evidence_span(),
    }
}

struct VisitFrame {
    module: ModuleId,
    next_edge: usize,
}

impl VisitFrame {
    const fn new(module: ModuleId) -> Self {
        Self {
            module,
            next_edge: 0,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum VisitState {
    Unvisited,
    Visiting,
    Visited,
}
