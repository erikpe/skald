//! Allocation-free aggregate facts derived from lifecycle planning.

use super::model::PlannedMirProgram;

/// Deterministic summary of the exact static activation decision made during
/// lifecycle planning.
///
/// These values are derived from the already-built planning product. They are
/// suitable for operational reporting without exposing planning-only graph or
/// witness representation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StaticActivationStatistics {
    declared_fields: usize,
    active_fields: usize,
    inactive_fields: usize,
    active_explicit_fields: usize,
    active_zero_default_fields: usize,
    inactive_explicit_fields: usize,
    reachable_execution_nodes: usize,
    activation_edges: usize,
    conservative_targets: usize,
}

impl StaticActivationStatistics {
    pub const fn declared_fields(self) -> usize {
        self.declared_fields
    }

    pub const fn active_fields(self) -> usize {
        self.active_fields
    }

    pub const fn inactive_fields(self) -> usize {
        self.inactive_fields
    }

    pub const fn active_explicit_fields(self) -> usize {
        self.active_explicit_fields
    }

    pub const fn active_zero_default_fields(self) -> usize {
        self.active_zero_default_fields
    }

    pub const fn inactive_explicit_fields(self) -> usize {
        self.inactive_explicit_fields
    }

    pub const fn reachable_execution_nodes(self) -> usize {
        self.reachable_execution_nodes
    }

    pub const fn activation_edges(self) -> usize {
        self.activation_edges
    }

    pub const fn conservative_targets(self) -> usize {
        self.conservative_targets
    }
}

impl PlannedMirProgram {
    /// Returns already-known aggregate facts about the exact activation set.
    ///
    /// The query scans only the declaration inventory and performs no graph
    /// traversal, witness construction, logging, or dump rendering.
    pub fn activation_statistics(&self) -> StaticActivationStatistics {
        let activation = self.planning_report().activation();
        let analysis_counts = activation.counts();
        let mut active_explicit_fields = 0;
        let mut active_zero_default_fields = 0;
        let mut inactive_explicit_fields = 0;
        for field in self.static_fields() {
            match (activation.is_active(field.field), field.initializer) {
                (true, Some(_)) => active_explicit_fields += 1,
                (true, None) => active_zero_default_fields += 1,
                (false, Some(_)) => inactive_explicit_fields += 1,
                (false, None) => {}
            }
        }
        StaticActivationStatistics {
            declared_fields: analysis_counts.declared_fields,
            active_fields: analysis_counts.active_fields,
            inactive_fields: analysis_counts.inactive_fields,
            active_explicit_fields,
            active_zero_default_fields,
            inactive_explicit_fields,
            reachable_execution_nodes: analysis_counts.reachable_execution_nodes,
            activation_edges: analysis_counts.edges,
            conservative_targets: activation
                .target_counts()
                .iter()
                .map(|count| count.targets())
                .sum(),
        }
    }
}
