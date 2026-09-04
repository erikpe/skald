//! Read-only candidate analysis for normalized final-MIR CFG rewrites.
//!
//! These queries deliberately retain only identities and immutable edge
//! facts. Their results are invalidated by the first structural edit.

use std::collections::BTreeMap;

use crate::mir::BlockId;

use super::{MirFinalCfgFacts, MirLocalCfgBlockFacts, MirLocalCfgEdge, MirLocalCfgTerminatorKind};

/// A block which can be removed by redirecting all incoming edges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirEmptyBlockForwardingCandidate {
    block: BlockId,
    direct_target: BlockId,
    incoming_edges: Vec<MirLocalCfgEdge>,
}

impl MirEmptyBlockForwardingCandidate {
    pub(crate) const fn block(&self) -> BlockId {
        self.block
    }

    pub(crate) const fn direct_target(&self) -> BlockId {
        self.direct_target
    }

    pub(crate) fn incoming_edges(&self) -> &[MirLocalCfgEdge] {
        &self.incoming_edges
    }
}

/// The final non-forwardable target for one removable empty block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MirEmptyBlockForwardingResolution {
    block: BlockId,
    target: BlockId,
}

impl MirEmptyBlockForwardingResolution {
    pub(crate) const fn block(self) -> BlockId {
        self.block
    }

    pub(crate) const fn target(self) -> BlockId {
        self.target
    }
}

/// Complete deterministic forwarding authorization for one CFG snapshot.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirEmptyBlockForwardingPlan {
    resolutions: Vec<MirEmptyBlockForwardingResolution>,
}

impl MirEmptyBlockForwardingPlan {
    pub(crate) fn resolutions(&self) -> &[MirEmptyBlockForwardingResolution] {
        &self.resolutions
    }

    pub(crate) fn target_for(&self, block: BlockId) -> Option<BlockId> {
        self.resolutions
            .iter()
            .find(|resolution| resolution.block == block)
            .map(|resolution| resolution.target)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.resolutions.is_empty()
    }
}

/// The first frozen eligibility rule which prevents empty-block forwarding.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirEmptyBlockForwardingBarrierKind {
    BodyEntry,
    PermanentAttachment,
    InstructionBearing,
    NonGotoTerminator,
    SelfLoop,
    IncomingPermanentAttachment,
    Cycle,
    LeadsToCycle,
}

/// One retained block and its deterministic forwarding barrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MirEmptyBlockForwardingBarrier {
    block: BlockId,
    kind: MirEmptyBlockForwardingBarrierKind,
}

impl MirEmptyBlockForwardingBarrier {
    pub(crate) const fn block(self) -> BlockId {
        self.block
    }

    pub(crate) const fn kind(self) -> MirEmptyBlockForwardingBarrierKind {
        self.kind
    }
}

/// Stable aggregate counts for one empty-forwarding query.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirEmptyBlockForwardingCounts {
    examined_blocks: usize,
    candidates: usize,
    barriers: BTreeMap<MirEmptyBlockForwardingBarrierKind, usize>,
}

impl MirEmptyBlockForwardingCounts {
    pub(crate) const fn examined_blocks(&self) -> usize {
        self.examined_blocks
    }

    pub(crate) const fn candidates(&self) -> usize {
        self.candidates
    }

    pub(crate) fn barriers(&self) -> usize {
        self.barriers.values().sum()
    }

    pub(crate) fn barriers_of_kind(&self, kind: MirEmptyBlockForwardingBarrierKind) -> usize {
        self.barriers.get(&kind).copied().unwrap_or_default()
    }
}

/// Immutable forwarding opportunities, resolutions, and refusal reasons.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirEmptyBlockForwardingAnalysis {
    candidates: Vec<MirEmptyBlockForwardingCandidate>,
    plan: MirEmptyBlockForwardingPlan,
    barriers: Vec<MirEmptyBlockForwardingBarrier>,
    counts: MirEmptyBlockForwardingCounts,
}

impl MirEmptyBlockForwardingAnalysis {
    pub(crate) fn candidates(&self) -> &[MirEmptyBlockForwardingCandidate] {
        &self.candidates
    }

    pub(crate) const fn plan(&self) -> &MirEmptyBlockForwardingPlan {
        &self.plan
    }

    pub(crate) fn barriers(&self) -> &[MirEmptyBlockForwardingBarrier] {
        &self.barriers
    }

    pub(crate) const fn counts(&self) -> &MirEmptyBlockForwardingCounts {
        &self.counts
    }
}

#[derive(Clone, Debug)]
struct LocallyForwardableBlock {
    direct_target: BlockId,
    incoming_edges: Vec<MirLocalCfgEdge>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ForwardingResolution {
    Target(BlockId),
    Cycle,
    LeadsToCycle,
}

/// Classifies empty-block forwarding against one normalized final CFG.
pub(crate) fn analyze_empty_block_forwarding(
    facts: &MirFinalCfgFacts,
) -> MirEmptyBlockForwardingAnalysis {
    let mut local = BTreeMap::new();
    let mut initial_barriers = BTreeMap::new();

    for block in facts.blocks() {
        match local_forwarding_shape(facts, block) {
            Ok(candidate) => {
                local.insert(block.block(), candidate);
            }
            Err(kind) => {
                initial_barriers.insert(block.block(), kind);
            }
        }
    }

    let resolutions = resolve_forwarding_targets(facts, &local, &initial_barriers);

    let mut candidates = Vec::new();
    let mut plan_resolutions = Vec::new();
    let mut barriers = Vec::new();
    for block in facts.blocks() {
        let block_id = block.block();
        match resolutions.get(&block_id).copied() {
            Some(ForwardingResolution::Target(target)) => {
                let candidate = &local[&block_id];
                candidates.push(MirEmptyBlockForwardingCandidate {
                    block: block_id,
                    direct_target: candidate.direct_target,
                    incoming_edges: candidate.incoming_edges.clone(),
                });
                plan_resolutions.push(MirEmptyBlockForwardingResolution {
                    block: block_id,
                    target,
                });
            }
            Some(ForwardingResolution::Cycle) => barriers.push(MirEmptyBlockForwardingBarrier {
                block: block_id,
                kind: MirEmptyBlockForwardingBarrierKind::Cycle,
            }),
            Some(ForwardingResolution::LeadsToCycle) => {
                barriers.push(MirEmptyBlockForwardingBarrier {
                    block: block_id,
                    kind: MirEmptyBlockForwardingBarrierKind::LeadsToCycle,
                });
            }
            None => barriers.push(MirEmptyBlockForwardingBarrier {
                block: block_id,
                kind: initial_barriers[&block_id],
            }),
        }
    }

    let counts = forwarding_counts(facts.blocks().len(), candidates.len(), &barriers);
    MirEmptyBlockForwardingAnalysis {
        candidates,
        plan: MirEmptyBlockForwardingPlan {
            resolutions: plan_resolutions,
        },
        barriers,
        counts,
    }
}

fn local_forwarding_shape(
    facts: &MirFinalCfgFacts,
    block: &MirLocalCfgBlockFacts,
) -> Result<LocallyForwardableBlock, MirEmptyBlockForwardingBarrierKind> {
    if block.is_entry() {
        return Err(MirEmptyBlockForwardingBarrierKind::BodyEntry);
    }
    if block.is_permanent_attachment() {
        return Err(MirEmptyBlockForwardingBarrierKind::PermanentAttachment);
    }
    if block.instruction_count() != 0 {
        return Err(MirEmptyBlockForwardingBarrierKind::InstructionBearing);
    }
    if block.terminator_kind() != MirLocalCfgTerminatorKind::Goto {
        return Err(MirEmptyBlockForwardingBarrierKind::NonGotoTerminator);
    }

    let target = block.successor_edges()[0].target();
    if target == block.block() {
        return Err(MirEmptyBlockForwardingBarrierKind::SelfLoop);
    }
    if block.predecessor_edges().iter().any(|edge| {
        facts
            .block(edge.source())
            .expect("CFG edge source belongs to its snapshot")
            .is_permanent_attachment()
    }) {
        return Err(MirEmptyBlockForwardingBarrierKind::IncomingPermanentAttachment);
    }

    Ok(LocallyForwardableBlock {
        direct_target: target,
        incoming_edges: block.predecessor_edges().to_vec(),
    })
}

fn resolve_forwarding_targets(
    facts: &MirFinalCfgFacts,
    local: &BTreeMap<BlockId, LocallyForwardableBlock>,
    initial_barriers: &BTreeMap<BlockId, MirEmptyBlockForwardingBarrierKind>,
) -> BTreeMap<BlockId, ForwardingResolution> {
    let mut resolutions = BTreeMap::new();
    for start in facts.blocks().iter().map(MirLocalCfgBlockFacts::block) {
        if !local.contains_key(&start) || resolutions.contains_key(&start) {
            continue;
        }

        let mut path = Vec::new();
        let mut positions = BTreeMap::new();
        let mut current = start;
        loop {
            if let Some(known) = resolutions.get(&current).copied() {
                let prefix_resolution = match known {
                    ForwardingResolution::Target(target) => ForwardingResolution::Target(target),
                    ForwardingResolution::Cycle | ForwardingResolution::LeadsToCycle => {
                        ForwardingResolution::LeadsToCycle
                    }
                };
                record_path_resolution(&mut resolutions, &path, prefix_resolution);
                break;
            }

            if let Some(cycle_start) = positions.get(&current).copied() {
                record_path_resolution(
                    &mut resolutions,
                    &path[..cycle_start],
                    ForwardingResolution::LeadsToCycle,
                );
                record_path_resolution(
                    &mut resolutions,
                    &path[cycle_start..],
                    ForwardingResolution::Cycle,
                );
                break;
            }

            let Some(candidate) = local.get(&current) else {
                let resolution = if initial_barriers.get(&current)
                    == Some(&MirEmptyBlockForwardingBarrierKind::SelfLoop)
                {
                    ForwardingResolution::LeadsToCycle
                } else {
                    ForwardingResolution::Target(current)
                };
                record_path_resolution(&mut resolutions, &path, resolution);
                break;
            };

            positions.insert(current, path.len());
            path.push(current);
            current = candidate.direct_target;
        }
    }
    resolutions
}

fn record_path_resolution(
    resolutions: &mut BTreeMap<BlockId, ForwardingResolution>,
    path: &[BlockId],
    resolution: ForwardingResolution,
) {
    for block in path {
        resolutions.insert(*block, resolution);
    }
}

fn forwarding_counts(
    examined_blocks: usize,
    candidate_count: usize,
    barriers: &[MirEmptyBlockForwardingBarrier],
) -> MirEmptyBlockForwardingCounts {
    let mut counts = MirEmptyBlockForwardingCounts {
        examined_blocks,
        candidates: candidate_count,
        barriers: BTreeMap::new(),
    };
    for barrier in barriers {
        *counts.barriers.entry(barrier.kind).or_default() += 1;
    }
    counts
}

/// One exact predecessor/successor pair eligible for linear block merging.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MirBasicBlockMergeCandidate {
    predecessor: BlockId,
    successor: BlockId,
}

impl MirBasicBlockMergeCandidate {
    #[cfg(test)]
    pub(crate) const fn unchecked(predecessor: BlockId, successor: BlockId) -> Self {
        Self {
            predecessor,
            successor,
        }
    }

    pub(crate) const fn predecessor(self) -> BlockId {
        self.predecessor
    }

    pub(crate) const fn successor(self) -> BlockId {
        self.successor
    }
}

/// The first frozen eligibility rule which prevents a block merge.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirBasicBlockMergeBarrierKind {
    NonGotoPredecessor,
    SelfLoop,
    PredecessorPermanentAttachment,
    SuccessorIsEntry,
    SuccessorPermanentAttachment,
    NonUniqueIncomingEdge,
}

/// One retained predecessor and its deterministic merge barrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MirBasicBlockMergeBarrier {
    predecessor: BlockId,
    successor: Option<BlockId>,
    kind: MirBasicBlockMergeBarrierKind,
    incoming_edge_count: Option<usize>,
}

impl MirBasicBlockMergeBarrier {
    pub(crate) const fn predecessor(self) -> BlockId {
        self.predecessor
    }

    pub(crate) const fn successor(self) -> Option<BlockId> {
        self.successor
    }

    pub(crate) const fn kind(self) -> MirBasicBlockMergeBarrierKind {
        self.kind
    }

    pub(crate) const fn incoming_edge_count(self) -> Option<usize> {
        self.incoming_edge_count
    }
}

/// Stable aggregate counts for one block-merge query.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirBasicBlockMergeCounts {
    examined_blocks: usize,
    candidates: usize,
    barriers: BTreeMap<MirBasicBlockMergeBarrierKind, usize>,
}

impl MirBasicBlockMergeCounts {
    pub(crate) const fn examined_blocks(&self) -> usize {
        self.examined_blocks
    }

    pub(crate) const fn candidates(&self) -> usize {
        self.candidates
    }

    pub(crate) fn barriers(&self) -> usize {
        self.barriers.values().sum()
    }

    pub(crate) fn barriers_of_kind(&self, kind: MirBasicBlockMergeBarrierKind) -> usize {
        self.barriers.get(&kind).copied().unwrap_or_default()
    }
}

/// Immutable merge opportunities and refusal reasons for one CFG snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirBasicBlockMergeAnalysis {
    candidates: Vec<MirBasicBlockMergeCandidate>,
    barriers: Vec<MirBasicBlockMergeBarrier>,
    counts: MirBasicBlockMergeCounts,
}

impl MirBasicBlockMergeAnalysis {
    pub(crate) fn candidates(&self) -> &[MirBasicBlockMergeCandidate] {
        &self.candidates
    }

    pub(crate) fn first_candidate(&self) -> Option<MirBasicBlockMergeCandidate> {
        self.candidates.first().copied()
    }

    pub(crate) fn barriers(&self) -> &[MirBasicBlockMergeBarrier] {
        &self.barriers
    }

    pub(crate) const fn counts(&self) -> &MirBasicBlockMergeCounts {
        &self.counts
    }
}

/// Classifies block merging against one normalized final CFG.
///
/// Callers must discard this result after any structural edit, rebuild final
/// CFG facts, and call the query again. `first_candidate` therefore gives the
/// deterministic current-block-order choice for a future rescan loop.
pub(crate) fn analyze_basic_block_merging(facts: &MirFinalCfgFacts) -> MirBasicBlockMergeAnalysis {
    let mut candidates = Vec::new();
    let mut barriers = Vec::new();

    for predecessor in facts.blocks() {
        match merge_candidate(facts, predecessor) {
            Ok(candidate) => candidates.push(candidate),
            Err(barrier) => barriers.push(barrier),
        }
    }

    let mut counts = MirBasicBlockMergeCounts {
        examined_blocks: facts.blocks().len(),
        candidates: candidates.len(),
        barriers: BTreeMap::new(),
    };
    for barrier in &barriers {
        *counts.barriers.entry(barrier.kind).or_default() += 1;
    }

    MirBasicBlockMergeAnalysis {
        candidates,
        barriers,
        counts,
    }
}

fn merge_candidate(
    facts: &MirFinalCfgFacts,
    predecessor: &MirLocalCfgBlockFacts,
) -> Result<MirBasicBlockMergeCandidate, MirBasicBlockMergeBarrier> {
    let predecessor_id = predecessor.block();
    if predecessor.terminator_kind() != MirLocalCfgTerminatorKind::Goto {
        return Err(merge_barrier(
            predecessor_id,
            None,
            MirBasicBlockMergeBarrierKind::NonGotoPredecessor,
            None,
        ));
    }

    let successor_id = predecessor.successor_edges()[0].target();
    if predecessor_id == successor_id {
        return Err(merge_barrier(
            predecessor_id,
            Some(successor_id),
            MirBasicBlockMergeBarrierKind::SelfLoop,
            None,
        ));
    }
    if predecessor.is_permanent_attachment() {
        return Err(merge_barrier(
            predecessor_id,
            Some(successor_id),
            MirBasicBlockMergeBarrierKind::PredecessorPermanentAttachment,
            None,
        ));
    }

    let successor = facts
        .block(successor_id)
        .expect("CFG successor belongs to its snapshot");
    if successor.is_entry() {
        return Err(merge_barrier(
            predecessor_id,
            Some(successor_id),
            MirBasicBlockMergeBarrierKind::SuccessorIsEntry,
            None,
        ));
    }
    if successor.is_permanent_attachment() {
        return Err(merge_barrier(
            predecessor_id,
            Some(successor_id),
            MirBasicBlockMergeBarrierKind::SuccessorPermanentAttachment,
            None,
        ));
    }

    let incoming_edge_count = successor.predecessor_edges().len();
    if incoming_edge_count != 1 {
        return Err(merge_barrier(
            predecessor_id,
            Some(successor_id),
            MirBasicBlockMergeBarrierKind::NonUniqueIncomingEdge,
            Some(incoming_edge_count),
        ));
    }

    Ok(MirBasicBlockMergeCandidate {
        predecessor: predecessor_id,
        successor: successor_id,
    })
}

const fn merge_barrier(
    predecessor: BlockId,
    successor: Option<BlockId>,
    kind: MirBasicBlockMergeBarrierKind,
    incoming_edge_count: Option<usize>,
) -> MirBasicBlockMergeBarrier {
    MirBasicBlockMergeBarrier {
        predecessor,
        successor,
        kind,
        incoming_edge_count,
    }
}

#[cfg(test)]
mod tests;
