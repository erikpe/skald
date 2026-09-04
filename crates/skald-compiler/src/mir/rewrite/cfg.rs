//! Immutable callable-local control-flow facts and normalized candidates.
//!
//! This analysis deliberately owns only a short-lived snapshot. It does not
//! cache facts across rewrites or infer higher-level dominance, liveness, or
//! loop structure. Post-proof forwarding and merge queries live behind this
//! facade and require the normalized-only [`MirFinalCfgFacts`] wrapper.

mod canonicalization;

pub(crate) use canonicalization::{
    analyze_basic_block_merging, analyze_empty_block_forwarding, MirBasicBlockMergeAnalysis,
    MirBasicBlockMergeBarrier, MirBasicBlockMergeBarrierKind, MirBasicBlockMergeCandidate,
    MirBasicBlockMergeCounts, MirEmptyBlockForwardingAnalysis, MirEmptyBlockForwardingBarrier,
    MirEmptyBlockForwardingBarrierKind, MirEmptyBlockForwardingCandidate,
    MirEmptyBlockForwardingCounts, MirEmptyBlockForwardingPlan, MirEmptyBlockForwardingResolution,
};

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ops::Deref,
};

use crate::{
    identity::CallableId,
    mir::{
        classify_local_identity_site, BlockId, MirBasicBlock, MirDefinitionRef,
        MirIdentitySiteRole, MirTerminator, ValueId,
    },
};

use super::{
    census::{value_use_census_for_definition, MirValueUseCensus},
    edit::MirCallableEdit,
    map::observe_definition_local_identities,
    MirLocalIdentity, MirLocalIdentityObserver, MirLocalIdentitySite, MirReferenceFailure,
    MirRewriteError,
};

/// One non-executable reference which protects a block from local deletion.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MirProtectedBlockRoot {
    site: MirLocalIdentitySite,
    block: BlockId,
}

impl MirProtectedBlockRoot {
    pub(crate) const fn site(self) -> MirLocalIdentitySite {
        self.site
    }

    pub(crate) const fn block(self) -> BlockId {
        self.block
    }
}

/// One executable successor occurrence in a callable-local terminator.
///
/// `successor_index` is the stable position in [`MirTerminator::successors`]
/// semantic order. Retaining the occurrence instead of only the endpoint keeps
/// parallel edges distinct for later structural eligibility checks.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MirLocalCfgEdge {
    source: BlockId,
    target: BlockId,
    successor_index: usize,
}

impl MirLocalCfgEdge {
    pub(crate) const fn source(self) -> BlockId {
        self.source
    }

    pub(crate) const fn target(self) -> BlockId {
        self.target
    }

    pub(crate) const fn successor_index(self) -> usize {
        self.successor_index
    }
}

/// Closed executable terminator classification for local CFG analyses.
///
/// The exhaustive conversion from [`MirTerminator`] is the maintenance point
/// which requires every future terminator form to choose a structural shape.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirLocalCfgTerminatorKind {
    Return,
    ReturnShared,
    ReturnOptionalShared,
    Panic,
    Goto,
    Branch,
    ShiftCountCheck,
    IntegerDivisorCheck,
    PrimitiveCastRangeCheck,
    CheckedCast,
    SharedCast,
    OptionalUnwrap,
    OptionalSharedUnwrap,
    BeginOptionalView,
    BeginOptionalBoxView,
    CheckOptionalMutation,
    ArrayPositionCheck,
    ArrayOperationCheck,
    ArrayLoop,
    Terminate,
}

impl MirLocalCfgTerminatorKind {
    const fn classify(terminator: &MirTerminator) -> Self {
        match terminator {
            MirTerminator::Return { .. } => Self::Return,
            MirTerminator::ReturnShared { .. } => Self::ReturnShared,
            MirTerminator::ReturnOptionalShared { .. } => Self::ReturnOptionalShared,
            MirTerminator::Panic { .. } => Self::Panic,
            MirTerminator::Goto { .. } => Self::Goto,
            MirTerminator::Branch { .. } => Self::Branch,
            MirTerminator::ShiftCountCheck { .. } => Self::ShiftCountCheck,
            MirTerminator::IntegerDivisorCheck { .. } => Self::IntegerDivisorCheck,
            MirTerminator::PrimitiveCastRangeCheck { .. } => Self::PrimitiveCastRangeCheck,
            MirTerminator::CheckedCast { .. } => Self::CheckedCast,
            MirTerminator::SharedCast { .. } => Self::SharedCast,
            MirTerminator::OptionalUnwrap { .. } => Self::OptionalUnwrap,
            MirTerminator::OptionalSharedUnwrap { .. } => Self::OptionalSharedUnwrap,
            MirTerminator::BeginOptionalView { .. } => Self::BeginOptionalView,
            MirTerminator::BeginOptionalBoxView { .. } => Self::BeginOptionalBoxView,
            MirTerminator::CheckOptionalMutation { .. } => Self::CheckOptionalMutation,
            MirTerminator::ArrayPositionCheck { .. } => Self::ArrayPositionCheck,
            MirTerminator::ArrayOperationCheck { .. } => Self::ArrayOperationCheck,
            MirTerminator::ArrayLoop { .. } => Self::ArrayLoop,
            MirTerminator::Terminate { .. } => Self::Terminate,
        }
    }

    const fn successor_count(self) -> usize {
        match self {
            Self::Return
            | Self::ReturnShared
            | Self::ReturnOptionalShared
            | Self::Panic
            | Self::Terminate => 0,
            Self::Goto => 1,
            Self::Branch
            | Self::ShiftCountCheck
            | Self::IntegerDivisorCheck
            | Self::PrimitiveCastRangeCheck
            | Self::CheckedCast
            | Self::SharedCast
            | Self::OptionalUnwrap
            | Self::OptionalSharedUnwrap
            | Self::CheckOptionalMutation
            | Self::ArrayPositionCheck
            | Self::ArrayOperationCheck
            | Self::ArrayLoop => 2,
            Self::BeginOptionalView | Self::BeginOptionalBoxView => 3,
        }
    }
}

/// Structural shape, edges, and transient definitions owned by one block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirLocalCfgBlockFacts {
    block: BlockId,
    successors: Vec<BlockId>,
    successor_edges: Vec<MirLocalCfgEdge>,
    predecessor_edges: Vec<MirLocalCfgEdge>,
    defined_values: Vec<ValueId>,
    instruction_count: usize,
    terminator_kind: MirLocalCfgTerminatorKind,
    is_entry: bool,
    is_protected_root: bool,
    is_permanent_attachment: bool,
}

impl MirLocalCfgBlockFacts {
    pub(crate) const fn block(&self) -> BlockId {
        self.block
    }

    pub(crate) fn successors(&self) -> &[BlockId] {
        &self.successors
    }

    pub(crate) fn successor_edges(&self) -> &[MirLocalCfgEdge] {
        &self.successor_edges
    }

    pub(crate) fn predecessor_edges(&self) -> &[MirLocalCfgEdge] {
        &self.predecessor_edges
    }

    pub(crate) fn defined_values(&self) -> &[ValueId] {
        &self.defined_values
    }

    pub(crate) const fn instruction_count(&self) -> usize {
        self.instruction_count
    }

    pub(crate) const fn terminator_kind(&self) -> MirLocalCfgTerminatorKind {
        self.terminator_kind
    }

    pub(crate) const fn is_entry(&self) -> bool {
        self.is_entry
    }

    pub(crate) const fn is_protected_root(&self) -> bool {
        self.is_protected_root
    }

    pub(crate) const fn is_permanent_attachment(&self) -> bool {
        self.is_permanent_attachment
    }
}

/// Deterministic roots, adjacency, closure, and block-owned value facts.
///
/// All block lists use the callable's current explicit block order. Protected
/// roots retain exhaustive identity-observation order and may therefore name
/// the same block in more than one semantic role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirLocalCfgFacts {
    callable: CallableId,
    entry: BlockId,
    protected_roots: Vec<MirProtectedBlockRoot>,
    permanent_roots: Vec<MirProtectedBlockRoot>,
    edges: Vec<MirLocalCfgEdge>,
    blocks: Vec<MirLocalCfgBlockFacts>,
    entry_reachable: Vec<BlockId>,
    reachable: Vec<BlockId>,
    protected_but_entry_unreachable: Vec<BlockId>,
    unreachable: Vec<BlockId>,
}

/// CFG facts proven to come from normalized final MIR.
///
/// Keeping this wrapper opaque prevents post-proof candidate analysis from
/// accidentally accepting a proof-rich snapshot whose metadata roots still
/// constrain executable control flow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirFinalCfgFacts(MirLocalCfgFacts);

impl Deref for MirFinalCfgFacts {
    type Target = MirLocalCfgFacts;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl MirLocalCfgFacts {
    pub(crate) const fn callable(&self) -> CallableId {
        self.callable
    }

    pub(crate) const fn entry(&self) -> BlockId {
        self.entry
    }

    pub(crate) fn protected_roots(&self) -> &[MirProtectedBlockRoot] {
        &self.protected_roots
    }

    pub(crate) fn permanent_roots(&self) -> &[MirProtectedBlockRoot] {
        &self.permanent_roots
    }

    pub(crate) fn edges(&self) -> &[MirLocalCfgEdge] {
        &self.edges
    }

    pub(crate) fn blocks(&self) -> &[MirLocalCfgBlockFacts] {
        &self.blocks
    }

    pub(crate) fn block(&self, block: BlockId) -> Option<&MirLocalCfgBlockFacts> {
        (block.callable() == self.callable)
            .then(|| self.blocks.iter().find(|facts| facts.block == block))
            .flatten()
    }

    pub(crate) fn entry_reachable(&self) -> &[BlockId] {
        &self.entry_reachable
    }

    pub(crate) fn reachable(&self) -> &[BlockId] {
        &self.reachable
    }

    pub(crate) fn protected_but_entry_unreachable(&self) -> &[BlockId] {
        &self.protected_but_entry_unreachable
    }

    pub(crate) fn unreachable(&self) -> &[BlockId] {
        &self.unreachable
    }
}

/// Computes CFG facts directly from a borrowed dense definition.
pub(crate) fn local_cfg_facts_for_definition(
    definition: MirDefinitionRef<'_>,
) -> Result<MirLocalCfgFacts, MirRewriteError> {
    cfg_facts_for_definition(definition, MirCfgRootContract::ProofRich)
}

/// Computes executable CFG facts using only roots valid after proof
/// provenance has been consumed.
pub(crate) fn final_cfg_facts_for_definition(
    definition: MirDefinitionRef<'_>,
) -> Result<MirFinalCfgFacts, MirRewriteError> {
    cfg_facts_for_definition(definition, MirCfgRootContract::Final).map(MirFinalCfgFacts)
}

fn cfg_facts_for_definition(
    definition: MirDefinitionRef<'_>,
    root_contract: MirCfgRootContract,
) -> Result<MirLocalCfgFacts, MirRewriteError> {
    let callable = definition.callable();
    let mut blocks = Vec::with_capacity(definition.body().blocks.len());
    for (index, block) in definition.body().blocks.iter().enumerate() {
        validate_dense_declaration(callable, index, block.id)?;
        blocks.push(snapshot_block(block)?);
    }
    for block in &blocks {
        for successor in &block.successors {
            validate_dense_block_reference(
                definition,
                *successor,
                MirLocalIdentitySite::Terminator(block.block.index()),
            )?;
        }
    }

    let mut roots = RootCollector::new(root_contract);
    observe_definition_local_identities(definition, &mut roots)?;
    let entry = roots.entry.ok_or(MirRewriteError::InvalidReference {
        expected: callable,
        identity: MirLocalIdentity::Block(definition.body().entry),
        site: MirLocalIdentitySite::BodyEntry,
        failure: MirReferenceFailure::Unknown,
    })?;
    for root in &roots.protected {
        validate_dense_block_reference(definition, root.block, root.site)?;
    }
    validate_dense_block_reference(definition, entry, MirLocalIdentitySite::BodyEntry)?;

    build_facts(
        callable,
        entry,
        roots.protected,
        blocks,
        value_use_census_for_definition(definition)?,
    )
}

impl MirCallableEdit {
    /// Computes CFG facts from the current sparse edit snapshot.
    pub(crate) fn local_cfg_facts(&self) -> Result<MirLocalCfgFacts, MirRewriteError> {
        self.cfg_facts(MirCfgRootContract::ProofRich)
    }

    /// Computes executable CFG facts from a normalized sparse edit snapshot.
    pub(crate) fn final_cfg_facts(&self) -> Result<MirFinalCfgFacts, MirRewriteError> {
        self.cfg_facts(MirCfgRootContract::Final)
            .map(MirFinalCfgFacts)
    }

    fn cfg_facts(
        &self,
        root_contract: MirCfgRootContract,
    ) -> Result<MirLocalCfgFacts, MirRewriteError> {
        let callable = self.callable();
        let mut seen = BTreeSet::new();
        let mut blocks = Vec::with_capacity(self.block_order().len());
        for block_id in self.block_order() {
            if !seen.insert(*block_id) {
                return Err(MirRewriteError::DuplicateOrderIdentity {
                    identity: MirLocalIdentity::Block(*block_id),
                });
            }
            let block = self.block(*block_id)?;
            blocks.push(snapshot_block(block)?);
        }
        for live in self.block_ids() {
            if !seen.contains(&live) {
                return Err(MirRewriteError::MissingOrderIdentity {
                    identity: MirLocalIdentity::Block(live),
                });
            }
        }
        for block in &blocks {
            for successor in &block.successors {
                validate_edit_block_reference(
                    self,
                    *successor,
                    MirLocalIdentitySite::Terminator(block.block.index()),
                )?;
            }
        }

        let mut roots = RootCollector::new(root_contract);
        self.observe_cfg_roots(&mut roots)?;
        let entry = roots.entry.ok_or(MirRewriteError::InvalidReference {
            expected: callable,
            identity: MirLocalIdentity::Block(self.entry()),
            site: MirLocalIdentitySite::BodyEntry,
            failure: MirReferenceFailure::Unknown,
        })?;
        validate_edit_block_reference(self, entry, MirLocalIdentitySite::BodyEntry)?;
        for root in &roots.protected {
            validate_edit_block_reference(self, root.block, root.site)?;
        }

        build_facts(
            callable,
            entry,
            roots.protected,
            blocks,
            self.value_use_census()?,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MirLocalCfgBlockSnapshot {
    block: BlockId,
    successors: Vec<BlockId>,
    instruction_count: usize,
    terminator_kind: MirLocalCfgTerminatorKind,
}

fn snapshot_block(block: &MirBasicBlock) -> Result<MirLocalCfgBlockSnapshot, MirRewriteError> {
    let terminator = block
        .terminator
        .as_ref()
        .ok_or(MirRewriteError::MissingBlockTerminator { block: block.id })?;
    let terminator_kind = MirLocalCfgTerminatorKind::classify(terminator);
    let successors = terminator.successors().collect::<Vec<_>>();
    debug_assert_eq!(successors.len(), terminator_kind.successor_count());
    Ok(MirLocalCfgBlockSnapshot {
        block: block.id,
        successors,
        instruction_count: block.instructions.len(),
        terminator_kind,
    })
}

fn build_facts(
    callable: CallableId,
    entry: BlockId,
    protected_roots: Vec<MirProtectedBlockRoot>,
    snapshots: Vec<MirLocalCfgBlockSnapshot>,
    census: MirValueUseCensus,
) -> Result<MirLocalCfgFacts, MirRewriteError> {
    let block_order = snapshots
        .iter()
        .map(|block| block.block)
        .collect::<Vec<_>>();
    let adjacency_by_block = snapshots
        .iter()
        .map(|block| (block.block, block.successors.clone()))
        .collect::<BTreeMap<_, _>>();
    let entry_closure = closure([entry], &adjacency_by_block);
    let all_closure = closure(
        std::iter::once(entry).chain(protected_roots.iter().map(|root| root.block)),
        &adjacency_by_block,
    );

    let mut definitions = BTreeMap::<BlockId, Vec<ValueId>>::new();
    for value in census.iter() {
        let Some(site) = value.definition() else {
            // Some intermediate and test MIR snapshots can retain an unused
            // declaration. This query inventories definitions; it does not
            // turn declaration liveness into a new verifier responsibility.
            continue;
        };
        let MirLocalIdentitySite::Instruction { block, .. } = site else {
            return Err(MirRewriteError::InvalidValueDefinitionSite {
                value: value.value(),
                site,
            });
        };
        let block = BlockId::new(callable, block);
        if !adjacency_by_block.contains_key(&block) {
            return Err(MirRewriteError::InvalidReference {
                expected: callable,
                identity: MirLocalIdentity::Block(block),
                site,
                failure: MirReferenceFailure::Unknown,
            });
        }
        definitions.entry(block).or_default().push(value.value());
    }

    let protected_blocks = protected_roots
        .iter()
        .map(|root| root.block)
        .collect::<BTreeSet<_>>();
    let permanent_roots = protected_roots
        .iter()
        .copied()
        .filter(|root| {
            classify_local_identity_site(root.site) == MirIdentitySiteRole::PermanentAttachment
        })
        .collect::<Vec<_>>();
    let permanent_blocks = permanent_roots
        .iter()
        .map(|root| root.block)
        .collect::<BTreeSet<_>>();

    let edges = snapshots
        .iter()
        .flat_map(|block| {
            block
                .successors
                .iter()
                .copied()
                .enumerate()
                .map(|(successor_index, target)| MirLocalCfgEdge {
                    source: block.block,
                    target,
                    successor_index,
                })
        })
        .collect::<Vec<_>>();
    let mut successor_edges = BTreeMap::<BlockId, Vec<MirLocalCfgEdge>>::new();
    let mut predecessor_edges = BTreeMap::<BlockId, Vec<MirLocalCfgEdge>>::new();
    for edge in &edges {
        successor_edges.entry(edge.source).or_default().push(*edge);
        predecessor_edges
            .entry(edge.target)
            .or_default()
            .push(*edge);
    }

    let blocks = snapshots
        .into_iter()
        .map(|snapshot| MirLocalCfgBlockFacts {
            block: snapshot.block,
            successors: snapshot.successors,
            successor_edges: successor_edges.remove(&snapshot.block).unwrap_or_default(),
            predecessor_edges: predecessor_edges
                .remove(&snapshot.block)
                .unwrap_or_default(),
            defined_values: definitions.remove(&snapshot.block).unwrap_or_default(),
            instruction_count: snapshot.instruction_count,
            terminator_kind: snapshot.terminator_kind,
            is_entry: snapshot.block == entry,
            is_protected_root: protected_blocks.contains(&snapshot.block),
            is_permanent_attachment: permanent_blocks.contains(&snapshot.block),
        })
        .collect();
    let select = |members: &BTreeSet<BlockId>| {
        block_order
            .iter()
            .copied()
            .filter(|block| members.contains(block))
            .collect::<Vec<_>>()
    };
    let protected_only = all_closure
        .difference(&entry_closure)
        .copied()
        .collect::<BTreeSet<_>>();
    let unreachable_set = block_order
        .iter()
        .copied()
        .filter(|block| !all_closure.contains(block))
        .collect::<BTreeSet<_>>();

    Ok(MirLocalCfgFacts {
        callable,
        entry,
        protected_roots,
        permanent_roots,
        edges,
        blocks,
        entry_reachable: select(&entry_closure),
        reachable: select(&all_closure),
        protected_but_entry_unreachable: select(&protected_only),
        unreachable: select(&unreachable_set),
    })
}

fn closure(
    roots: impl IntoIterator<Item = BlockId>,
    adjacency: &BTreeMap<BlockId, Vec<BlockId>>,
) -> BTreeSet<BlockId> {
    let mut reachable = BTreeSet::new();
    let mut pending = VecDeque::from_iter(roots);
    while let Some(block) = pending.pop_front() {
        if !reachable.insert(block) {
            continue;
        }
        if let Some(successors) = adjacency.get(&block) {
            pending.extend(successors.iter().copied());
        }
    }
    reachable
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MirCfgRootContract {
    ProofRich,
    Final,
}

struct RootCollector {
    contract: MirCfgRootContract,
    entry: Option<BlockId>,
    protected: Vec<MirProtectedBlockRoot>,
}

impl RootCollector {
    const fn new(contract: MirCfgRootContract) -> Self {
        Self {
            contract,
            entry: None,
            protected: Vec::new(),
        }
    }
}

impl MirLocalIdentityObserver for RootCollector {
    type Error = MirRewriteError;

    fn observe_block(
        &mut self,
        site: MirLocalIdentitySite,
        block: BlockId,
    ) -> Result<(), Self::Error> {
        match classify_local_identity_site(site) {
            MirIdentitySiteRole::BodyEntry => self.entry = Some(block),
            MirIdentitySiteRole::PermanentAttachment => {
                self.protected.push(MirProtectedBlockRoot { site, block });
            }
            MirIdentitySiteRole::ConsumableProof => match self.contract {
                MirCfgRootContract::ProofRich => {
                    self.protected.push(MirProtectedBlockRoot { site, block });
                }
                MirCfgRootContract::Final => {
                    return Err(MirRewriteError::ConsumedProofRootInFinalCfg { block, site });
                }
            },
            MirIdentitySiteRole::Ordinary => {}
        }
        Ok(())
    }
}

fn validate_dense_declaration(
    callable: CallableId,
    index: usize,
    actual: BlockId,
) -> Result<(), MirRewriteError> {
    if actual.callable() != callable {
        return Err(MirRewriteError::ForeignIdentity {
            expected: callable,
            identity: MirLocalIdentity::Block(actual),
        });
    }
    let expected = BlockId::new(callable, index);
    if actual != expected {
        return Err(MirRewriteError::DeclarationIdentityMismatch {
            expected: MirLocalIdentity::Block(expected),
            actual: MirLocalIdentity::Block(actual),
        });
    }
    Ok(())
}

fn validate_dense_block_reference(
    definition: MirDefinitionRef<'_>,
    block: BlockId,
    site: MirLocalIdentitySite,
) -> Result<(), MirRewriteError> {
    let expected = definition.callable();
    if block.callable() != expected {
        return Err(invalid_block_reference(
            expected,
            block,
            site,
            MirReferenceFailure::Foreign,
        ));
    }
    if definition.block(block).is_none() {
        return Err(invalid_block_reference(
            expected,
            block,
            site,
            MirReferenceFailure::Unknown,
        ));
    }
    Ok(())
}

fn validate_edit_block_reference(
    edit: &MirCallableEdit,
    block: BlockId,
    site: MirLocalIdentitySite,
) -> Result<(), MirRewriteError> {
    match edit.block(block) {
        Ok(_) => Ok(()),
        Err(MirRewriteError::ForeignIdentity { .. }) => Err(invalid_block_reference(
            edit.callable(),
            block,
            site,
            MirReferenceFailure::Foreign,
        )),
        Err(MirRewriteError::UnknownIdentity { .. }) => Err(invalid_block_reference(
            edit.callable(),
            block,
            site,
            MirReferenceFailure::Unknown,
        )),
        Err(MirRewriteError::DeletedIdentity { .. }) => Err(invalid_block_reference(
            edit.callable(),
            block,
            site,
            MirReferenceFailure::Deleted,
        )),
        Err(error) => Err(error),
    }
}

fn invalid_block_reference(
    expected: CallableId,
    block: BlockId,
    site: MirLocalIdentitySite,
    failure: MirReferenceFailure,
) -> MirRewriteError {
    MirRewriteError::InvalidReference {
        expected,
        identity: MirLocalIdentity::Block(block),
        site,
        failure,
    }
}

#[cfg(test)]
mod tests;
