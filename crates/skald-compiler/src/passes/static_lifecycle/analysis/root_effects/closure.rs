//! Independent analysis closure for normalized lifecycle-root effects.

use std::collections::{BTreeSet, VecDeque};

use crate::mir::{
    MirProgram, MirStaticLifecycleDefinition, PreliminaryMirProgram, StaticEffectNode,
    StaticLifecycleAuthority, StaticLifecycleEffectFact, StaticLifecycleRootAuthority,
};

use super::{
    super::extract::ExtractedGraph,
    model::{
        lifecycle_root_uses_for_definitions, lifecycle_root_uses_for_fields,
        StaticLifecycleRootEffectError,
    },
};

#[cfg(test)]
use super::model::lifecycle_root_uses;

#[cfg(test)]
pub(crate) fn analyze(
    program: &PreliminaryMirProgram,
    graph: &ExtractedGraph,
) -> Result<StaticLifecycleAuthority, StaticLifecycleRootEffectError> {
    let declared_fields = program
        .static_fields()
        .map(|field| field.field)
        .collect::<BTreeSet<_>>();
    let roots = lifecycle_root_uses(program)
        .into_iter()
        .map(|root| root.node)
        .collect::<BTreeSet<_>>();
    analyze_roots(graph, declared_fields, roots)
}

pub(crate) fn analyze_for_fields(
    program: &PreliminaryMirProgram,
    graph: &ExtractedGraph,
    active_fields: &[crate::identity::StaticFieldId],
) -> Result<StaticLifecycleAuthority, StaticLifecycleRootEffectError> {
    let declared_fields = program
        .static_fields()
        .map(|field| field.field)
        .collect::<BTreeSet<_>>();
    let active_fields = active_fields.iter().copied().collect::<BTreeSet<_>>();
    let roots = lifecycle_root_uses_for_fields(program, &active_fields)
        .into_iter()
        .map(|root| root.node)
        .collect::<BTreeSet<_>>();
    analyze_roots(graph, declared_fields, roots)
}

pub(crate) fn analyze_final(
    program: &MirProgram,
    definitions: &[MirStaticLifecycleDefinition],
    graph: &ExtractedGraph,
) -> Result<StaticLifecycleAuthority, StaticLifecycleRootEffectError> {
    let declared_fields = program
        .classes
        .iter()
        .flat_map(|class| class.static_fields.iter().map(|field| field.id))
        .collect::<BTreeSet<_>>();
    let roots = lifecycle_root_uses_for_definitions(program, definitions)
        .into_iter()
        .map(|root| root.node)
        .collect::<BTreeSet<_>>();
    analyze_roots(graph, declared_fields, roots)
}

fn analyze_roots(
    graph: &ExtractedGraph,
    declared_fields: BTreeSet<crate::identity::StaticFieldId>,
    roots: BTreeSet<StaticEffectNode>,
) -> Result<StaticLifecycleAuthority, StaticLifecycleRootEffectError> {
    let summaries = roots
        .into_iter()
        .map(|root| {
            effects_for(root, graph, &declared_fields).map(|effects| {
                StaticLifecycleRootAuthority::new(root, effects.into_iter().collect())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(StaticLifecycleAuthority::new(summaries))
}

fn effects_for(
    root: StaticEffectNode,
    graph: &ExtractedGraph,
    declared_fields: &BTreeSet<crate::identity::StaticFieldId>,
) -> Result<BTreeSet<StaticLifecycleEffectFact>, StaticLifecycleRootEffectError> {
    if !graph.nodes.contains_key(&root) {
        return Err(StaticLifecycleRootEffectError::MissingRoot(root));
    }

    let mut effects = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut pending = VecDeque::from([(root, None)]);
    while let Some((node, root_phase)) = pending.pop_front() {
        if !visited.insert((node, root_phase)) {
            continue;
        }
        let draft =
            graph
                .nodes
                .get(&node)
                .ok_or(StaticLifecycleRootEffectError::ForeignEdgeTarget {
                    source: root,
                    target: node,
                })?;
        for direct in &draft.direct {
            if !declared_fields.contains(&direct.field) {
                return Err(StaticLifecycleRootEffectError::ForeignStaticField {
                    node,
                    field: direct.field,
                });
            }
            effects.insert(StaticLifecycleEffectFact::new(
                direct.field,
                direct.access,
                root_phase.unwrap_or(direct.phase),
                direct.lifecycle_owned,
            ));
        }
        for edge in &draft.edges {
            if edge.source != node {
                return Err(StaticLifecycleRootEffectError::ForeignEdgeSource {
                    node,
                    source: edge.source,
                });
            }
            if !graph.nodes.contains_key(&edge.target) {
                return Err(StaticLifecycleRootEffectError::ForeignEdgeTarget {
                    source: node,
                    target: edge.target,
                });
            }
            let target_phase = root_phase.or(Some(edge.phase));
            if !visited.contains(&(edge.target, target_phase)) {
                pending.push_back((edge.target, target_phase));
            }
        }
    }
    Ok(effects)
}
