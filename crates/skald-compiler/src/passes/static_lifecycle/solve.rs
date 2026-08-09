//! Recursive-component condensation, effect propagation, and witnesses.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::identity::StaticFieldId;

use super::{
    extract::{ExtractedGraph, NodeDraft},
    model::{StaticAccessEvidence, StaticEffectAnalysis, StaticEffectNode, StaticEffectSummary},
};

pub(crate) fn solve(graph: ExtractedGraph) -> StaticEffectAnalysis {
    let nodes = graph.nodes.keys().copied().collect::<Vec<_>>();
    let indices = nodes
        .iter()
        .copied()
        .enumerate()
        .map(|(index, node)| (node, index))
        .collect::<BTreeMap<_, _>>();
    let adjacency = nodes
        .iter()
        .map(|node| {
            graph.nodes[node]
                .edges
                .iter()
                .map(|edge| indices[&edge.target])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let components = strongly_connected_components(&adjacency);
    let component_count = components
        .iter()
        .copied()
        .max()
        .map_or(0, |maximum| maximum + 1);
    let component_fields = propagated_component_fields(
        &nodes,
        &graph.nodes,
        &adjacency,
        &components,
        component_count,
    );

    let summaries = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let draft = &graph.nodes[node];
            let effects = component_fields[components[index]]
                .iter()
                .filter_map(|field| witness_for(*node, *field, &graph.nodes))
                .collect();
            StaticEffectSummary {
                node: *node,
                direct_effects: draft.direct.clone(),
                effects,
            }
        })
        .collect();
    let recursive_components = recursive_component_count(&adjacency, &components, component_count);

    StaticEffectAnalysis::new(summaries, recursive_components)
}

fn strongly_connected_components(adjacency: &[Vec<usize>]) -> Vec<usize> {
    let mut visited = vec![false; adjacency.len()];
    let mut finish_order = Vec::with_capacity(adjacency.len());
    for node in 0..adjacency.len() {
        if visited[node] {
            continue;
        }
        visited[node] = true;
        let mut pending = vec![(node, 0)];
        while let Some((current, next_edge)) = pending.last_mut() {
            if let Some(target) = adjacency[*current].get(*next_edge).copied() {
                *next_edge += 1;
                if !std::mem::replace(&mut visited[target], true) {
                    pending.push((target, 0));
                }
            } else {
                finish_order.push(*current);
                pending.pop();
            }
        }
    }

    let mut reverse = vec![Vec::new(); adjacency.len()];
    for (source, targets) in adjacency.iter().enumerate() {
        for target in targets {
            reverse[*target].push(source);
        }
    }
    for edges in &mut reverse {
        edges.sort_unstable();
        edges.dedup();
    }

    let mut components = vec![usize::MAX; adjacency.len()];
    let mut component = 0;
    for node in finish_order.into_iter().rev() {
        if components[node] == usize::MAX {
            components[node] = component;
            let mut pending = vec![node];
            while let Some(current) = pending.pop() {
                for target in &reverse[current] {
                    if components[*target] == usize::MAX {
                        components[*target] = component;
                        pending.push(*target);
                    }
                }
            }
            component += 1;
        }
    }
    components
}

fn propagated_component_fields(
    nodes: &[StaticEffectNode],
    drafts: &BTreeMap<StaticEffectNode, NodeDraft>,
    adjacency: &[Vec<usize>],
    components: &[usize],
    component_count: usize,
) -> Vec<BTreeSet<StaticFieldId>> {
    let mut direct = vec![BTreeSet::new(); component_count];
    let mut successors = vec![BTreeSet::new(); component_count];
    for (index, node) in nodes.iter().enumerate() {
        let component = components[index];
        direct[component].extend(drafts[node].direct.iter().map(|effect| effect.field));
        for target in &adjacency[index] {
            let target_component = components[*target];
            if component != target_component {
                successors[component].insert(target_component);
            }
        }
    }

    let mut predecessors = vec![BTreeSet::new(); component_count];
    let mut remaining_successors = vec![0usize; component_count];
    for (component, targets) in successors.iter().enumerate() {
        remaining_successors[component] = targets.len();
        for target in targets {
            predecessors[*target].insert(component);
        }
    }
    let mut ready = remaining_successors
        .iter()
        .enumerate()
        .filter_map(|(component, count)| (*count == 0).then_some(component))
        .collect::<BTreeSet<_>>();
    let mut propagated = direct;
    let mut completed = 0;
    while let Some(component) = ready.pop_first() {
        completed += 1;
        for predecessor in predecessors[component].iter().copied() {
            let inherited = propagated[component].clone();
            propagated[predecessor].extend(inherited);
            remaining_successors[predecessor] -= 1;
            if remaining_successors[predecessor] == 0 {
                ready.insert(predecessor);
            }
        }
    }
    debug_assert_eq!(
        completed, component_count,
        "component graph must be acyclic"
    );
    propagated
}

fn witness_for(
    root: StaticEffectNode,
    field: StaticFieldId,
    drafts: &BTreeMap<StaticEffectNode, NodeDraft>,
) -> Option<StaticAccessEvidence> {
    let mut queue = VecDeque::from([(root, Vec::new())]);
    let mut visited = BTreeSet::new();
    while let Some((node, path)) = queue.pop_front() {
        if !visited.insert(node) {
            continue;
        }
        if let Some(direct) = drafts[&node]
            .direct
            .iter()
            .find(|effect| effect.field == field)
        {
            let mut evidence = direct.clone();
            evidence.witness = path;
            if let Some(first) = evidence.witness.first() {
                evidence.phase = first.phase;
            }
            return Some(evidence);
        }
        for edge in &drafts[&node].edges {
            if !visited.contains(&edge.target) {
                let mut target_path = path.clone();
                target_path.push(edge.clone());
                queue.push_back((edge.target, target_path));
            }
        }
    }
    None
}

fn recursive_component_count(
    adjacency: &[Vec<usize>],
    components: &[usize],
    component_count: usize,
) -> usize {
    let mut sizes = vec![0usize; component_count];
    let mut self_edges = vec![false; component_count];
    for (source, targets) in adjacency.iter().enumerate() {
        sizes[components[source]] += 1;
        self_edges[components[source]] |= targets.contains(&source);
    }
    sizes
        .into_iter()
        .zip(self_edges)
        .filter(|(size, self_edge)| *size > 1 || *self_edge)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn condenses_self_and_mutually_recursive_components() {
        let adjacency = vec![vec![0], vec![2], vec![1], vec![]];
        let components = strongly_connected_components(&adjacency);

        assert_eq!(components[1], components[2]);
        assert_ne!(components[0], components[1]);
        assert_eq!(
            recursive_component_count(
                &adjacency,
                &components,
                components.iter().max().unwrap() + 1
            ),
            2
        );
    }
}
