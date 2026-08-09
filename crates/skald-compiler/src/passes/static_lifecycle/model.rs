//! Public, deterministic static-effect analysis model.

use crate::{
    identity::{ArrayTypeId, CallableId, ClassId, StaticFieldId},
    source::Span,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StaticClassLifecycleOperation {
    CopyConstructor,
    CopyAssignment,
    CompleteFinalizer,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StaticArrayLifecycleOperation {
    Default,
    Copy,
    Assignment,
    Destruction,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StaticEffectNode {
    Callable(CallableId),
    ClassLifecycle {
        class: ClassId,
        operation: StaticClassLifecycleOperation,
    },
    ArrayLifecycle {
        array: ArrayTypeId,
        operation: StaticArrayLifecycleOperation,
    },
}

impl StaticEffectNode {
    pub const fn callable(callable: CallableId) -> Self {
        Self::Callable(callable)
    }

    pub const fn class(class: ClassId, operation: StaticClassLifecycleOperation) -> Self {
        Self::ClassLifecycle { class, operation }
    }

    pub const fn array(array: ArrayTypeId, operation: StaticArrayLifecycleOperation) -> Self {
        Self::ArrayLifecycle { array, operation }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StaticAccessKind {
    Read,
    Write,
    Borrow,
    Initialize,
    Replace,
    Destroy,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StaticEffectPhase {
    Ordinary,
    InitializerBeforePublication,
    InitializerAfterPublication,
    Copy,
    Destruction,
    ArrayLifecycle,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StaticEffectEdgeKind {
    DirectCall,
    StaticCall,
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
    pub span: Span,
    /// Empty for a direct access. Otherwise ordered from summary root to the
    /// body or lifecycle operation containing the direct access.
    pub witness: Vec<StaticEffectEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticEffectSummary {
    pub node: StaticEffectNode,
    pub direct_effects: Vec<StaticAccessEvidence>,
    pub effects: Vec<StaticAccessEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticEffectAnalysis {
    summaries: Vec<StaticEffectSummary>,
    recursive_components: usize,
}

impl StaticEffectAnalysis {
    pub(crate) fn new(summaries: Vec<StaticEffectSummary>, recursive_components: usize) -> Self {
        Self {
            summaries,
            recursive_components,
        }
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
    (usize, usize, usize),
) {
    (
        evidence.field,
        evidence.access,
        evidence.phase,
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
