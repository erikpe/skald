//! Normalized lifecycle-root effect model.

use std::collections::BTreeSet;

use crate::{
    identity::StaticFieldId,
    mir::{
        PreliminaryMirProgram, StaticAccessEvidence, StaticAccessKind, StaticEffectNode,
        StaticEffectPhase,
    },
};

use super::super::roots::{destruction_roots, is_lifecycle_destination_or_published_self_parts};

/// One semantic static effect authorized for a lifecycle root.
///
/// Evidence location and graph shape are intentionally absent: they may change
/// when final MIR is optimized without changing lifecycle safety.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StaticLifecycleEffectFact {
    pub(crate) target: StaticFieldId,
    pub(crate) access: StaticAccessKind,
    pub(crate) phase: StaticEffectPhase,
    pub(crate) lifecycle_owned: bool,
}

impl StaticLifecycleEffectFact {
    pub(crate) fn from_evidence(
        evidence: &StaticAccessEvidence,
        root_phase: Option<StaticEffectPhase>,
    ) -> Self {
        Self {
            target: evidence.field,
            access: evidence.access,
            phase: root_phase.unwrap_or(evidence.phase),
            lifecycle_owned: evidence.lifecycle_owned,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StaticLifecycleRootEffectSummary {
    pub(crate) root: StaticEffectNode,
    pub(crate) effects: Vec<StaticLifecycleEffectFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StaticLifecycleRootEffectAnalysis {
    summaries: Vec<StaticLifecycleRootEffectSummary>,
}

impl StaticLifecycleRootEffectAnalysis {
    pub(super) fn new(summaries: Vec<StaticLifecycleRootEffectSummary>) -> Self {
        Self { summaries }
    }

    pub(crate) fn summaries(
        &self,
    ) -> impl ExactSizeIterator<Item = &StaticLifecycleRootEffectSummary> {
        self.summaries.iter()
    }

    pub(crate) fn summary(
        &self,
        root: StaticEffectNode,
    ) -> Option<&StaticLifecycleRootEffectSummary> {
        self.summaries
            .binary_search_by_key(&root, |summary| summary.root)
            .ok()
            .map(|index| &self.summaries[index])
    }

    pub(crate) fn dependency_pairs(
        &self,
        program: &PreliminaryMirProgram,
    ) -> BTreeSet<(StaticFieldId, StaticFieldId)> {
        let mut dependencies = BTreeSet::new();
        for root in lifecycle_root_uses(program) {
            let summary = self
                .summary(root.node)
                .expect("inventoried lifecycle root must have normalized effects");
            for effect in &summary.effects {
                if root.kind == LifecycleRootKind::Initialization
                    && is_lifecycle_destination_or_published_self_parts(
                        root.owner,
                        effect.target,
                        effect.access,
                        effect.phase,
                        effect.lifecycle_owned,
                    )
                {
                    continue;
                }
                dependencies.insert((effect.target, root.owner));
            }
        }
        dependencies
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum LifecycleRootKind {
    Initialization,
    Destruction,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct LifecycleRootUse {
    pub(super) owner: StaticFieldId,
    pub(super) kind: LifecycleRootKind,
    pub(super) node: StaticEffectNode,
}

pub(super) fn lifecycle_root_uses(program: &PreliminaryMirProgram) -> Vec<LifecycleRootUse> {
    let mut roots = Vec::new();
    for field in program.static_fields() {
        if let Some(initializer) = field.initializer {
            roots.push(LifecycleRootUse {
                owner: field.field,
                kind: LifecycleRootKind::Initialization,
                node: StaticEffectNode::callable(initializer.into()),
            });
        }
        roots.extend(
            destruction_roots(program.program(), field.ty)
                .into_iter()
                .map(|node| LifecycleRootUse {
                    owner: field.field,
                    kind: LifecycleRootKind::Destruction,
                    node,
                }),
        );
    }
    roots.sort_unstable();
    roots.dedup();
    roots
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StaticLifecycleRootEffectError {
    MissingRoot(StaticEffectNode),
    ForeignEdgeSource {
        node: StaticEffectNode,
        source: StaticEffectNode,
    },
    ForeignEdgeTarget {
        source: StaticEffectNode,
        target: StaticEffectNode,
    },
    ForeignStaticField {
        node: StaticEffectNode,
        field: StaticFieldId,
    },
}
