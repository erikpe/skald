//! Static-effect analysis evidence and deterministic comparison helpers.

use crate::identity::{CallableId, FunctionTypeId};
pub(crate) use crate::mir::{
    StaticAccessKind, StaticArrayLifecycleOperation, StaticClassLifecycleOperation,
    StaticEffectNode, StaticEffectPhase,
};

pub(crate) use crate::passes::reachability::mir_span_key as span_key;
use crate::{identity::StaticFieldId, source::Span};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StaticEffectEdgeKind {
    DirectCall,
    StaticCall,
    IndirectCall,
    DirectMethodCall,
    VirtualDispatch,
    InterfaceDispatch,
    Initializer,
    CopyConstructor,
    CopyAssignment,
    UserCopyBody,
    BaseCopy,
    FieldCopy,
    CompleteFinalizer,
    UserDestructor,
    FieldFinalizer,
    BaseFinalizer,
    SharedFinalizer,
    TemporaryCleanup,
    OptionalCleanup,
    ArrayDefault,
    ArrayCopy,
    ArrayAssignment,
    ArrayDestruction,
}

/// One exact internal target retained because a callable-address operation
/// forms its function value somewhere in the closed program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticFunctionValueTarget {
    pub callable: CallableId,
    pub first_reference_span: Span,
}

/// The deterministic address-taken target set for one exact function type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticFunctionValueCandidates {
    pub function_type: FunctionTypeId,
    pub targets: Vec<StaticFunctionValueTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticEffectEdge {
    pub source: StaticEffectNode,
    pub target: StaticEffectNode,
    pub kind: StaticEffectEdgeKind,
    pub phase: StaticEffectPhase,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticAccessEvidence {
    pub field: StaticFieldId,
    pub access: StaticAccessKind,
    pub phase: StaticEffectPhase,
    /// True only for the unpublished destination root owned by this field's
    /// initializer. Ordinary static-place accesses always set this to false.
    pub lifecycle_owned: bool,
    pub span: Span,
    /// Empty for a direct access. Otherwise ordered from summary root to the
    /// body or lifecycle operation containing the direct access.
    pub witness: Vec<StaticEffectEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticEffectSummary {
    pub node: StaticEffectNode,
    pub direct_effects: Vec<StaticAccessEvidence>,
    pub possible_targets: Vec<StaticEffectEdge>,
    pub effects: Vec<StaticAccessEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticEffectAnalysis {
    function_value_candidates: Vec<StaticFunctionValueCandidates>,
    summaries: Vec<StaticEffectSummary>,
    recursive_components: usize,
}

impl StaticEffectAnalysis {
    pub(crate) fn new(
        function_value_candidates: Vec<StaticFunctionValueCandidates>,
        summaries: Vec<StaticEffectSummary>,
        recursive_components: usize,
    ) -> Self {
        Self {
            function_value_candidates,
            summaries,
            recursive_components,
        }
    }

    pub fn function_value_candidates(
        &self,
    ) -> impl ExactSizeIterator<Item = &StaticFunctionValueCandidates> {
        self.function_value_candidates.iter()
    }

    pub fn function_value_candidates_for(
        &self,
        function_type: FunctionTypeId,
    ) -> Option<&StaticFunctionValueCandidates> {
        self.function_value_candidates
            .binary_search_by_key(&function_type, |candidates| candidates.function_type)
            .ok()
            .map(|index| &self.function_value_candidates[index])
    }

    pub fn summaries(&self) -> impl ExactSizeIterator<Item = &StaticEffectSummary> {
        self.summaries.iter()
    }

    pub fn summary(&self, node: StaticEffectNode) -> Option<&StaticEffectSummary> {
        self.summaries
            .binary_search_by_key(&node, |summary| summary.node)
            .ok()
            .map(|index| &self.summaries[index])
    }

    pub const fn recursive_components(&self) -> usize {
        self.recursive_components
    }
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
