//! Deterministic, non-recursive analysis of inline class dependencies.

use std::collections::VecDeque;

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    identity::{ClassId, FieldId},
    resolve::{ResolvedProgram, ResolvedTypeKind},
};

use super::RECURSIVE_INLINE_CONTAINMENT;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ContainmentEdge {
    source: ClassId,
    target: ClassId,
    kind: ContainmentEdgeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContainmentEdgeKind {
    Base,
    Field(FieldId),
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
            if let Some(target) = program.hierarchy.direct_base(class.id) {
                edges[class.id.index()].push(ContainmentEdge {
                    source: class.id,
                    target,
                    kind: ContainmentEdgeKind::Base,
                });
                reverse_edges[target.index()].push(class.id.index());
            }
            for field in &class.fields {
                let Some(target) = inline_class_target(program, field.type_syntax.kind) else {
                    continue;
                };
                if target.index() >= class_count {
                    continue;
                }
                edges[class.id.index()].push(ContainmentEdge {
                    source: class.id,
                    target,
                    kind: ContainmentEdgeKind::Field(field.id),
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

fn inline_class_target(program: &ResolvedProgram, mut kind: ResolvedTypeKind) -> Option<ClassId> {
    loop {
        match kind {
            ResolvedTypeKind::Class(class) => return Some(class),
            ResolvedTypeKind::Optional(optional) => {
                kind = program.optional_types.get(optional)?.payload.kind;
            }
            ResolvedTypeKind::Array(_)
            | ResolvedTypeKind::Shared(_)
            | ResolvedTypeKind::Function(_) => return None,
            ResolvedTypeKind::I64
            | ResolvedTypeKind::U64
            | ResolvedTypeKind::U8
            | ResolvedTypeKind::F64
            | ResolvedTypeKind::Bool
            | ResolvedTypeKind::Unit
            | ResolvedTypeKind::Obj
            | ResolvedTypeKind::Interface(_) => return None,
        }
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
        current = edge.source.index();
    }
    reversed.reverse();
    Some(reversed)
}

fn cycle_diagnostic(program: &ResolvedProgram, cycle: &[ContainmentEdge]) -> Diagnostic {
    let path = render_cycle(program, cycle);
    let mut diagnostic = Diagnostic::error(
        RECURSIVE_INLINE_CONTAINMENT,
        format!("recursive inline containment: {path}"),
    )
    .with_primary_label(edge_span(program, cycle[0]), edge_label(program, cycle[0]));

    for &edge in &cycle[1..] {
        diagnostic =
            diagnostic.with_secondary_label(edge_span(program, edge), edge_label(program, edge));
    }

    diagnostic
        .with_note("inline class fields and base subobjects must form an acyclic, finite layout")
}

fn render_cycle(program: &ResolvedProgram, cycle: &[ContainmentEdge]) -> String {
    let mut path = Vec::with_capacity(cycle.len() + 1);
    for edge in cycle {
        let owner = program
            .class(edge.source)
            .expect("containment edge must have a resolved owner");
        match edge.kind {
            ContainmentEdgeKind::Base => {
                let target = program
                    .class(edge.target)
                    .expect("base containment edge must target a class");
                path.push(format!("{} extends {}", owner.name, target.name));
            }
            ContainmentEdgeKind::Field(field) => {
                let field = program
                    .field(field)
                    .expect("field containment edge must reference a field");
                path.push(format!("{}.{}", owner.name, field.name));
            }
        }
    }
    let destination = program
        .class(cycle.last().expect("cycle cannot be empty").target)
        .expect("containment edge must target a resolved class");
    path.push(destination.name.clone());
    format!("`{}`", path.join(" -> "))
}

fn edge_span(program: &ResolvedProgram, edge: ContainmentEdge) -> crate::source::Span {
    match edge.kind {
        ContainmentEdgeKind::Base => {
            program
                .class(edge.source)
                .and_then(|class| class.direct_base)
                .filter(|base| base.class == edge.target)
                .expect("base containment edge must reference resolved source metadata")
                .span
        }
        ContainmentEdgeKind::Field(field) => {
            program
                .field(field)
                .expect("field containment edge must reference a field")
                .type_syntax
                .span
        }
    }
}

fn edge_label(program: &ResolvedProgram, edge: ContainmentEdge) -> String {
    let target = program
        .class(edge.target)
        .expect("containment edge must target a resolved class");
    match edge.kind {
        ContainmentEdgeKind::Base => {
            let source = program
                .class(edge.source)
                .expect("base containment edge must have a resolved owner");
            format!(
                "class `{}` contains base `{}` inline",
                source.name, target.name
            )
        }
        ContainmentEdgeKind::Field(field) => {
            let field = program
                .field(field)
                .expect("field containment edge must reference a field");
            format!("field `{}` contains `{}` inline", field.name, target.name)
        }
    }
}
