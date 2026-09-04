//! Stable aggregate vocabulary for local primitive CSE measurements.

use crate::identity::CallableId;

use super::site::RedundancySiteExample;

pub type LocalCseCount<T> = super::count::RedundancyCount<T>;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LocalCseOperationFamily {
    IntegerUnary,
    BooleanUnary,
    IntegerBinary,
    IntegerComparison,
    BooleanComparison,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LocalCseOutcome {
    Replaceable,
    DeadResult,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LocalCseBlocker {
    MalformedIdentity,
    UnsupportedTypeOrOperation,
    ProtectedMetadataOrUse,
    SourceObservation,
    ControlFlowBoundary,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LocalCseConsumer {
    Dead,
    TotalPrimitive,
    PrimitiveCast,
    ConditionalBranch,
    Store,
    Return,
    Call,
    CheckedProtocol,
    ProtectedMetadata,
    OwnershipOrLifecycle,
    InputOutput,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LocalCseExcludedFamily {
    Constant,
    Cast,
    Load,
    FloatingOperation,
    CheckedProtocol,
    Call,
    OwnershipOrLifecycle,
    SourceObservation,
    SemanticQuery,
    Other,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LocalCseObservationCounts {
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
    pub(super) replaceable_uses: u64,
    pub(super) maximum_repetitions_per_key: u64,
    pub(super) scalar_spill_unlocks: u64,
    pub(super) saturated: bool,
    pub(super) outcomes: Vec<LocalCseCount<LocalCseOutcome>>,
    pub(super) operation_families: Vec<LocalCseCount<LocalCseOperationFamily>>,
    pub(super) primary_blockers: Vec<LocalCseCount<LocalCseBlocker>>,
    pub(super) barriers: Vec<LocalCseCount<LocalCseBlocker>>,
    pub(super) consumers: Vec<LocalCseCount<LocalCseConsumer>>,
    pub(super) excluded_families: Vec<LocalCseCount<LocalCseExcludedFamily>>,
}

impl LocalCseObservationCounts {
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
    pub const fn replaceable_uses(&self) -> u64 {
        self.replaceable_uses
    }
    pub const fn maximum_repetitions_per_key(&self) -> u64 {
        self.maximum_repetitions_per_key
    }
    pub const fn scalar_spill_unlocks(&self) -> u64 {
        self.scalar_spill_unlocks
    }
    pub const fn saturated(&self) -> bool {
        self.saturated
    }
    pub fn outcomes(&self) -> &[LocalCseCount<LocalCseOutcome>] {
        &self.outcomes
    }
    pub fn operation_families(&self) -> &[LocalCseCount<LocalCseOperationFamily>] {
        &self.operation_families
    }
    pub fn primary_blockers(&self) -> &[LocalCseCount<LocalCseBlocker>] {
        &self.primary_blockers
    }
    pub fn barriers(&self) -> &[LocalCseCount<LocalCseBlocker>] {
        &self.barriers
    }
    pub fn consumers(&self) -> &[LocalCseCount<LocalCseConsumer>] {
        &self.consumers
    }
    pub fn excluded_families(&self) -> &[LocalCseCount<LocalCseExcludedFamily>] {
        &self.excluded_families
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalCseCallableObservation {
    callable: CallableId,
    counts: LocalCseObservationCounts,
    examples: Vec<RedundancySiteExample<LocalCseBlocker>>,
}

impl LocalCseCallableObservation {
    pub const fn callable(&self) -> CallableId {
        self.callable
    }
    pub const fn counts(&self) -> &LocalCseObservationCounts {
        &self.counts
    }
    pub fn examples(&self) -> &[RedundancySiteExample<LocalCseBlocker>] {
        &self.examples
    }
    pub(super) const fn new(
        callable: CallableId,
        counts: LocalCseObservationCounts,
        examples: Vec<RedundancySiteExample<LocalCseBlocker>>,
    ) -> Self {
        Self {
            callable,
            counts,
            examples,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LocalCseObservation {
    counts: LocalCseObservationCounts,
    callables: Vec<LocalCseCallableObservation>,
    examples: Vec<RedundancySiteExample<LocalCseBlocker>>,
}

impl LocalCseObservation {
    pub const fn counts(&self) -> &LocalCseObservationCounts {
        &self.counts
    }
    pub fn callables(&self) -> &[LocalCseCallableObservation] {
        &self.callables
    }
    pub fn examples(&self) -> &[RedundancySiteExample<LocalCseBlocker>] {
        &self.examples
    }
    pub(super) const fn new(
        counts: LocalCseObservationCounts,
        callables: Vec<LocalCseCallableObservation>,
        examples: Vec<RedundancySiteExample<LocalCseBlocker>>,
    ) -> Self {
        Self {
            counts,
            callables,
            examples,
        }
    }
}
