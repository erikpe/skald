//! Shared exact-function-type coupling for reached function-value operations.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    identity::{CallableId, FunctionTypeId},
    mir::MirExecutionNode,
};

use super::{
    mir_dependency_edge_key, mir_execution_node_key, mir_span_key, MirCallableAddressFormation,
    MirDependencyEdge, MirDependencyEdgeKey, MirDependencyEdgeKind, MirDependencyExtraction,
    MirDependencyRegion, MirDependencyTarget, MirIndirectCallSite,
};

type MirFormationKey = ((u8, usize, usize, usize), (usize, usize, usize));
type MirIndirectCallSiteKey = (
    (u8, usize, usize, usize),
    MirDependencyRegion,
    (usize, usize, usize),
);

/// Couples reached callable-address formations and indirect-call sites.
///
/// Consumers submit each reached execution node. The worklist returns only
/// newly selected indirect execution edges, in canonical order, while keeping
/// target evidence available for final reachability queries.
pub(crate) struct MirFunctionValueCoupling {
    formations_by_source: BTreeMap<MirExecutionNode, Vec<MirCallableAddressFormation>>,
    indirect_calls_by_source: BTreeMap<MirExecutionNode, Vec<MirIndirectCallSite>>,
    reached_sources: BTreeSet<MirExecutionNode>,
    candidates: BTreeMap<FunctionTypeId, BTreeMap<CallableId, MirCallableAddressFormation>>,
    active_sites: BTreeMap<FunctionTypeId, BTreeMap<MirIndirectCallSiteKey, MirIndirectCallSite>>,
}

impl MirFunctionValueCoupling {
    pub(crate) fn new(extraction: &MirDependencyExtraction) -> Self {
        Self::from_parts(
            extraction.callable_addresses().iter().copied(),
            extraction.indirect_calls().iter().copied(),
        )
    }

    fn from_parts(
        formations: impl IntoIterator<Item = MirCallableAddressFormation>,
        sites: impl IntoIterator<Item = MirIndirectCallSite>,
    ) -> Self {
        let mut formations_by_source = BTreeMap::new();
        for formation in formations {
            formations_by_source
                .entry(formation.source())
                .or_insert_with(Vec::new)
                .push(formation);
        }
        for formations in formations_by_source.values_mut() {
            formations.sort_by_key(|formation| {
                (
                    formation.function_type(),
                    formation.target(),
                    formation_key(*formation),
                )
            });
        }

        let mut indirect_calls_by_source = BTreeMap::new();
        for site in sites {
            indirect_calls_by_source
                .entry(site.source())
                .or_insert_with(Vec::new)
                .push(site);
        }
        for sites in indirect_calls_by_source.values_mut() {
            sites.sort_by_key(|site| (site.function_type(), indirect_call_site_key(*site)));
        }

        Self {
            formations_by_source,
            indirect_calls_by_source,
            reached_sources: BTreeSet::new(),
            candidates: BTreeMap::new(),
            active_sites: BTreeMap::new(),
        }
    }

    /// Records one newly reached source and returns its newly selected edges.
    pub(crate) fn reach(&mut self, source: MirExecutionNode) -> Vec<MirDependencyEdge> {
        if !self.reached_sources.insert(source) {
            return Vec::new();
        }

        let mut edges = BTreeMap::<MirDependencyEdgeKey, MirDependencyEdge>::new();
        for formation in self
            .formations_by_source
            .get(&source)
            .cloned()
            .unwrap_or_default()
        {
            self.record_formation(formation, &mut edges);
        }
        for site in self
            .indirect_calls_by_source
            .get(&source)
            .cloned()
            .unwrap_or_default()
        {
            self.record_site(site, &mut edges);
        }
        edges.into_values().collect()
    }

    pub(crate) fn into_candidates(
        self,
    ) -> impl Iterator<Item = (FunctionTypeId, Vec<MirCallableAddressFormation>)> {
        self.candidates
            .into_iter()
            .map(|(function_type, candidates)| (function_type, candidates.into_values().collect()))
    }

    fn record_formation(
        &mut self,
        formation: MirCallableAddressFormation,
        edges: &mut BTreeMap<MirDependencyEdgeKey, MirDependencyEdge>,
    ) {
        let candidates = self
            .candidates
            .entry(formation.function_type())
            .or_default();
        let is_new = match candidates.entry(formation.target()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(formation);
                true
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if formation_key(formation) < formation_key(*entry.get()) {
                    entry.insert(formation);
                }
                false
            }
        };
        if !is_new {
            return;
        }

        for site in self
            .active_sites
            .get(&formation.function_type())
            .into_iter()
            .flat_map(|sites| sites.values())
            .copied()
        {
            record_indirect_edge(edges, site, formation.target());
        }
    }

    fn record_site(
        &mut self,
        site: MirIndirectCallSite,
        edges: &mut BTreeMap<MirDependencyEdgeKey, MirDependencyEdge>,
    ) {
        let sites = self.active_sites.entry(site.function_type()).or_default();
        if sites.insert(indirect_call_site_key(site), site).is_some() {
            return;
        }

        for target in self
            .candidates
            .get(&site.function_type())
            .into_iter()
            .flat_map(|candidates| candidates.keys())
            .copied()
        {
            record_indirect_edge(edges, site, target);
        }
    }
}

fn record_indirect_edge(
    edges: &mut BTreeMap<MirDependencyEdgeKey, MirDependencyEdge>,
    site: MirIndirectCallSite,
    target: CallableId,
) {
    let edge = MirDependencyEdge::new(
        site.source(),
        MirDependencyTarget::Execution(MirExecutionNode::callable(target)),
        MirDependencyEdgeKind::IndirectCall,
        site.span(),
    );
    edges.entry(mir_dependency_edge_key(&edge)).or_insert(edge);
}

fn formation_key(formation: MirCallableAddressFormation) -> MirFormationKey {
    (
        mir_execution_node_key(formation.source()),
        mir_span_key(formation.span()),
    )
}

fn indirect_call_site_key(site: MirIndirectCallSite) -> MirIndirectCallSiteKey {
    (
        mir_execution_node_key(site.source()),
        site.region(),
        mir_span_key(site.span()),
    )
}

#[cfg(test)]
mod tests;
