//! Immutable deterministic whole-program reachability facts and queries.

use crate::{
    identity::{CallableId, FunctionTypeId, InterfaceRequirementId, VirtualFamilyId},
    mir::MirExecutionNode,
    source::Span,
};

use super::{
    mir_execution_node_key, MirCallableAddressFormation, MirDependencyEdge, MirReachabilityRoot,
    MirRetainedDefinition, MirRuntimeEntity, MirStaticAccess,
};

/// Canonical reachable candidates for one exact function type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirReachableFunctionValueCandidates {
    function_type: FunctionTypeId,
    targets: Vec<MirReachableFunctionValueTarget>,
}

impl MirReachableFunctionValueCandidates {
    pub(super) const fn new(
        function_type: FunctionTypeId,
        targets: Vec<MirReachableFunctionValueTarget>,
    ) -> Self {
        Self {
            function_type,
            targets,
        }
    }

    pub(crate) const fn function_type(&self) -> FunctionTypeId {
        self.function_type
    }

    pub(crate) fn targets(&self) -> &[MirReachableFunctionValueTarget] {
        &self.targets
    }
}

/// The canonical first reached formation of one exact callable target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MirReachableFunctionValueTarget {
    callable: CallableId,
    source: MirExecutionNode,
    first_formation_span: Span,
}

impl MirReachableFunctionValueTarget {
    pub(super) const fn from_formation(formation: MirCallableAddressFormation) -> Self {
        Self {
            callable: formation.target(),
            source: formation.source(),
            first_formation_span: formation.span(),
        }
    }

    pub(crate) const fn callable(&self) -> CallableId {
        self.callable
    }

    pub(crate) const fn source(&self) -> MirExecutionNode {
        self.source
    }

    pub(crate) const fn first_formation_span(&self) -> Span {
        self.first_formation_span
    }
}

/// Canonically ordered dependencies whose source is one reachable node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirReachableOutgoingDependencies {
    source: MirExecutionNode,
    dependencies: Vec<MirDependencyEdge>,
}

impl MirReachableOutgoingDependencies {
    pub(super) const fn new(
        source: MirExecutionNode,
        dependencies: Vec<MirDependencyEdge>,
    ) -> Self {
        Self {
            source,
            dependencies,
        }
    }

    pub(crate) const fn source(&self) -> MirExecutionNode {
        self.source
    }

    pub(crate) fn dependencies(&self) -> &[MirDependencyEdge] {
        &self.dependencies
    }
}

/// Canonical first reason and dependency path that reached one execution node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirReachabilityExplanation {
    node: MirExecutionNode,
    root: MirReachabilityRoot,
    dependencies: Vec<MirDependencyEdge>,
}

impl MirReachabilityExplanation {
    pub(super) const fn new(
        node: MirExecutionNode,
        root: MirReachabilityRoot,
        dependencies: Vec<MirDependencyEdge>,
    ) -> Self {
        Self {
            node,
            root,
            dependencies,
        }
    }

    pub(crate) const fn node(&self) -> MirExecutionNode {
        self.node
    }

    pub(crate) const fn root(&self) -> MirReachabilityRoot {
        self.root
    }

    pub(crate) fn dependencies(&self) -> &[MirDependencyEdge] {
        &self.dependencies
    }
}

/// Stable counts derived without rescanning MIR.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirReachabilityCounts {
    pub(crate) roots: usize,
    pub(crate) reachable_nodes: usize,
    pub(crate) reachable_callables: usize,
    pub(crate) retained_definitions: usize,
    pub(crate) dependencies: usize,
    pub(crate) static_accesses: usize,
    pub(crate) runtime_entities: usize,
    pub(crate) virtual_families: usize,
    pub(crate) interface_requirements: usize,
    pub(crate) function_value_signatures: usize,
    pub(crate) function_value_targets: usize,
}

/// Immutable target-independent reachability product for one complete MIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirReachabilityAnalysis {
    roots: Vec<MirReachabilityRoot>,
    reachable_nodes: Vec<MirExecutionNode>,
    reachable_callables: Vec<CallableId>,
    retained_definitions: Vec<MirRetainedDefinition>,
    outgoing: Vec<MirReachableOutgoingDependencies>,
    static_accesses: Vec<MirStaticAccess>,
    function_values: Vec<MirReachableFunctionValueCandidates>,
    runtime_entities: Vec<MirRuntimeEntity>,
    virtual_families: Vec<VirtualFamilyId>,
    interface_requirements: Vec<InterfaceRequirementId>,
    explanations: Vec<MirReachabilityExplanation>,
    counts: MirReachabilityCounts,
}

pub(super) struct MirReachabilityAnalysisParts {
    pub(super) roots: Vec<MirReachabilityRoot>,
    pub(super) reachable_nodes: Vec<MirExecutionNode>,
    pub(super) reachable_callables: Vec<CallableId>,
    pub(super) retained_definitions: Vec<MirRetainedDefinition>,
    pub(super) outgoing: Vec<MirReachableOutgoingDependencies>,
    pub(super) static_accesses: Vec<MirStaticAccess>,
    pub(super) function_values: Vec<MirReachableFunctionValueCandidates>,
    pub(super) runtime_entities: Vec<MirRuntimeEntity>,
    pub(super) virtual_families: Vec<VirtualFamilyId>,
    pub(super) interface_requirements: Vec<InterfaceRequirementId>,
    pub(super) explanations: Vec<MirReachabilityExplanation>,
    pub(super) counts: MirReachabilityCounts,
}

impl MirReachabilityAnalysis {
    pub(super) fn from_parts(parts: MirReachabilityAnalysisParts) -> Self {
        Self {
            roots: parts.roots,
            reachable_nodes: parts.reachable_nodes,
            reachable_callables: parts.reachable_callables,
            retained_definitions: parts.retained_definitions,
            outgoing: parts.outgoing,
            static_accesses: parts.static_accesses,
            function_values: parts.function_values,
            runtime_entities: parts.runtime_entities,
            virtual_families: parts.virtual_families,
            interface_requirements: parts.interface_requirements,
            explanations: parts.explanations,
            counts: parts.counts,
        }
    }

    pub(crate) fn roots(&self) -> &[MirReachabilityRoot] {
        &self.roots
    }

    pub(crate) fn reachable_nodes(&self) -> &[MirExecutionNode] {
        &self.reachable_nodes
    }

    pub(crate) fn is_reachable(&self, node: MirExecutionNode) -> bool {
        self.reachable_nodes
            .binary_search_by_key(&mir_execution_node_key(node), |candidate| {
                mir_execution_node_key(*candidate)
            })
            .is_ok()
    }

    pub(crate) fn reachable_callables(&self) -> &[CallableId] {
        &self.reachable_callables
    }

    pub(crate) fn retained_definitions(&self) -> &[MirRetainedDefinition] {
        &self.retained_definitions
    }

    pub(crate) fn has_retained_definition(&self, callable: CallableId) -> bool {
        self.retained_definitions
            .binary_search_by_key(
                &mir_execution_node_key(MirExecutionNode::callable(callable)),
                |candidate| mir_execution_node_key(candidate.execution_node()),
            )
            .is_ok()
    }

    pub(crate) fn outgoing_dependencies(&self, source: MirExecutionNode) -> &[MirDependencyEdge] {
        self.outgoing
            .binary_search_by_key(&mir_execution_node_key(source), |outgoing| {
                mir_execution_node_key(outgoing.source)
            })
            .ok()
            .map_or(&[], |index| self.outgoing[index].dependencies())
    }

    pub(crate) fn outgoing(&self) -> &[MirReachableOutgoingDependencies] {
        &self.outgoing
    }

    /// Exact direct static-place accesses contained in reachable execution
    /// nodes, in canonical source/effect order.
    pub(crate) fn static_accesses(&self) -> &[MirStaticAccess] {
        &self.static_accesses
    }

    pub(crate) fn static_accesses_from(&self, source: MirExecutionNode) -> &[MirStaticAccess] {
        let source_key = mir_execution_node_key(source);
        let start = self
            .static_accesses
            .partition_point(|access| mir_execution_node_key(access.source()) < source_key);
        let count = self.static_accesses[start..]
            .partition_point(|access| mir_execution_node_key(access.source()) == source_key);
        &self.static_accesses[start..start + count]
    }

    /// Canonical root and dependency path selecting an access's source node.
    pub(crate) fn static_access_explanation(
        &self,
        access: &MirStaticAccess,
    ) -> Option<&MirReachabilityExplanation> {
        self.static_accesses
            .binary_search_by_key(&super::mir_static_access_key(access), |candidate| {
                super::mir_static_access_key(candidate)
            })
            .ok()
            .and_then(|_| self.explanation(access.source()))
    }

    pub(crate) fn function_value_candidates(&self) -> &[MirReachableFunctionValueCandidates] {
        &self.function_values
    }

    pub(crate) fn candidates_for_function_type(
        &self,
        function_type: FunctionTypeId,
    ) -> &[MirReachableFunctionValueTarget] {
        self.function_values
            .binary_search_by_key(&function_type, |candidates| candidates.function_type)
            .ok()
            .map_or(&[], |index| self.function_values[index].targets())
    }

    pub(crate) fn runtime_entities(&self) -> &[MirRuntimeEntity] {
        &self.runtime_entities
    }

    pub(crate) fn used_virtual_families(&self) -> &[VirtualFamilyId] {
        &self.virtual_families
    }

    pub(crate) fn used_interface_requirements(&self) -> &[InterfaceRequirementId] {
        &self.interface_requirements
    }

    pub(crate) fn explanation(
        &self,
        node: MirExecutionNode,
    ) -> Option<&MirReachabilityExplanation> {
        self.explanations
            .binary_search_by_key(&mir_execution_node_key(node), |explanation| {
                mir_execution_node_key(explanation.node)
            })
            .ok()
            .map(|index| &self.explanations[index])
    }

    pub(crate) const fn counts(&self) -> MirReachabilityCounts {
        self.counts
    }
}
