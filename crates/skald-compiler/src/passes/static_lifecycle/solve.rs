//! Recursive-component condensation, effect propagation, and witnesses.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::identity::StaticFieldId;
use crate::passes::graph::strongly_connected_components;

use super::{
    extract::{ExtractedGraph, NodeDraft},
    model::{
        edge_key, evidence_key, StaticAccessEvidence, StaticAccessKind, StaticEffectAnalysis,
        StaticEffectNode, StaticEffectPhase, StaticEffectSummary,
    },
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
                .flat_map(|field| witnesses_for(*node, *field, &graph.nodes))
                .collect();
            StaticEffectSummary {
                node: *node,
                direct_effects: draft.direct.clone(),
                possible_targets: draft.edges.clone(),
                effects,
            }
        })
        .collect();
    let recursive_components = recursive_component_count(&adjacency, &components, component_count);

    StaticEffectAnalysis::new(
        graph.function_value_candidates,
        summaries,
        recursive_components,
    )
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

fn witnesses_for(
    root: StaticEffectNode,
    field: StaticFieldId,
    drafts: &BTreeMap<StaticEffectNode, NodeDraft>,
) -> Vec<StaticAccessEvidence> {
    let mut queue = VecDeque::from([(root, None, Vec::new())]);
    let mut visited = BTreeSet::new();
    let mut representatives =
        BTreeMap::<(StaticEffectPhase, StaticAccessKind, bool), StaticAccessEvidence>::new();
    while let Some((node, root_phase, path)) = queue.pop_front() {
        if !visited.insert((node, root_phase)) {
            continue;
        }
        for direct in drafts[&node]
            .direct
            .iter()
            .filter(|effect| effect.field == field)
        {
            let mut evidence = direct.clone();
            evidence.witness.clone_from(&path);
            if let Some(phase) = root_phase {
                evidence.phase = phase;
            }
            let key = (evidence.phase, evidence.access, evidence.lifecycle_owned);
            match representatives.entry(key) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(evidence);
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    if compare_witness(&evidence, entry.get()).is_lt() {
                        entry.insert(evidence);
                    }
                }
            }
        }
        for edge in &drafts[&node].edges {
            let target_phase = root_phase.or(Some(edge.phase));
            if !visited.contains(&(edge.target, target_phase)) {
                let mut target_path = path.clone();
                target_path.push(edge.clone());
                queue.push_back((edge.target, target_phase, target_path));
            }
        }
    }
    representatives.into_values().collect()
}

fn compare_witness(
    left: &StaticAccessEvidence,
    right: &StaticAccessEvidence,
) -> std::cmp::Ordering {
    left.witness
        .len()
        .cmp(&right.witness.len())
        .then_with(|| {
            left.witness
                .iter()
                .map(|edge| (edge_key(edge), edge.phase))
                .cmp(
                    right
                        .witness
                        .iter()
                        .map(|edge| (edge_key(edge), edge.phase)),
                )
        })
        .then_with(|| evidence_key(left).cmp(&evidence_key(right)))
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
    fn counts_recursive_components_separately_from_acyclic_nodes() {
        let adjacency = vec![vec![0], vec![2], vec![1], vec![]];
        let components = strongly_connected_components(&adjacency);

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
