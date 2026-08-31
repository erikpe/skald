//! Immutable activation facts and canonical comparison helpers.

use std::cmp::Ordering;

use crate::{
    identity::StaticFieldId,
    mir::{MirExecutionNode, StaticAccessKind, StaticEffectPhase},
    passes::reachability::{mir_dependency_edge_kind_key, mir_span_key, MirDependencyEdgeKind},
    source::Span,
};

/// A node in the coupled execution/static-field activation graph.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum StaticActivationNode {
    Execution(MirExecutionNode),
    Field(StaticFieldId),
}

impl StaticActivationNode {
    pub(crate) const fn execution(node: MirExecutionNode) -> Self {
        Self::Execution(node)
    }

    pub(crate) const fn field(field: StaticFieldId) -> Self {
        Self::Field(field)
    }
}

/// Why one activation edge exists.
///
/// Execution dependencies reuse the canonical reachability vocabulary. A
/// static access is deliberately separate because it crosses from execution
/// into field lifetime policy. Initializer and destruction edges cross back
/// from an active field into executable work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StaticActivationTrigger {
    ExecutionDependency(MirDependencyEdgeKind),
    StaticAccess {
        access: StaticAccessKind,
        phase: StaticEffectPhase,
    },
    Initializer,
    Destruction,
}

/// The selected entry that starts every activation explanation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StaticActivationRoot {
    entry: MirExecutionNode,
    span: Span,
}

impl StaticActivationRoot {
    pub(crate) const fn new(entry: MirExecutionNode, span: Span) -> Self {
        Self { entry, span }
    }

    pub(crate) const fn entry(self) -> MirExecutionNode {
        self.entry
    }

    pub(crate) const fn span(self) -> Span {
        self.span
    }
}

/// One deterministic step in an activation explanation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StaticActivationEdge {
    source: StaticActivationNode,
    target: StaticActivationNode,
    trigger: StaticActivationTrigger,
    span: Span,
}

impl StaticActivationEdge {
    pub(crate) const fn execution_dependency(
        source: MirExecutionNode,
        target: MirExecutionNode,
        kind: MirDependencyEdgeKind,
        span: Span,
    ) -> Self {
        Self {
            source: StaticActivationNode::execution(source),
            target: StaticActivationNode::execution(target),
            trigger: StaticActivationTrigger::ExecutionDependency(kind),
            span,
        }
    }

    pub(crate) fn static_access(
        source: MirExecutionNode,
        target: StaticFieldId,
        access: StaticAccessKind,
        phase: StaticEffectPhase,
        span: Span,
    ) -> Self {
        debug_assert!(matches!(
            access,
            StaticAccessKind::Read
                | StaticAccessKind::Write
                | StaticAccessKind::Borrow
                | StaticAccessKind::Replace
        ));
        Self {
            source: StaticActivationNode::execution(source),
            target: StaticActivationNode::field(target),
            trigger: StaticActivationTrigger::StaticAccess { access, phase },
            span,
        }
    }

    pub(crate) const fn initializer(
        field: StaticFieldId,
        target: MirExecutionNode,
        span: Span,
    ) -> Self {
        Self {
            source: StaticActivationNode::field(field),
            target: StaticActivationNode::execution(target),
            trigger: StaticActivationTrigger::Initializer,
            span,
        }
    }

    pub(crate) const fn destruction(
        field: StaticFieldId,
        target: MirExecutionNode,
        span: Span,
    ) -> Self {
        Self {
            source: StaticActivationNode::field(field),
            target: StaticActivationNode::execution(target),
            trigger: StaticActivationTrigger::Destruction,
            span,
        }
    }

    pub(crate) const fn source(self) -> StaticActivationNode {
        self.source
    }

    pub(crate) const fn target(self) -> StaticActivationNode {
        self.target
    }

    pub(crate) const fn trigger(self) -> StaticActivationTrigger {
        self.trigger
    }

    pub(crate) const fn span(self) -> Span {
        self.span
    }
}

/// Canonical first path from the selected entry to one reached graph node.
///
/// Path order is semantic evidence and therefore remains intact. The analysis
/// solver is responsible for choosing the canonical path; this model merely
/// exposes it immutably.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StaticActivationWitness {
    root: StaticActivationRoot,
    edges: Vec<StaticActivationEdge>,
}

impl StaticActivationWitness {
    pub(super) const fn new(root: StaticActivationRoot, edges: Vec<StaticActivationEdge>) -> Self {
        Self { root, edges }
    }

    pub(crate) const fn root(&self) -> StaticActivationRoot {
        self.root
    }

    pub(crate) fn edges(&self) -> &[StaticActivationEdge] {
        &self.edges
    }

    pub(crate) fn target(&self) -> StaticActivationNode {
        self.edges.last().map_or_else(
            || StaticActivationNode::execution(self.root.entry()),
            |edge| edge.target(),
        )
    }

    pub(super) fn is_contiguous(&self) -> bool {
        let mut expected = StaticActivationNode::execution(self.root.entry());
        for edge in &self.edges {
            if edge.source() != expected {
                return false;
            }
            expected = edge.target();
        }
        true
    }
}

/// One active field and its canonical first activation explanation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StaticActivationField {
    field: StaticFieldId,
    witness: StaticActivationWitness,
}

impl StaticActivationField {
    pub(super) fn new(field: StaticFieldId, witness: StaticActivationWitness) -> Self {
        debug_assert_eq!(witness.target(), StaticActivationNode::field(field));
        debug_assert!(witness.is_contiguous());
        Self { field, witness }
    }

    pub(crate) const fn field(&self) -> StaticFieldId {
        self.field
    }

    pub(crate) const fn witness(&self) -> &StaticActivationWitness {
        &self.witness
    }

    pub(crate) fn first_trigger(&self) -> StaticActivationEdge {
        *self
            .witness
            .edges()
            .last()
            .expect("an active field must have one activation access")
    }
}

/// One activation-reachable execution node and its canonical explanation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StaticActivationExecution {
    node: MirExecutionNode,
    witness: StaticActivationWitness,
}

impl StaticActivationExecution {
    pub(super) fn new(node: MirExecutionNode, witness: StaticActivationWitness) -> Self {
        debug_assert_eq!(witness.target(), StaticActivationNode::execution(node));
        debug_assert!(witness.is_contiguous());
        Self { node, witness }
    }

    pub(crate) const fn node(&self) -> MirExecutionNode {
        self.node
    }

    pub(crate) const fn witness(&self) -> &StaticActivationWitness {
        &self.witness
    }
}

/// Stable counts derived once from an immutable activation result.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct StaticActivationCounts {
    pub(crate) declared_fields: usize,
    pub(crate) active_fields: usize,
    pub(crate) inactive_fields: usize,
    pub(crate) reachable_execution_nodes: usize,
    pub(crate) edges: usize,
    pub(crate) static_accesses: usize,
    pub(crate) execution_dependencies: usize,
    pub(crate) initializer_roots: usize,
    pub(crate) destruction_roots: usize,
}

/// Immutable planning-only activation facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StaticActivationAnalysis {
    active_fields: Vec<StaticActivationField>,
    inactive_fields: Vec<StaticFieldId>,
    reachable_execution: Vec<StaticActivationExecution>,
    edges: Vec<StaticActivationEdge>,
    counts: StaticActivationCounts,
}

pub(crate) struct StaticActivationAnalysisParts {
    pub(super) active_fields: Vec<StaticActivationField>,
    pub(super) inactive_fields: Vec<StaticFieldId>,
    pub(super) reachable_execution: Vec<StaticActivationExecution>,
    pub(super) edges: Vec<StaticActivationEdge>,
}

impl StaticActivationAnalysis {
    pub(super) fn from_parts(mut parts: StaticActivationAnalysisParts) -> Self {
        parts.active_fields.sort_by(compare_active_fields);
        parts
            .active_fields
            .dedup_by(|left, right| left.field == right.field);
        parts
            .inactive_fields
            .sort_unstable_by_key(|field| field_key(*field));
        parts.inactive_fields.dedup();
        parts.reachable_execution.sort_by(compare_execution);
        parts
            .reachable_execution
            .dedup_by(|left, right| left.node == right.node);
        parts.edges.sort_unstable_by_key(static_activation_edge_key);
        parts.edges.dedup();

        debug_assert!(parts.active_fields.iter().all(|active| {
            parts
                .inactive_fields
                .binary_search_by_key(&field_key(active.field()), |field| field_key(*field))
                .is_err()
        }));

        let counts = counts_for(&parts);
        Self {
            active_fields: parts.active_fields,
            inactive_fields: parts.inactive_fields,
            reachable_execution: parts.reachable_execution,
            edges: parts.edges,
            counts,
        }
    }

    pub(crate) fn active_fields(&self) -> &[StaticActivationField] {
        &self.active_fields
    }

    pub(crate) fn inactive_fields(&self) -> &[StaticFieldId] {
        &self.inactive_fields
    }

    pub(crate) fn is_active(&self, field: StaticFieldId) -> bool {
        self.active_fields
            .binary_search_by_key(&field_key(field), |active| field_key(active.field))
            .is_ok()
    }

    pub(crate) fn field(&self, field: StaticFieldId) -> Option<&StaticActivationField> {
        self.active_fields
            .binary_search_by_key(&field_key(field), |active| field_key(active.field))
            .ok()
            .map(|index| &self.active_fields[index])
    }

    pub(crate) fn reachable_execution(&self) -> &[StaticActivationExecution] {
        &self.reachable_execution
    }

    pub(crate) fn is_execution_reachable(&self, node: MirExecutionNode) -> bool {
        self.reachable_execution
            .binary_search_by_key(&crate::mir::mir_execution_node_key(node), |execution| {
                crate::mir::mir_execution_node_key(execution.node)
            })
            .is_ok()
    }

    pub(crate) fn execution(&self, node: MirExecutionNode) -> Option<&StaticActivationExecution> {
        self.reachable_execution
            .binary_search_by_key(&crate::mir::mir_execution_node_key(node), |execution| {
                crate::mir::mir_execution_node_key(execution.node)
            })
            .ok()
            .map(|index| &self.reachable_execution[index])
    }

    pub(crate) fn edges(&self) -> &[StaticActivationEdge] {
        &self.edges
    }

    pub(crate) const fn counts(&self) -> StaticActivationCounts {
        self.counts
    }
}

pub(crate) type StaticActivationNodeKey = (u8, u8, usize, usize, usize);

/// Canonical node ordering shared by future worklists, results, and dumps.
pub(crate) const fn static_activation_node_key(
    node: StaticActivationNode,
) -> StaticActivationNodeKey {
    match node {
        StaticActivationNode::Execution(node) => {
            let (kind, first, second, third) = crate::mir::mir_execution_node_key(node);
            (0, kind, first, second, third)
        }
        StaticActivationNode::Field(field) => (1, 0, field.class().index(), field.index(), 0),
    }
}

pub(crate) type StaticActivationTriggerKey = (u8, u8, u8);

const fn trigger_key(trigger: StaticActivationTrigger) -> StaticActivationTriggerKey {
    match trigger {
        StaticActivationTrigger::ExecutionDependency(kind) => {
            (0, mir_dependency_edge_kind_key(kind), 0)
        }
        StaticActivationTrigger::StaticAccess { access, phase } => {
            (1, static_access_key(access), static_phase_key(phase))
        }
        StaticActivationTrigger::Initializer => (2, 0, 0),
        StaticActivationTrigger::Destruction => (3, 0, 0),
    }
}

const fn static_access_key(access: StaticAccessKind) -> u8 {
    match access {
        StaticAccessKind::Read => 0,
        StaticAccessKind::Write => 1,
        StaticAccessKind::Borrow => 2,
        StaticAccessKind::Initialize => 3,
        StaticAccessKind::Replace => 4,
        StaticAccessKind::Destroy => 5,
    }
}

const fn static_phase_key(phase: StaticEffectPhase) -> u8 {
    match phase {
        StaticEffectPhase::Ordinary => 0,
        StaticEffectPhase::InitializerBeforePublication => 1,
        StaticEffectPhase::InitializerAfterPublication => 2,
        StaticEffectPhase::Copy => 3,
        StaticEffectPhase::Destruction => 4,
        StaticEffectPhase::ArrayLifecycle => 5,
    }
}

pub(crate) type StaticActivationEdgeKey = (
    StaticActivationNodeKey,
    StaticActivationTriggerKey,
    StaticActivationNodeKey,
    (usize, usize, usize),
);

/// Canonical edge ordering independent of graph storage or worklist strategy.
pub(crate) const fn static_activation_edge_key(
    edge: &StaticActivationEdge,
) -> StaticActivationEdgeKey {
    (
        static_activation_node_key(edge.source),
        trigger_key(edge.trigger),
        static_activation_node_key(edge.target),
        mir_span_key(edge.span),
    )
}

const fn field_key(field: StaticFieldId) -> (usize, usize) {
    (field.class().index(), field.index())
}

fn compare_witness(left: &StaticActivationWitness, right: &StaticActivationWitness) -> Ordering {
    let root_order = crate::mir::mir_execution_node_key(left.root.entry())
        .cmp(&crate::mir::mir_execution_node_key(right.root.entry()))
        .then_with(|| mir_span_key(left.root.span()).cmp(&mir_span_key(right.root.span())));
    root_order.then_with(|| {
        left.edges
            .iter()
            .map(static_activation_edge_key)
            .cmp(right.edges.iter().map(static_activation_edge_key))
    })
}

fn compare_active_fields(left: &StaticActivationField, right: &StaticActivationField) -> Ordering {
    field_key(left.field)
        .cmp(&field_key(right.field))
        .then_with(|| compare_witness(&left.witness, &right.witness))
}

fn compare_execution(
    left: &StaticActivationExecution,
    right: &StaticActivationExecution,
) -> Ordering {
    crate::mir::mir_execution_node_key(left.node)
        .cmp(&crate::mir::mir_execution_node_key(right.node))
        .then_with(|| compare_witness(&left.witness, &right.witness))
}

fn counts_for(parts: &StaticActivationAnalysisParts) -> StaticActivationCounts {
    let mut counts = StaticActivationCounts {
        declared_fields: parts.active_fields.len() + parts.inactive_fields.len(),
        active_fields: parts.active_fields.len(),
        inactive_fields: parts.inactive_fields.len(),
        reachable_execution_nodes: parts.reachable_execution.len(),
        edges: parts.edges.len(),
        ..StaticActivationCounts::default()
    };
    for edge in &parts.edges {
        match edge.trigger() {
            StaticActivationTrigger::ExecutionDependency(_) => {
                counts.execution_dependencies += 1;
            }
            StaticActivationTrigger::StaticAccess { .. } => counts.static_accesses += 1,
            StaticActivationTrigger::Initializer => counts.initializer_roots += 1,
            StaticActivationTrigger::Destruction => counts.destruction_roots += 1,
        }
    }
    counts
}
