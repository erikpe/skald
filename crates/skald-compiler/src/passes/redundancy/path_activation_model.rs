//! Stable owned vocabulary for dead normalized path-activation analysis.

use crate::{
    identity::CallableId,
    mir::{BlockId, MirInstruction, MirStorage, MirValue, StorageId},
};

use super::site::RedundancyStorageExample;

pub type DeadPathActivationCount<T> = super::count::RedundancyCount<T>;

/// Why one normalized activation cannot be removed as a complete protocol.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DeadPathActivationBlocker {
    SourceBinding,
    NonBooleanStorage,
    Attachment,
    NoncanonicalReadPlace,
    NoncanonicalWritePlace,
    AuthorizedWrite,
    MaterialLoadResult,
    MalformedLoadResult,
    CheckedProtocol,
    ProofMetadata,
    AliasExposure,
    Call,
    OwnershipOrLifecycle,
    InputOutput,
    OtherExecutable,
    MalformedInstructionSite,
}

/// Semantic role of one instruction owned by a removable carrier protocol.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DeadPathActivationInstructionKind {
    Load,
    Store,
    LifetimeLive,
    LifetimeDead,
}

/// Exact owned instruction snapshot retained for later plan revalidation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeadPathActivationInstruction {
    block: BlockId,
    instruction: usize,
    kind: DeadPathActivationInstructionKind,
    expected: MirInstruction,
}

impl DeadPathActivationInstruction {
    pub const fn block(&self) -> BlockId {
        self.block
    }

    pub const fn instruction(&self) -> usize {
        self.instruction
    }

    pub const fn kind(&self) -> DeadPathActivationInstructionKind {
        self.kind
    }

    pub const fn expected(&self) -> &MirInstruction {
        &self.expected
    }

    pub(super) const fn new(
        block: BlockId,
        instruction: usize,
        kind: DeadPathActivationInstructionKind,
        expected: MirInstruction,
    ) -> Self {
        Self {
            block,
            instruction,
            kind,
            expected,
        }
    }
}

/// One complete protocol which can be deleted atomically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeadPathActivationCandidate {
    declaration_index: usize,
    declaration: MirStorage,
    instructions: Vec<DeadPathActivationInstruction>,
    load_results: Vec<MirValue>,
}

impl DeadPathActivationCandidate {
    pub const fn storage(&self) -> StorageId {
        self.declaration.id
    }

    pub const fn declaration_index(&self) -> usize {
        self.declaration_index
    }

    pub const fn declaration(&self) -> &MirStorage {
        &self.declaration
    }

    pub fn instructions(&self) -> &[DeadPathActivationInstruction] {
        &self.instructions
    }

    pub fn load_results(&self) -> &[MirValue] {
        &self.load_results
    }

    pub const fn removable_storages_upper_bound(&self) -> u64 {
        1
    }

    pub fn removable_values_upper_bound(&self) -> u64 {
        usize_to_u64(self.load_results.len())
    }

    pub fn removable_instructions_upper_bound(&self) -> u64 {
        usize_to_u64(self.instructions.len())
    }

    pub(super) const fn new(
        declaration_index: usize,
        declaration: MirStorage,
        instructions: Vec<DeadPathActivationInstruction>,
        load_results: Vec<MirValue>,
    ) -> Self {
        Self {
            declaration_index,
            declaration,
            instructions,
            load_results,
        }
    }
}

/// One normalized activation rejected by the exact candidate boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockedPathActivation {
    storage: StorageId,
    blockers: Vec<DeadPathActivationBlocker>,
}

impl BlockedPathActivation {
    pub const fn storage(&self) -> StorageId {
        self.storage
    }

    pub fn blockers(&self) -> &[DeadPathActivationBlocker] {
        &self.blockers
    }

    pub(super) const fn new(storage: StorageId, blockers: Vec<DeadPathActivationBlocker>) -> Self {
        Self { storage, blockers }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeadPathActivationCounts {
    pub(super) inspected: u64,
    pub(super) interesting: u64,
    pub(super) proven: u64,
    pub(super) blocked: u64,
    pub(super) non_candidates: u64,
    pub(super) affected_callables: u64,
    pub(super) supporting_values: u64,
    pub(super) supporting_instructions: u64,
    pub(super) removable_storages_upper_bound: u64,
    pub(super) removable_values_upper_bound: u64,
    pub(super) removable_instructions_upper_bound: u64,
    pub(super) removable_loads_upper_bound: u64,
    pub(super) removable_stores_upper_bound: u64,
    pub(super) removable_lifetime_markers_upper_bound: u64,
    pub(super) maximum_protocol_size: u64,
    pub(super) primary_blockers: Vec<DeadPathActivationCount<DeadPathActivationBlocker>>,
    pub(super) barriers: Vec<DeadPathActivationCount<DeadPathActivationBlocker>>,
    pub(super) saturated: bool,
}

macro_rules! count_getter {
    ($name:ident) => {
        pub const fn $name(&self) -> u64 {
            self.$name
        }
    };
}

impl DeadPathActivationCounts {
    count_getter!(inspected);
    count_getter!(interesting);
    count_getter!(proven);
    count_getter!(blocked);
    count_getter!(non_candidates);
    count_getter!(affected_callables);
    count_getter!(supporting_values);
    count_getter!(supporting_instructions);
    count_getter!(removable_storages_upper_bound);
    count_getter!(removable_values_upper_bound);
    count_getter!(removable_instructions_upper_bound);
    count_getter!(removable_loads_upper_bound);
    count_getter!(removable_stores_upper_bound);
    count_getter!(removable_lifetime_markers_upper_bound);
    count_getter!(maximum_protocol_size);

    pub fn primary_blockers(&self) -> &[DeadPathActivationCount<DeadPathActivationBlocker>] {
        &self.primary_blockers
    }

    pub fn barriers(&self) -> &[DeadPathActivationCount<DeadPathActivationBlocker>] {
        &self.barriers
    }

    pub const fn saturated(&self) -> bool {
        self.saturated
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeadPathActivationCallableObservation {
    callable: CallableId,
    counts: DeadPathActivationCounts,
    candidates: Vec<DeadPathActivationCandidate>,
    blocked_activations: Vec<BlockedPathActivation>,
    examples: Vec<RedundancyStorageExample<DeadPathActivationBlocker>>,
}

impl DeadPathActivationCallableObservation {
    pub const fn callable(&self) -> CallableId {
        self.callable
    }

    pub const fn counts(&self) -> &DeadPathActivationCounts {
        &self.counts
    }

    pub fn candidates(&self) -> &[DeadPathActivationCandidate] {
        &self.candidates
    }

    pub fn blocked_activations(&self) -> &[BlockedPathActivation] {
        &self.blocked_activations
    }

    pub fn examples(&self) -> &[RedundancyStorageExample<DeadPathActivationBlocker>] {
        &self.examples
    }

    pub(super) const fn new(
        callable: CallableId,
        counts: DeadPathActivationCounts,
        candidates: Vec<DeadPathActivationCandidate>,
        blocked_activations: Vec<BlockedPathActivation>,
        examples: Vec<RedundancyStorageExample<DeadPathActivationBlocker>>,
    ) -> Self {
        Self {
            callable,
            counts,
            candidates,
            blocked_activations,
            examples,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeadPathActivationObservation {
    counts: DeadPathActivationCounts,
    callables: Vec<DeadPathActivationCallableObservation>,
    examples: Vec<RedundancyStorageExample<DeadPathActivationBlocker>>,
}

impl DeadPathActivationObservation {
    pub const fn counts(&self) -> &DeadPathActivationCounts {
        &self.counts
    }

    pub fn callables(&self) -> &[DeadPathActivationCallableObservation] {
        &self.callables
    }

    pub fn examples(&self) -> &[RedundancyStorageExample<DeadPathActivationBlocker>] {
        &self.examples
    }

    pub(super) const fn new(
        counts: DeadPathActivationCounts,
        callables: Vec<DeadPathActivationCallableObservation>,
        examples: Vec<RedundancyStorageExample<DeadPathActivationBlocker>>,
    ) -> Self {
        Self {
            counts,
            callables,
            examples,
        }
    }
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
