//! Stable aggregate vocabulary for primitive-cast redundancy measurements.

use crate::{
    identity::CallableId,
    mir::{MirPrimitiveCastKind, MirPrimitiveType},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrimitiveCastShape {
    kind: MirPrimitiveCastKind,
    source: MirPrimitiveType,
    target: MirPrimitiveType,
}

impl PrimitiveCastShape {
    pub const fn kind(self) -> MirPrimitiveCastKind {
        self.kind
    }
    pub const fn source(self) -> MirPrimitiveType {
        self.source
    }
    pub const fn target(self) -> MirPrimitiveType {
        self.target
    }
    pub(super) const fn new(
        kind: MirPrimitiveCastKind,
        source: MirPrimitiveType,
        target: MirPrimitiveType,
    ) -> Self {
        Self {
            kind,
            source,
            target,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PrimitiveCastDisposition {
    Identity,
    RemovableChain,
    RequiredIntegerNarrowing,
    RequiredIntegerWidening,
    RequiredIntegerBitConversion,
    BooleanCanonicalization,
    FloatingNumericConversion,
    RawBitReinterpretation,
    CheckedFloatingToInteger,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PrimitiveCastBlocker {
    MalformedIdentity,
    UnsupportedTypeOrOperation,
    ProtectedMetadataOrUse,
    MultipleUses,
    ControlFlowBoundary,
    MissingValueDomainFact,
    CheckedFailure,
    FloatingPayload,
    NonAdjacentProvenance,
    UnsupportedComposition,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PrimitiveCastConsumer {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrimitiveCastCount<T> {
    pub(super) key: T,
    pub(super) sites: u64,
}

impl<T: Copy> PrimitiveCastCount<T> {
    pub const fn key(self) -> T {
        self.key
    }
    pub const fn sites(self) -> u64 {
        self.sites
    }
}

impl<T> PrimitiveCastCount<T> {
    pub(super) const fn new(key: T, sites: u64) -> Self {
        Self { key, sites }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PrimitiveCastObservationCounts {
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
    pub(super) excluded_checked_conversions: u64,
    pub(super) excluded_checked_range_checks: u64,
    pub(super) saturated: bool,
    pub(super) shapes: Vec<PrimitiveCastCount<PrimitiveCastShape>>,
    pub(super) dispositions: Vec<PrimitiveCastCount<PrimitiveCastDisposition>>,
    pub(super) primary_blockers: Vec<PrimitiveCastCount<PrimitiveCastBlocker>>,
    pub(super) barriers: Vec<PrimitiveCastCount<PrimitiveCastBlocker>>,
    pub(super) consumers: Vec<PrimitiveCastCount<PrimitiveCastConsumer>>,
}

impl PrimitiveCastObservationCounts {
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
    pub const fn excluded_checked_conversions(&self) -> u64 {
        self.excluded_checked_conversions
    }
    pub const fn excluded_checked_range_checks(&self) -> u64 {
        self.excluded_checked_range_checks
    }
    pub const fn saturated(&self) -> bool {
        self.saturated
    }
    pub fn shapes(&self) -> &[PrimitiveCastCount<PrimitiveCastShape>] {
        &self.shapes
    }
    pub fn dispositions(&self) -> &[PrimitiveCastCount<PrimitiveCastDisposition>] {
        &self.dispositions
    }
    pub fn primary_blockers(&self) -> &[PrimitiveCastCount<PrimitiveCastBlocker>] {
        &self.primary_blockers
    }
    pub fn barriers(&self) -> &[PrimitiveCastCount<PrimitiveCastBlocker>] {
        &self.barriers
    }
    pub fn consumers(&self) -> &[PrimitiveCastCount<PrimitiveCastConsumer>] {
        &self.consumers
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrimitiveCastCallableObservation {
    callable: CallableId,
    counts: PrimitiveCastObservationCounts,
}

impl PrimitiveCastCallableObservation {
    pub const fn callable(&self) -> CallableId {
        self.callable
    }
    pub const fn counts(&self) -> &PrimitiveCastObservationCounts {
        &self.counts
    }
    pub(super) const fn new(callable: CallableId, counts: PrimitiveCastObservationCounts) -> Self {
        Self { callable, counts }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PrimitiveCastObservation {
    counts: PrimitiveCastObservationCounts,
    callables: Vec<PrimitiveCastCallableObservation>,
}

impl PrimitiveCastObservation {
    pub const fn counts(&self) -> &PrimitiveCastObservationCounts {
        &self.counts
    }
    pub fn callables(&self) -> &[PrimitiveCastCallableObservation] {
        &self.callables
    }
    pub(super) const fn new(
        counts: PrimitiveCastObservationCounts,
        callables: Vec<PrimitiveCastCallableObservation>,
    ) -> Self {
        Self { counts, callables }
    }
}
