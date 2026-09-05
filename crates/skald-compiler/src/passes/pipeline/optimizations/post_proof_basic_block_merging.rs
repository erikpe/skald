//! Normalized linear basic-block merging.

use crate::mir::rewrite::{
    analyze_basic_block_merging, final_cfg_facts_for_definition, MirBasicBlockMergeAnalysis,
    MirBasicBlockMergeBarrierKind,
};

use super::super::{
    execution::{
        MirFinalPassCapability, MirFinalPassOutcome, MirPassData, MirPassFailure,
        MirPassMeasurement,
    },
    policy::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
    MirPassIdentity, MirPassStage,
};

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(8);
const NAME: &str = "post-proof-basic-block-merging";
const DESCRIPTION: &str =
    "Fuses maximal eligible single-incoming goto chains while preserving operation order.";
const MERGED_BLOCK_PAIRS: &str = "merged block pairs";
const MOVED_INSTRUCTIONS: &str = "moved instructions";
const REMOVED_BLOCKS: &str = "removed blocks";
const RETAINED_MULTIPLE_INCOMING_EDGE_BARRIERS: &str = "retained multiple-incoming-edge barriers";
const RETAINED_PERMANENT_ATTACHMENT_BARRIERS: &str = "retained permanent-attachment barriers";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, MirPassStage::Final, NAME, DESCRIPTION),
    MirPassImplementation::final_stage(IDENTITY, transform),
);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct MergeCounts {
    merged_pairs: usize,
    moved_instructions: usize,
    retained_multiple_incoming_edge_barriers: usize,
    retained_permanent_attachment_barriers: usize,
}

impl MergeCounts {
    fn add_final_analysis(&mut self, analysis: &MirBasicBlockMergeAnalysis) {
        let barriers = analysis.counts();
        self.retained_multiple_incoming_edge_barriers = self
            .retained_multiple_incoming_edge_barriers
            .saturating_add(
                barriers.barriers_of_kind(MirBasicBlockMergeBarrierKind::NonUniqueIncomingEdge),
            );
        let permanent_attachment_barriers = barriers
            .barriers_of_kind(MirBasicBlockMergeBarrierKind::PredecessorPermanentAttachment)
            .saturating_add(
                barriers
                    .barriers_of_kind(MirBasicBlockMergeBarrierKind::SuccessorPermanentAttachment),
            );
        self.retained_permanent_attachment_barriers = self
            .retained_permanent_attachment_barriers
            .saturating_add(permanent_attachment_barriers);
    }
}

fn transform(capability: MirFinalPassCapability) -> Result<MirFinalPassOutcome, MirPassFailure> {
    let mut processed_callables = 0usize;
    let mut has_candidate = false;
    let mut unchanged_counts = MergeCounts::default();

    for definition in capability.verified().program().executable_definitions() {
        processed_callables = processed_callables.saturating_add(1);
        let facts = final_cfg_facts_for_definition(definition).map_err(MirPassFailure::Rewrite)?;
        let analysis = analyze_basic_block_merging(&facts);
        if analysis.first_candidate().is_some() {
            has_candidate = true;
        } else {
            unchanged_counts.add_final_analysis(&analysis);
        }
    }

    if !has_candidate {
        return capability.unchanged_with(pass_data(processed_callables, 0, unchanged_counts));
    }

    let mut changed_callables = 0usize;
    let mut counts = MergeCounts::default();
    let rewritten = capability.rewrite_cfg(|_, edit| {
        let mut callable_changed = false;
        loop {
            let facts = edit.facts()?;
            let analysis = analyze_basic_block_merging(&facts);
            let Some(candidate) = analysis.first_candidate() else {
                counts.add_final_analysis(&analysis);
                break;
            };

            let merged = edit.merge_basic_blocks(&facts, candidate)?;
            callable_changed = true;
            counts.merged_pairs = counts.merged_pairs.saturating_add(1);
            counts.moved_instructions = counts
                .moved_instructions
                .saturating_add(merged.moved_instructions());
        }
        if callable_changed {
            changed_callables = changed_callables.saturating_add(1);
        }
        Ok(())
    })?;

    rewritten.finish(pass_data(0, changed_callables, counts))
}

fn pass_data(
    processed_callables: usize,
    changed_callables: usize,
    counts: MergeCounts,
) -> MirPassData {
    let data = if changed_callables == 0 {
        MirPassData::processed(processed_callables)
    } else {
        MirPassData::changed(changed_callables)
    };
    data.with_measurement(MirPassMeasurement::count(
        MERGED_BLOCK_PAIRS,
        count(counts.merged_pairs),
    ))
    .with_measurement(MirPassMeasurement::count(
        MOVED_INSTRUCTIONS,
        count(counts.moved_instructions),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_BLOCKS,
        count(counts.merged_pairs),
    ))
    .with_measurement(MirPassMeasurement::count(
        RETAINED_MULTIPLE_INCOMING_EDGE_BARRIERS,
        count(counts.retained_multiple_incoming_edge_barriers),
    ))
    .with_measurement(MirPassMeasurement::count(
        RETAINED_PERMANENT_ATTACHMENT_BARRIERS,
        count(counts.retained_permanent_attachment_barriers),
    ))
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "post_proof_basic_block_merging/tests.rs"]
mod tests;
