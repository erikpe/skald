//! Stable aggregate vocabulary for local redundancy measurements.

use crate::identity::CallableId;

use super::site::RedundancySiteExample;

pub type ScalarSpillCount<T> = super::count::RedundancyCount<T>;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ScalarSpillDepth {
    Direct,
    OneHop,
    Transitive,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ScalarSpillBlocker {
    MalformedIdentity,
    UnsupportedTypeOrOperation,
    NoncanonicalPlace,
    ProtectedMetadataOrUse,
    AliasExposure,
    LifecycleParticipation,
    AmbiguousWrites,
    MissingDominance,
    ControlFlowBoundary,
    OtherUnsupportedProducer,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ScalarSpillConsumer {
    CheckedIntegerProtocol,
    TotalPrimitive,
    PrimitiveCast,
    ConditionalBranch,
    Store,
    Return,
    Call,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ScalarSpillUnlock {
    CheckedFolding,
    PrimitiveFolding,
    CastSimplification,
    BranchFolding,
    CommonSubexpression,
    DirectSubstitution,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ScalarSpillProvenanceCounts {
    pub(super) inspected: u64,
    pub(super) interesting: u64,
    pub(super) proven: u64,
    pub(super) blocked: u64,
    pub(super) non_candidates: u64,
    pub(super) affected_callables: u64,
    pub(super) supporting_values: u64,
    pub(super) supporting_instructions: u64,
    pub(super) removable_values_upper_bound: u64,
    pub(super) removable_instructions_upper_bound: u64,
    pub(super) saturated: bool,
    pub(super) depths: Vec<ScalarSpillCount<ScalarSpillDepth>>,
    pub(super) primary_blockers: Vec<ScalarSpillCount<ScalarSpillBlocker>>,
    pub(super) barriers: Vec<ScalarSpillCount<ScalarSpillBlocker>>,
    pub(super) consumers: Vec<ScalarSpillCount<ScalarSpillConsumer>>,
    pub(super) unlocks: Vec<ScalarSpillCount<ScalarSpillUnlock>>,
}

impl ScalarSpillProvenanceCounts {
    pub const fn inspected(&self) -> u64 {
        self.inspected
    }
    pub const fn interesting(&self) -> u64 {
        self.interesting
    }
    pub const fn proven(&self) -> u64 {
        self.proven
    }
    pub const fn blocked(&self) -> u64 {
        self.blocked
    }
    pub const fn non_candidates(&self) -> u64 {
        self.non_candidates
    }
    pub const fn affected_callables(&self) -> u64 {
        self.affected_callables
    }
    pub const fn supporting_values(&self) -> u64 {
        self.supporting_values
    }
    pub const fn supporting_instructions(&self) -> u64 {
        self.supporting_instructions
    }
    pub const fn removable_values_upper_bound(&self) -> u64 {
        self.removable_values_upper_bound
    }
    pub const fn removable_instructions_upper_bound(&self) -> u64 {
        self.removable_instructions_upper_bound
    }
    pub const fn saturated(&self) -> bool {
        self.saturated
    }
    pub fn depths(&self) -> &[ScalarSpillCount<ScalarSpillDepth>] {
        &self.depths
    }
    pub fn primary_blockers(&self) -> &[ScalarSpillCount<ScalarSpillBlocker>] {
        &self.primary_blockers
    }
    pub fn barriers(&self) -> &[ScalarSpillCount<ScalarSpillBlocker>] {
        &self.barriers
    }
    pub fn consumers(&self) -> &[ScalarSpillCount<ScalarSpillConsumer>] {
        &self.consumers
    }
    pub fn unlocks(&self) -> &[ScalarSpillCount<ScalarSpillUnlock>] {
        &self.unlocks
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarSpillCallableObservation {
    callable: CallableId,
    counts: ScalarSpillProvenanceCounts,
    examples: Vec<RedundancySiteExample<ScalarSpillBlocker>>,
}

impl ScalarSpillCallableObservation {
    pub const fn callable(&self) -> CallableId {
        self.callable
    }
    pub const fn counts(&self) -> &ScalarSpillProvenanceCounts {
        &self.counts
    }
    pub fn examples(&self) -> &[RedundancySiteExample<ScalarSpillBlocker>] {
        &self.examples
    }
    pub(super) const fn new(
        callable: CallableId,
        counts: ScalarSpillProvenanceCounts,
        examples: Vec<RedundancySiteExample<ScalarSpillBlocker>>,
    ) -> Self {
        Self {
            callable,
            counts,
            examples,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ScalarSpillProvenanceObservation {
    counts: ScalarSpillProvenanceCounts,
    callables: Vec<ScalarSpillCallableObservation>,
    examples: Vec<RedundancySiteExample<ScalarSpillBlocker>>,
}

impl ScalarSpillProvenanceObservation {
    pub const fn counts(&self) -> &ScalarSpillProvenanceCounts {
        &self.counts
    }
    pub fn callables(&self) -> &[ScalarSpillCallableObservation] {
        &self.callables
    }
    pub fn examples(&self) -> &[RedundancySiteExample<ScalarSpillBlocker>] {
        &self.examples
    }
    pub(super) const fn new(
        counts: ScalarSpillProvenanceCounts,
        callables: Vec<ScalarSpillCallableObservation>,
        examples: Vec<RedundancySiteExample<ScalarSpillBlocker>>,
    ) -> Self {
        Self {
            counts,
            callables,
            examples,
        }
    }
}
