//! Static-effect model facade and deterministic comparison helpers.

pub use crate::mir::{
    StaticAccessEvidence, StaticAccessKind, StaticArrayLifecycleOperation,
    StaticClassLifecycleOperation, StaticEffectAnalysis, StaticEffectEdge, StaticEffectEdgeKind,
    StaticEffectNode, StaticEffectPhase, StaticEffectSummary, StaticFunctionValueCandidates,
    StaticFunctionValueTarget,
};

use crate::{identity::StaticFieldId, source::Span};

pub(crate) fn span_key(span: Span) -> (usize, usize, usize) {
    (
        span.source_id().index(),
        span.range().start(),
        span.range().end(),
    )
}

pub(crate) fn evidence_key(
    evidence: &StaticAccessEvidence,
) -> (
    StaticFieldId,
    StaticAccessKind,
    StaticEffectPhase,
    bool,
    (usize, usize, usize),
) {
    (
        evidence.field,
        evidence.access,
        evidence.phase,
        evidence.lifecycle_owned,
        span_key(evidence.span),
    )
}

pub(crate) fn edge_key(
    edge: &StaticEffectEdge,
) -> (
    StaticEffectNode,
    StaticEffectEdgeKind,
    (usize, usize, usize),
) {
    (edge.target, edge.kind, span_key(edge.span))
}
