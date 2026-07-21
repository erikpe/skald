//! Deterministic, non-recursive analysis of class-field dependencies.

use std::collections::VecDeque;

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    identity::{ClassId, FieldId},
    resolve::{ResolvedProgram, ResolvedTypeKind},
};

use super::RECURSIVE_INLINE_CONTAINMENT;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ContainmentEdge {
    field: FieldId,
    target: ClassId,
}

pub(in crate::typeck) fn validate_containment(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) {
    let graph = ContainmentGraph::new(program);
    for component in graph.recursive_components() {
        let cycle = graph
            .representative_cycle(&component)
            .expect("a recursive component must contain a cycle");
        diagnostics.push(cycle_diagnostic(program, &cycle));
    }
}

struct ContainmentGraph {
    edges: Vec<Vec<ContainmentEdge>>,
    reverse_edges: Vec<Vec<usize>>,
}

impl ContainmentGraph {
    fn new(program: &ResolvedProgram) -> Self {
        let class_count = program.classes.len();
        let mut edges = vec![Vec::new(); class_count];
        let mut reverse_edges = vec![Vec::new(); class_count];

        for class in program.classes.iter() {
            for field in &class.fields {
                let ResolvedTypeKind::Class(target) = field.type_syntax.kind else {
                    continue;
                };
                if target.index() >= class_count {
                    continue;
                }
                edges[class.id.index()].push(ContainmentEdge {
                    field: field.id,
                    target,
                });
                reverse_edges[target.index()].push(class.id.index());
            }
        }

        Self {
            edges,
            reverse_edges,
        }
    }

    fn recursive_components(&self) -> Vec<Vec<usize>> {
        let finishing_order = self.finishing_order();
        let mut assigned = vec![false; self.edges.len()];
        let mut components = Vec::new();

        for &start in finishing_order.iter().rev() {
            if assigned[start] {
                continue;
            }
            assigned[start] = true;
            let mut stack = vec![start];
            let mut component = Vec::new();
            while let Some(class) = stack.pop() {
                component.push(class);
                for &predecessor in &self.reverse_edges[class] {
                    if !assigned[predecessor] {
                        assigned[predecessor] = true;
                        stack.push(predecessor);
                    }
                }
            }
            component.sort_unstable();
            if self.is_recursive(&component) {
                components.push(component);
            }
        }

        components.sort_by_key(|component| component[0]);
        components
    }

    fn finishing_order(&self) -> Vec<usize> {
        let mut visited = vec![false; self.edges.len()];
        let mut order = Vec::with_capacity(self.edges.len());

        for start in 0..self.edges.len() {
            if visited[start] {
                continue;
            }
            visited[start] = true;
            let mut stack = vec![(start, 0usize)];
            while let Some((class, next_edge)) = stack.last_mut() {
                if let Some(edge) = self.edges[*class].get(*next_edge) {
                    *next_edge += 1;
                    let target = edge.target.index();
                    if !visited[target] {
                        visited[target] = true;
                        stack.push((target, 0));
                    }
                } else {
                    let (finished, _) = stack.pop().expect("active traversal frame must exist");
                    order.push(finished);
                }
            }
        }

        order
    }

    fn is_recursive(&self, component: &[usize]) -> bool {
        component.len() > 1
            || self.edges[component[0]]
                .iter()
                .any(|edge| edge.target.index() == component[0])
    }

    fn representative_cycle(&self, component: &[usize]) -> Option<Vec<ContainmentEdge>> {
        let start = component[0];
        let mut in_component = vec![false; self.edges.len()];
        for &class in component {
            in_component[class] = true;
        }

        for &first in &self.edges[start] {
            let target = first.target.index();
            if !in_component[target] {
                continue;
            }
            if target == start {
                return Some(vec![first]);
            }
            if let Some(mut remainder) = self.path_to(target, start, &in_component) {
                let mut cycle = Vec::with_capacity(remainder.len() + 1);
                cycle.push(first);
                cycle.append(&mut remainder);
                return Some(cycle);
            }
        }

        None
    }

    fn path_to(
        &self,
        from: usize,
        destination: usize,
        allowed: &[bool],
    ) -> Option<Vec<ContainmentEdge>> {
        let mut predecessor = vec![None; self.edges.len()];
        let mut visited = vec![false; self.edges.len()];
        let mut queue = VecDeque::new();
        visited[from] = true;
        queue.push_back(from);

        while let Some(class) = queue.pop_front() {
            for &edge in &self.edges[class] {
                let target = edge.target.index();
                if !allowed[target] || visited[target] {
                    continue;
                }
                visited[target] = true;
                predecessor[target] = Some(edge);
                if target == destination {
                    return reconstruct_path(from, destination, &predecessor);
                }
                queue.push_back(target);
            }
        }

        None
    }
}

fn reconstruct_path(
    from: usize,
    destination: usize,
    predecessor: &[Option<ContainmentEdge>],
) -> Option<Vec<ContainmentEdge>> {
    let mut reversed = Vec::new();
    let mut current = destination;
    while current != from {
        let edge = predecessor.get(current).copied().flatten()?;
        reversed.push(edge);
        current = edge.field.class().index();
    }
    reversed.reverse();
    Some(reversed)
}

fn cycle_diagnostic(program: &ResolvedProgram, cycle: &[ContainmentEdge]) -> Diagnostic {
    let path = render_cycle(program, cycle);
    let first = program
        .field(cycle[0].field)
        .expect("containment edge must reference its resolved field");
    let first_target = program
        .class(cycle[0].target)
        .expect("containment edge must target a resolved class");
    let mut diagnostic = Diagnostic::error(
        RECURSIVE_INLINE_CONTAINMENT,
        format!("recursive inline containment: {path}"),
    )
    .with_primary_label(
        first.type_syntax.span,
        format!(
            "field `{}` contains `{}` inline",
            first.name, first_target.name
        ),
    );

    for &edge in &cycle[1..] {
        let field = program
            .field(edge.field)
            .expect("containment edge must reference its resolved field");
        let target = program
            .class(edge.target)
            .expect("containment edge must target a resolved class");
        diagnostic = diagnostic.with_secondary_label(
            field.type_syntax.span,
            format!("field `{}` contains `{}` inline", field.name, target.name),
        );
    }

    diagnostic.with_note("inline class fields must form an acyclic, finite layout")
}

fn render_cycle(program: &ResolvedProgram, cycle: &[ContainmentEdge]) -> String {
    let mut path = Vec::with_capacity(cycle.len() + 1);
    for edge in cycle {
        let field = program
            .field(edge.field)
            .expect("containment edge must reference its resolved field");
        let owner = program
            .class(edge.field.class())
            .expect("containment field must have a resolved owner");
        path.push(format!("{}.{}", owner.name, field.name));
    }
    let destination = program
        .class(cycle.last().expect("cycle cannot be empty").target)
        .expect("containment edge must target a resolved class");
    path.push(destination.name.clone());
    format!("`{}`", path.join(" -> "))
}
