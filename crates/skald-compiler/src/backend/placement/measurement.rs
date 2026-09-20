//! Deterministic structural work observations for placement-check measurements.
use super::{model::*, requirements::Requirements};
use crate::backend::selected::Payload;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::backend) struct PlacementCheckMetrics {
    pub resource_locations: usize,
    pub storage_locations: usize,
    pub abi_locations: usize,
    pub tokens: usize,
    pub state_bits: usize,
    pub selected_events: usize,
    pub transfers: usize,
    pub reachable_blocks: usize,
    pub edge_occurrences: usize,
    pub convergence_rounds: usize,
    pub block_visits: usize,
    pub edge_visits: usize,
    pub fact_removals: usize,
    pub peak_pending_blocks: usize,
}

impl PlacementCheckMetrics {
    pub(super) fn observe_requirements<P: Payload>(
        &mut self,
        draft: &PlacementDraft<'_, '_, P>,
        requirements: &Requirements<'_, P>,
    ) -> Result<(), super::requirements::CheckFailure> {
        for location in &requirements.locations {
            match location {
                Location::Resource(_) => self.resource_locations += 1,
                Location::Storage(_) => self.storage_locations += 1,
                Location::Abi { .. } => self.abi_locations += 1,
            }
        }
        self.tokens = requirements.tokens.len();
        self.state_bits = requirements
            .locations
            .len()
            .checked_mul(self.tokens)
            .ok_or_else(|| {
                requirements.failure(
                    super::requirements::CheckLocation::Entry,
                    super::requirements::CheckReason::Capacity,
                )
            })?;
        self.selected_events = requirements
            .blocks
            .values()
            .flat_map(|block| {
                block
                    .instructions
                    .iter()
                    .copied()
                    .chain(block.terminal.iter().copied())
            })
            .map(|payload| payload.describe().events().count())
            .sum();
        self.transfers = draft.transfers.values().map(Vec::len).sum();
        Ok(())
    }
}
