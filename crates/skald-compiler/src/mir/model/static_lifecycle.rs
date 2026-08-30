//! Static-lifecycle MIR schema and compact whole-program proof.

use std::fmt;

use crate::{
    identity::{
        ArrayTypeId, CallableId, ClassId, FunctionTypeId, StaticFieldId, StaticInitializerId,
    },
    source::Span,
};

use super::{
    MirArrayInstruction, MirClassOptionalCleanup, MirCleanup, MirOptionalSharedCleanup, MirPlace,
    MirSharedTarget, MirStaticInitializerBody, MirType,
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

/// One semantic static effect authorized for a lifecycle root.
///
/// Source locations, witness paths, directness, edge kinds, and intermediate
/// nodes are intentionally excluded because they may change without changing
/// static-lifecycle safety.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StaticLifecycleEffectFact {
    target: StaticFieldId,
    access: StaticAccessKind,
    phase: StaticEffectPhase,
    lifecycle_owned: bool,
}

impl StaticLifecycleEffectFact {
    pub(crate) const fn new(
        target: StaticFieldId,
        access: StaticAccessKind,
        phase: StaticEffectPhase,
        lifecycle_owned: bool,
    ) -> Self {
        Self {
            target,
            access,
            phase,
            lifecycle_owned,
        }
    }

    pub(crate) fn from_evidence(
        evidence: &StaticAccessEvidence,
        root_phase: Option<StaticEffectPhase>,
    ) -> Self {
        Self::new(
            evidence.field,
            evidence.access,
            root_phase.unwrap_or(evidence.phase),
            evidence.lifecycle_owned,
        )
    }

    pub const fn target(&self) -> StaticFieldId {
        self.target
    }

    pub const fn access(&self) -> StaticAccessKind {
        self.access
    }

    pub const fn phase(&self) -> StaticEffectPhase {
        self.phase
    }

    pub const fn is_lifecycle_owned(&self) -> bool {
        self.lifecycle_owned
    }

    #[cfg(test)]
    pub(crate) fn set_target_for_test(&mut self, target: StaticFieldId) {
        self.target = target;
    }

    #[cfg(test)]
    pub(crate) fn set_access_for_test(&mut self, access: StaticAccessKind) {
        self.access = access;
    }

    #[cfg(test)]
    pub(crate) fn set_phase_for_test(&mut self, phase: StaticEffectPhase) {
        self.phase = phase;
    }

    #[cfg(test)]
    pub(crate) fn set_lifecycle_owned_for_test(&mut self, lifecycle_owned: bool) {
        self.lifecycle_owned = lifecycle_owned;
    }
}

/// The exact normalized effects authorized for one lifecycle root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLifecycleRootAuthority {
    root: StaticEffectNode,
    effects: Vec<StaticLifecycleEffectFact>,
}

impl StaticLifecycleRootAuthority {
    pub(crate) fn new(root: StaticEffectNode, mut effects: Vec<StaticLifecycleEffectFact>) -> Self {
        effects.sort_unstable();
        effects.dedup();
        Self { root, effects }
    }

    pub const fn root(&self) -> StaticEffectNode {
        self.root
    }

    pub fn effects(&self) -> &[StaticLifecycleEffectFact] {
        &self.effects
    }

    #[cfg(test)]
    pub(crate) fn set_root_for_test(&mut self, root: StaticEffectNode) {
        self.root = root;
    }

    #[cfg(test)]
    pub(crate) fn effects_mut_for_test(&mut self) -> &mut Vec<StaticLifecycleEffectFact> {
        &mut self.effects
    }
}

/// Immutable baseline authority issued from verified preliminary MIR.
///
/// Roots and their fact sets are stored in deterministic sorted, unique order.
/// Public consumers can inspect the authority but cannot construct or mutate
/// it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLifecycleAuthority {
    roots: Vec<StaticLifecycleRootAuthority>,
}

impl StaticLifecycleAuthority {
    pub(crate) fn new(roots: Vec<StaticLifecycleRootAuthority>) -> Self {
        let mut by_root = std::collections::BTreeMap::<
            StaticEffectNode,
            std::collections::BTreeSet<StaticLifecycleEffectFact>,
        >::new();
        for root in roots {
            by_root.entry(root.root).or_default().extend(root.effects);
        }
        Self {
            roots: by_root
                .into_iter()
                .map(|(root, effects)| {
                    StaticLifecycleRootAuthority::new(root, effects.into_iter().collect())
                })
                .collect(),
        }
    }

    pub fn roots(&self) -> impl ExactSizeIterator<Item = &StaticLifecycleRootAuthority> {
        self.roots.iter()
    }

    pub fn root(&self, root: StaticEffectNode) -> Option<&StaticLifecycleRootAuthority> {
        self.roots
            .binary_search_by_key(&root, StaticLifecycleRootAuthority::root)
            .ok()
            .map(|index| &self.roots[index])
    }

    #[cfg(test)]
    pub(crate) fn roots_mut_for_test(&mut self) -> &mut Vec<StaticLifecycleRootAuthority> {
        &mut self.roots
    }
}

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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StaticLifetimePhase {
    Initialization,
    Destruction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLifetimeEvidence {
    pub root: StaticFieldId,
    pub root_span: Span,
    pub phase: StaticLifetimePhase,
    pub root_effect: StaticEffectNode,
    pub target: StaticFieldId,
    pub target_span: Span,
    pub access: StaticAccessKind,
    pub effect_phase: StaticEffectPhase,
    pub access_span: Span,
    pub witness: Vec<StaticEffectEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLifetimeDependency {
    /// The field that must be live before `dependent` is activated.
    pub prerequisite: StaticFieldId,
    /// The field whose initialization or destruction reaches `prerequisite`.
    pub dependent: StaticFieldId,
    pub evidence: StaticLifetimeEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLifecyclePlan {
    activation: Vec<StaticFieldId>,
    shutdown: Vec<StaticFieldId>,
}

impl StaticLifecyclePlan {
    pub(crate) fn new(activation: Vec<StaticFieldId>) -> Self {
        let shutdown = activation.iter().rev().copied().collect();
        Self {
            activation,
            shutdown,
        }
    }

    pub fn activation(&self) -> &[StaticFieldId] {
        &self.activation
    }

    pub fn shutdown(&self) -> &[StaticFieldId] {
        &self.shutdown
    }

    #[cfg(test)]
    pub(crate) fn activation_mut_for_test(&mut self) -> &mut Vec<StaticFieldId> {
        &mut self.activation
    }

    #[cfg(test)]
    pub(crate) fn shutdown_mut_for_test(&mut self) -> &mut Vec<StaticFieldId> {
        &mut self.shutdown
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirStaticFieldInitialization {
    ZeroDefault,
    Explicit(StaticInitializerId),
}

impl fmt::Display for MirStaticFieldInitialization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDefault => formatter.write_str("zero-default"),
            Self::Explicit(initializer) => write!(formatter, "explicit {initializer}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirStaticLifecycleIndices {
    pub activation: usize,
    pub shutdown: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirStaticLifecycleTransitionKind {
    /// Activates an initializer-free all-zero slot directly as live.
    ActivateZeroDefault,
    BeginInitialization,
    PublishLive,
    BeginDestruction,
    FinishDestruction,
}

/// Value work performed at one planned activation position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirStaticActivationWork {
    ZeroDefault,
    Explicit(StaticInitializerId),
}

/// One exact activation region in coordinator execution order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStaticActivationRegion {
    pub field: StaticFieldId,
    pub work: MirStaticActivationWork,
    /// A zero-default activation has one direct-to-live transition. Explicit
    /// initialization has begin and publish transitions in that order.
    pub transitions: Vec<MirStaticLifecycleTransition>,
}

/// Static shared-owner cleanup uses an ordinary live static place, unlike a
/// local `MirSharedRelease`, whose owner is addressed by `StorageId`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStaticSharedCleanup {
    pub destination: MirPlace,
    pub target: MirSharedTarget,
    pub span: Span,
}

/// Exact cleanup semantics for the current value of one live static slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirStaticValueCleanup {
    None,
    CompleteObject(MirCleanup),
    OptionalClass(MirClassOptionalCleanup),
    Shared(MirStaticSharedCleanup),
    OptionalShared(MirOptionalSharedCleanup),
    AggregateOptional(super::MirAggregateOptionalCleanup),
    Array(MirArrayInstruction),
}

impl MirStaticValueCleanup {
    pub(crate) fn for_field(
        optional_types: &super::MirOptionalTypeTable,
        ty: MirType,
        field: StaticFieldId,
        span: Span,
    ) -> Option<Self> {
        let destination = MirPlace::static_field(field);
        Some(match ty {
            MirType::Class(target) => Self::CompleteObject(MirCleanup {
                destination,
                target,
                span,
            }),
            MirType::Shared(target) => Self::Shared(MirStaticSharedCleanup {
                destination,
                target,
                span,
            }),
            MirType::Optional(optional) => {
                let metadata = optional_types.get(optional)?;
                match metadata.storage {
                    super::MirOptionalStorage::Scalar => Self::None,
                    super::MirOptionalStorage::InlineClass(class) => {
                        Self::OptionalClass(MirClassOptionalCleanup {
                            optional,
                            destination,
                            class,
                            span,
                        })
                    }
                    super::MirOptionalStorage::SharedOwner(target) => {
                        Self::OptionalShared(MirOptionalSharedCleanup {
                            optional,
                            destination,
                            target,
                            span,
                        })
                    }
                    super::MirOptionalStorage::Nested(_)
                    | super::MirOptionalStorage::InlineArray(_) => {
                        Self::AggregateOptional(super::MirAggregateOptionalCleanup {
                            optional,
                            destination,
                            span,
                        })
                    }
                }
            }
            MirType::Array(array) => Self::Array(MirArrayInstruction::Release {
                owner: destination,
                array,
                span,
            }),
            MirType::I64
            | MirType::U64
            | MirType::U8
            | MirType::F64
            | MirType::Bool
            | MirType::Function(_) => Self::None,
            MirType::Interface(_) | MirType::Obj | MirType::Unit => return None,
        })
    }
}

/// One exact destruction region in reverse activation order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStaticDestructionRegion {
    pub field: StaticFieldId,
    pub begin: MirStaticLifecycleTransition,
    pub cleanup: MirStaticValueCleanup,
    pub finish: MirStaticLifecycleTransition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirStaticLifecycleTransition {
    pub field: StaticFieldId,
    pub kind: MirStaticLifecycleTransitionKind,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirStaticLifecycleDefinition {
    pub field: StaticFieldId,
    pub ty: MirType,
    pub initialization: MirStaticFieldInitialization,
    pub final_span: Option<Span>,
    pub indices: MirStaticLifecycleIndices,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStaticLifecycleProof {
    authority: StaticLifecycleAuthority,
}

impl MirStaticLifecycleProof {
    pub(crate) const fn new(authority: StaticLifecycleAuthority) -> Self {
        Self { authority }
    }

    pub fn authority(&self) -> &StaticLifecycleAuthority {
        &self.authority
    }

    #[cfg(test)]
    pub(crate) fn authority_mut_for_test(&mut self) -> &mut StaticLifecycleAuthority {
        &mut self.authority
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirProgramLifecycle {
    definitions: Vec<MirStaticLifecycleDefinition>,
    activation: Vec<MirStaticLifecycleTransition>,
    shutdown: Vec<MirStaticLifecycleTransition>,
    plan: StaticLifecyclePlan,
    proof: MirStaticLifecycleProof,
}

/// Final program-owned lifecycle code and the compact proof that justifies it.
///
/// Initializer bodies remain independently identified CFGs so their existing
/// storage/value/block IDs and full-expression order never need rewriting.
/// Activation regions place their publication transition on the body's
/// checked publication edge; the next region begins only after that body has
/// completed its post-publication cleanup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStaticLifecycleCoordinator {
    lifecycle: MirProgramLifecycle,
    initializers: Vec<MirStaticInitializerBody>,
    activation: Vec<MirStaticActivationRegion>,
    shutdown: Vec<MirStaticDestructionRegion>,
}

impl MirStaticLifecycleCoordinator {
    pub(crate) fn new(
        lifecycle: MirProgramLifecycle,
        initializers: Vec<MirStaticInitializerBody>,
        activation: Vec<MirStaticActivationRegion>,
        shutdown: Vec<MirStaticDestructionRegion>,
    ) -> Self {
        Self {
            lifecycle,
            initializers,
            activation,
            shutdown,
        }
    }

    pub fn lifecycle(&self) -> &MirProgramLifecycle {
        &self.lifecycle
    }

    pub fn initializers(&self) -> &[MirStaticInitializerBody] {
        &self.initializers
    }

    pub fn activation(&self) -> &[MirStaticActivationRegion] {
        &self.activation
    }

    pub fn shutdown(&self) -> &[MirStaticDestructionRegion] {
        &self.shutdown
    }

    #[cfg(test)]
    pub(crate) fn activation_mut_for_test(&mut self) -> &mut Vec<MirStaticActivationRegion> {
        &mut self.activation
    }

    #[cfg(test)]
    pub(crate) fn shutdown_mut_for_test(&mut self) -> &mut Vec<MirStaticDestructionRegion> {
        &mut self.shutdown
    }

    #[cfg(test)]
    pub(crate) fn initializers_mut_for_test(&mut self) -> &mut Vec<MirStaticInitializerBody> {
        &mut self.initializers
    }

    #[cfg(test)]
    pub(crate) fn lifecycle_mut_for_test(&mut self) -> &mut MirProgramLifecycle {
        &mut self.lifecycle
    }
}

impl MirProgramLifecycle {
    pub(crate) fn new(
        definitions: Vec<MirStaticLifecycleDefinition>,
        activation: Vec<MirStaticLifecycleTransition>,
        shutdown: Vec<MirStaticLifecycleTransition>,
        plan: StaticLifecyclePlan,
        proof: MirStaticLifecycleProof,
    ) -> Self {
        Self {
            definitions,
            activation,
            shutdown,
            plan,
            proof,
        }
    }

    pub fn definitions(&self) -> &[MirStaticLifecycleDefinition] {
        &self.definitions
    }

    pub fn activation(&self) -> &[MirStaticLifecycleTransition] {
        &self.activation
    }

    pub fn shutdown(&self) -> &[MirStaticLifecycleTransition] {
        &self.shutdown
    }

    pub fn plan(&self) -> &StaticLifecyclePlan {
        &self.plan
    }

    pub fn proof(&self) -> &MirStaticLifecycleProof {
        &self.proof
    }

    #[cfg(test)]
    pub(crate) fn plan_mut_for_test(&mut self) -> &mut StaticLifecyclePlan {
        &mut self.plan
    }

    #[cfg(test)]
    pub(crate) fn definitions_mut_for_test(&mut self) -> &mut Vec<MirStaticLifecycleDefinition> {
        &mut self.definitions
    }

    #[cfg(test)]
    pub(crate) fn activation_mut_for_test(&mut self) -> &mut Vec<MirStaticLifecycleTransition> {
        &mut self.activation
    }

    #[cfg(test)]
    pub(crate) fn shutdown_mut_for_test(&mut self) -> &mut Vec<MirStaticLifecycleTransition> {
        &mut self.shutdown
    }

    #[cfg(test)]
    pub(crate) fn proof_mut_for_test(&mut self) -> &mut MirStaticLifecycleProof {
        &mut self.proof
    }
}
