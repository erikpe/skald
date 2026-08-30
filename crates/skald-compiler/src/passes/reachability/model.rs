//! Semantic identities and deterministic evidence for reachability analysis.

use crate::{
    identity::{
        CallableId, ClassId, ExternalLinkId, FunctionTypeId, InterfaceId, InterfaceRequirementId,
        LiteralDataId, OptionalBoxTypeId, OptionalTypeId, StaticFieldId, VirtualFamilyId,
    },
    intrinsic::Intrinsic,
    mir::MirExecutionNode,
    source::Span,
};

/// Why the whole-program contract makes an obligation live without a caller.
///
/// The reason is separate from the target so root policy can evolve without
/// changing execution-node identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirReachabilityRootReason {
    Entry,
    StaticActivation(StaticFieldId),
    StaticShutdown(StaticFieldId),
}

/// Semantic metadata or data required by reachable executable work.
///
/// These identities are not callable nodes and do not imply that a declaration
/// or executable body is physically retained. Target symbols and layout
/// choices deliberately remain backend concerns.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirRuntimeEntity {
    ClassDispatch(ClassId),
    VirtualFamily(VirtualFamilyId),
    InterfaceRequirement(InterfaceRequirementId),
    FunctionType(FunctionTypeId),
    ArrayLifecycle(crate::identity::ArrayTypeId),
    OptionalLifecycle(OptionalTypeId),
    OptionalBoxLayout(OptionalBoxTypeId),
    StaticStorage(StaticFieldId),
    LiteralBacking(LiteralDataId),
}

/// A stable semantic declaration retained in the closed-world program model.
///
/// Reachability may eventually remove a definition without removing the
/// declaration named here. This initial vocabulary names declaration kinds
/// needed to express that distinction; it is not a global metadata-pruning
/// API.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirSemanticDeclaration {
    Callable(CallableId),
    Class(ClassId),
    Interface(InterfaceId),
    StaticField(StaticFieldId),
}

/// Identity of a callable body physically present in one MIR product.
///
/// This wrapper records a retention fact. It grants no mutation capability and
/// is deliberately distinct from both a declaration and an execution node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MirRetainedDefinition(CallableId);

impl MirRetainedDefinition {
    pub(crate) const fn new(callable: CallableId) -> Self {
        Self(callable)
    }

    pub(crate) const fn callable(self) -> CallableId {
        self.0
    }

    pub(crate) const fn execution_node(self) -> MirExecutionNode {
        MirExecutionNode::Callable(self.0)
    }
}

/// A target selected by one target-independent dependency.
///
/// External and intrinsic functions are typed leaves: they may be called but
/// have no internal Skald definition. Runtime entities remain distinct from
/// executable targets.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirDependencyTarget {
    Execution(MirExecutionNode),
    RuntimeEntity(MirRuntimeEntity),
    External(ExternalLinkId),
    Intrinsic(Intrinsic),
}

/// Semantic cause of a target-independent dependency.
///
/// Static-effect phases, source evidence, target symbols, and presentation
/// text are not part of this identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirDependencyEdgeKind {
    DirectCall,
    StaticCall,
    DirectMethodCall,
    VirtualDispatch,
    InterfaceDispatch,
    CallableAddressRetention,
    IndirectCall,
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
    OptionalLifecycle,
    ArrayDefault,
    ArrayCopy,
    ArrayAssignment,
    ArrayDestruction,
    RuntimeEntityReference,
}

/// One dependency and its deterministic source evidence.
///
/// The span helps dumps and witnesses but is not part of either endpoint's
/// semantic identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MirDependencyEdge {
    source: MirExecutionNode,
    target: MirDependencyTarget,
    kind: MirDependencyEdgeKind,
    span: Span,
}

impl MirDependencyEdge {
    pub(crate) const fn new(
        source: MirExecutionNode,
        target: MirDependencyTarget,
        kind: MirDependencyEdgeKind,
        span: Span,
    ) -> Self {
        Self {
            source,
            target,
            kind,
            span,
        }
    }

    pub(crate) const fn source(&self) -> MirExecutionNode {
        self.source
    }

    pub(crate) const fn target(&self) -> MirDependencyTarget {
        self.target
    }

    pub(crate) const fn kind(&self) -> MirDependencyEdgeKind {
        self.kind
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

/// The kind of obligation named by a whole-program root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirReachabilityRootTarget {
    Execution(MirExecutionNode),
    RuntimeEntity(MirRuntimeEntity),
}

/// One explicit whole-program root and the evidence that selected it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MirReachabilityRoot {
    target: MirReachabilityRootTarget,
    reason: MirReachabilityRootReason,
    span: Span,
}

impl MirReachabilityRoot {
    pub(crate) const fn new(
        target: MirReachabilityRootTarget,
        reason: MirReachabilityRootReason,
        span: Span,
    ) -> Self {
        Self {
            target,
            reason,
            span,
        }
    }

    pub(crate) const fn target(&self) -> MirReachabilityRootTarget {
        self.target
    }

    pub(crate) const fn reason(&self) -> MirReachabilityRootReason {
        self.reason
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

/// Canonical ordering of edge semantics, independent of graph storage.
pub(crate) const fn mir_dependency_edge_kind_key(kind: MirDependencyEdgeKind) -> u8 {
    match kind {
        MirDependencyEdgeKind::DirectCall => 0,
        MirDependencyEdgeKind::StaticCall => 1,
        MirDependencyEdgeKind::DirectMethodCall => 2,
        MirDependencyEdgeKind::VirtualDispatch => 3,
        MirDependencyEdgeKind::InterfaceDispatch => 4,
        MirDependencyEdgeKind::CallableAddressRetention => 5,
        MirDependencyEdgeKind::IndirectCall => 6,
        MirDependencyEdgeKind::Initializer => 7,
        MirDependencyEdgeKind::CopyConstructor => 8,
        MirDependencyEdgeKind::CopyAssignment => 9,
        MirDependencyEdgeKind::UserCopyBody => 10,
        MirDependencyEdgeKind::BaseCopy => 11,
        MirDependencyEdgeKind::FieldCopy => 12,
        MirDependencyEdgeKind::CompleteFinalizer => 13,
        MirDependencyEdgeKind::UserDestructor => 14,
        MirDependencyEdgeKind::FieldFinalizer => 15,
        MirDependencyEdgeKind::BaseFinalizer => 16,
        MirDependencyEdgeKind::SharedFinalizer => 17,
        MirDependencyEdgeKind::TemporaryCleanup => 18,
        MirDependencyEdgeKind::OptionalLifecycle => 19,
        MirDependencyEdgeKind::ArrayDefault => 20,
        MirDependencyEdgeKind::ArrayCopy => 21,
        MirDependencyEdgeKind::ArrayAssignment => 22,
        MirDependencyEdgeKind::ArrayDestruction => 23,
        MirDependencyEdgeKind::RuntimeEntityReference => 24,
    }
}

/// Canonical ordering of whole-program root policy.
pub(crate) const fn mir_reachability_root_reason_key(
    reason: MirReachabilityRootReason,
) -> (u8, usize, usize) {
    match reason {
        MirReachabilityRootReason::Entry => (0, 0, 0),
        MirReachabilityRootReason::StaticActivation(field) => {
            (1, field.class().index(), field.index())
        }
        MirReachabilityRootReason::StaticShutdown(field) => {
            (2, field.class().index(), field.index())
        }
    }
}

/// Canonical source ordering shared by dependency analyses and dumps.
pub(crate) const fn mir_span_key(span: Span) -> (usize, usize, usize) {
    (
        span.source_id().index(),
        span.range().start(),
        span.range().end(),
    )
}
