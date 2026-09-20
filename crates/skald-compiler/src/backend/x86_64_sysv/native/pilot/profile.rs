//! Test-only native-pipeline phase and placement-work observations.
use super::super::placement::NativePlacementProfile;
use std::time::Duration;

#[derive(Clone, Copy)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) enum Phase {
    Planning,
    DiscoveryLowering,
    ExecutableLowering,
    Selection,
    FramePlanning,
    RealizationChecking,
    Publication,
}

#[derive(Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct NativePilotProfile {
    pub total: Duration,
    planning: Duration,
    discovery_lowering: Duration,
    executable_lowering: Duration,
    selection: Duration,
    frame_planning: Duration,
    realization_checking: Duration,
    publication: Duration,
    pub placements: Vec<NativePlacementProfile>,
}

impl NativePilotProfile {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn add(&mut self, phase: Phase, elapsed: Duration) {
        *match phase {
            Phase::Planning => &mut self.planning,
            Phase::DiscoveryLowering => &mut self.discovery_lowering,
            Phase::ExecutableLowering => &mut self.executable_lowering,
            Phase::Selection => &mut self.selection,
            Phase::FramePlanning => &mut self.frame_planning,
            Phase::RealizationChecking => &mut self.realization_checking,
            Phase::Publication => &mut self.publication,
        } += elapsed;
    }

    #[cfg(test)]
    pub(super) fn render(&self) -> String {
        let production = self
            .placements
            .iter()
            .map(|placement| placement.production)
            .sum::<Duration>();
        let checking = self
            .placements
            .iter()
            .map(|placement| placement.checking)
            .sum::<Duration>();
        let checks = self.placements.iter().map(|placement| &placement.check);
        let sum = |select: fn(&crate::backend::placement::PlacementCheckMetrics) -> usize| {
            checks.clone().map(select).sum::<usize>()
        };
        let peak_pending_blocks = checks
            .clone()
            .map(|metrics| metrics.peak_pending_blocks)
            .max()
            .unwrap_or(0);
        format!(
            concat!(
                "SKALD_PLACEMENT_PROFILE {{\"format\":1,",
                "\"total_ns\":{},\"planning_ns\":{},",
                "\"discovery_lowering_ns\":{},\"executable_lowering_ns\":{},",
                "\"selection_ns\":{},\"placement_production_ns\":{},",
                "\"placement_checking_ns\":{},\"frame_planning_ns\":{},",
                "\"realization_checking_ns\":{},\"publication_ns\":{},",
                "\"callables\":{},\"resource_locations\":{},",
                "\"storage_locations\":{},\"abi_locations\":{},",
                "\"tokens\":{},\"state_bits\":{},\"selected_events\":{},",
                "\"transfers\":{},\"reachable_blocks\":{},",
                "\"edge_occurrences\":{},\"convergence_rounds\":{},",
                "\"block_visits\":{},\"edge_visits\":{},",
                "\"fact_removals\":{},\"peak_pending_blocks\":{}}}"
            ),
            self.total.as_nanos(),
            self.planning.as_nanos(),
            self.discovery_lowering.as_nanos(),
            self.executable_lowering.as_nanos(),
            self.selection.as_nanos(),
            production.as_nanos(),
            checking.as_nanos(),
            self.frame_planning.as_nanos(),
            self.realization_checking.as_nanos(),
            self.publication.as_nanos(),
            self.placements.len(),
            sum(|metrics| metrics.resource_locations),
            sum(|metrics| metrics.storage_locations),
            sum(|metrics| metrics.abi_locations),
            sum(|metrics| metrics.tokens),
            sum(|metrics| metrics.state_bits),
            sum(|metrics| metrics.selected_events),
            sum(|metrics| metrics.transfers),
            sum(|metrics| metrics.reachable_blocks),
            sum(|metrics| metrics.edge_occurrences),
            sum(|metrics| metrics.convergence_rounds),
            sum(|metrics| metrics.block_visits),
            sum(|metrics| metrics.edge_visits),
            sum(|metrics| metrics.fact_removals),
            peak_pending_blocks,
        )
    }
}
