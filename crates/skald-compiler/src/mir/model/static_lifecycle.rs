//! Static-lifecycle MIR schema and checkable whole-program certificate.

use std::fmt;

use crate::{
    identity::{ArrayTypeId, CallableId, ClassId, StaticFieldId, StaticInitializerId},
    source::Span,
};

use super::{
    MirProgram, MirType, PreliminaryMirProgram, PreliminaryMirStaticField,
    PreliminaryMirStaticInitializer,
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

    #[cfg(test)]
    pub(crate) fn summaries_mut_for_test(&mut self) -> &mut Vec<StaticEffectSummary> {
        &mut self.summaries
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
    BeginInitialization,
    PublishLive,
    BeginDestruction,
    FinishDestruction,
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
    pub indices: MirStaticLifecycleIndices,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStaticLifecycleCertificate {
    effects: StaticEffectAnalysis,
    dependencies: Vec<StaticLifetimeDependency>,
}

impl MirStaticLifecycleCertificate {
    pub(crate) fn new(
        effects: StaticEffectAnalysis,
        dependencies: Vec<StaticLifetimeDependency>,
    ) -> Self {
        Self {
            effects,
            dependencies,
        }
    }

    pub fn effects(&self) -> &StaticEffectAnalysis {
        &self.effects
    }

    pub fn dependencies(&self) -> &[StaticLifetimeDependency] {
        &self.dependencies
    }

    #[cfg(test)]
    pub(crate) fn effects_mut_for_test(&mut self) -> &mut StaticEffectAnalysis {
        &mut self.effects
    }

    #[cfg(test)]
    pub(crate) fn dependencies_mut_for_test(&mut self) -> &mut Vec<StaticLifetimeDependency> {
        &mut self.dependencies
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirProgramLifecycle {
    definitions: Vec<MirStaticLifecycleDefinition>,
    activation: Vec<MirStaticLifecycleTransition>,
    shutdown: Vec<MirStaticLifecycleTransition>,
    plan: StaticLifecyclePlan,
    certificate: MirStaticLifecycleCertificate,
}

impl MirProgramLifecycle {
    pub(crate) fn new(
        definitions: Vec<MirStaticLifecycleDefinition>,
        activation: Vec<MirStaticLifecycleTransition>,
        shutdown: Vec<MirStaticLifecycleTransition>,
        plan: StaticLifecyclePlan,
        certificate: MirStaticLifecycleCertificate,
    ) -> Self {
        Self {
            definitions,
            activation,
            shutdown,
            plan,
            certificate,
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

    pub fn certificate(&self) -> &MirStaticLifecycleCertificate {
        &self.certificate
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
    pub(crate) fn certificate_mut_for_test(&mut self) -> &mut MirStaticLifecycleCertificate {
        &mut self.certificate
    }
}

/// Preliminary MIR plus explicit, checkable static-lifecycle MIR metadata.
///
/// The wrapped preliminary program remains private, so no backend can consume
/// initializer bodies before lifecycle coordinator synthesis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedMirProgram {
    preliminary: PreliminaryMirProgram,
    lifecycle: MirProgramLifecycle,
}

impl PlannedMirProgram {
    pub(crate) fn new(preliminary: PreliminaryMirProgram, lifecycle: MirProgramLifecycle) -> Self {
        Self {
            preliminary,
            lifecycle,
        }
    }

    pub fn lifecycle_mir(&self) -> &MirProgramLifecycle {
        &self.lifecycle
    }

    pub fn effects(&self) -> &StaticEffectAnalysis {
        self.lifecycle.certificate.effects()
    }

    pub fn dependencies(&self) -> &[StaticLifetimeDependency] {
        self.lifecycle.certificate.dependencies()
    }

    pub fn lifecycle(&self) -> &StaticLifecyclePlan {
        self.lifecycle.plan()
    }

    pub fn static_fields(&self) -> impl ExactSizeIterator<Item = &PreliminaryMirStaticField> {
        self.preliminary.static_fields()
    }

    pub fn static_initializers(
        &self,
    ) -> impl ExactSizeIterator<Item = &PreliminaryMirStaticInitializer> {
        self.preliminary.static_initializers()
    }

    /// Returns the typed storage, values, blocks, and publication boundary for
    /// one program-owned lifecycle initializer identity.
    pub fn static_initializer(
        &self,
        id: StaticInitializerId,
    ) -> Option<&PreliminaryMirStaticInitializer> {
        self.preliminary.static_initializer(id)
    }

    pub fn has_static_initializers(&self) -> bool {
        self.preliminary.has_static_initializers()
    }

    /// Converts only a product needing no lifecycle synthesis into the legacy
    /// final-MIR path. Explicit initializers remain unavailable to backends.
    pub fn try_into_final(self) -> Result<MirProgram, Box<Self>> {
        if self.has_static_initializers() {
            return Err(Box::new(self));
        }
        Ok(self
            .preliminary
            .try_into_final()
            .expect("initializer-free preliminary MIR must convert to final MIR"))
    }

    pub(crate) const fn preliminary(&self) -> &PreliminaryMirProgram {
        &self.preliminary
    }

    #[cfg(test)]
    pub(crate) fn preliminary_mut_for_test(&mut self) -> &mut PreliminaryMirProgram {
        &mut self.preliminary
    }

    #[cfg(test)]
    pub(crate) fn lifecycle_mut_for_test(&mut self) -> &mut MirProgramLifecycle {
        &mut self.lifecycle
    }
}
