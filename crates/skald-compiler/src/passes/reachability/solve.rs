//! Iterative deterministic closure, including the coupled function-value fixed point.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    identity::{CallableId, FunctionTypeId},
    mir::{MirExecutionNode, MirProgram},
};

use super::{
    analysis::MirReachabilityAnalysisParts, extract_final_dependencies, mir_dependency_edge_key,
    mir_execution_node_key, roots::collect_reachability_roots, MirCallableAddressFormation,
    MirDependencyEdge, MirDependencyEdgeKey, MirDependencyEdgeKind, MirDependencyExtraction,
    MirDependencyExtractionError, MirDependencyTarget, MirIndirectCallSite,
    MirReachabilityAnalysis, MirReachabilityCounts, MirReachabilityExplanation,
    MirReachabilityRoot, MirReachabilityRootTarget, MirReachableFunctionValueCandidates,
    MirReachableFunctionValueTarget, MirReachableOutgoingDependencies, MirRetainedDefinition,
    MirRuntimeEntity, MirStaticAccess,
};

pub(crate) fn analyze_reachability(
    program: &MirProgram,
) -> Result<MirReachabilityAnalysis, MirDependencyExtractionError> {
    let dependencies = extract_final_dependencies(program)?;
    let roots = collect_reachability_roots(program)?;
    ClosureSolver::new(program, dependencies, roots).solve()
}

struct ClosureSolver<'mir> {
    program: &'mir MirProgram,
    dependencies_by_source: BTreeMap<MirExecutionNode, Vec<MirDependencyEdge>>,
    formations_by_source: BTreeMap<MirExecutionNode, Vec<MirCallableAddressFormation>>,
    indirect_calls_by_source: BTreeMap<MirExecutionNode, Vec<MirIndirectCallSite>>,
    static_accesses_by_source: BTreeMap<MirExecutionNode, Vec<MirStaticAccess>>,
    roots: Vec<MirReachabilityRoot>,
    reachable: BTreeSet<MirExecutionNode>,
    pending: VecDeque<MirExecutionNode>,
    runtime_entities: BTreeSet<MirRuntimeEntity>,
    candidates: BTreeMap<FunctionTypeId, BTreeMap<CallableId, MirCallableAddressFormation>>,
    active_indirect_calls: BTreeMap<FunctionTypeId, Vec<MirIndirectCallSite>>,
    dependencies: Vec<MirDependencyEdge>,
    dependency_keys: BTreeSet<MirDependencyEdgeKey>,
    explanations: BTreeMap<MirExecutionNode, MirReachabilityExplanation>,
}

impl<'mir> ClosureSolver<'mir> {
    fn new(
        program: &'mir MirProgram,
        extraction: MirDependencyExtraction,
        roots: super::roots::MirReachabilityRoots,
    ) -> Self {
        let mut dependencies_by_source = BTreeMap::new();
        for dependency in extraction.dependencies() {
            dependencies_by_source
                .entry(dependency.edge().source())
                .or_insert_with(Vec::new)
                .push(*dependency.edge());
        }
        let mut formations_by_source = BTreeMap::new();
        for formation in extraction.callable_addresses() {
            formations_by_source
                .entry(formation.source())
                .or_insert_with(Vec::new)
                .push(*formation);
        }
        let mut indirect_calls_by_source = BTreeMap::new();
        for site in extraction.indirect_calls() {
            indirect_calls_by_source
                .entry(site.source())
                .or_insert_with(Vec::new)
                .push(*site);
        }
        let mut static_accesses_by_source = BTreeMap::new();
        for access in extraction.static_accesses() {
            static_accesses_by_source
                .entry(access.source())
                .or_insert_with(Vec::new)
                .push(*access);
        }
        Self {
            program,
            dependencies_by_source,
            formations_by_source,
            indirect_calls_by_source,
            static_accesses_by_source,
            roots: roots.roots,
            reachable: BTreeSet::new(),
            pending: VecDeque::new(),
            runtime_entities: roots.runtime_entities.into_iter().collect(),
            candidates: BTreeMap::new(),
            active_indirect_calls: BTreeMap::new(),
            dependencies: Vec::new(),
            dependency_keys: BTreeSet::new(),
            explanations: BTreeMap::new(),
        }
    }

    fn solve(mut self) -> Result<MirReachabilityAnalysis, MirDependencyExtractionError> {
        self.seed_roots();
        while let Some(source) = self.pending.pop_front() {
            self.process_dependencies(source)?;
            self.process_formations(source)?;
            self.process_indirect_calls(source)?;
        }
        Ok(self.finish())
    }

    fn seed_roots(&mut self) {
        for root in self.roots.clone() {
            match root.target() {
                MirReachabilityRootTarget::Execution(node) => {
                    if self.reachable.insert(node) {
                        self.pending.push_back(node);
                        self.explanations.insert(
                            node,
                            MirReachabilityExplanation::new(node, root, Vec::new()),
                        );
                    }
                }
                MirReachabilityRootTarget::RuntimeEntity(entity) => {
                    self.runtime_entities.insert(entity);
                }
            }
        }
    }

    fn process_dependencies(
        &mut self,
        source: MirExecutionNode,
    ) -> Result<(), MirDependencyExtractionError> {
        let dependencies = self
            .dependencies_by_source
            .get(&source)
            .cloned()
            .unwrap_or_default();
        for dependency in dependencies {
            self.follow(dependency)?;
        }
        Ok(())
    }

    fn process_formations(
        &mut self,
        source: MirExecutionNode,
    ) -> Result<(), MirDependencyExtractionError> {
        let formations = self
            .formations_by_source
            .get(&source)
            .cloned()
            .unwrap_or_default();
        for formation in formations {
            let targets = self
                .candidates
                .entry(formation.function_type())
                .or_default();
            let is_new = match targets.entry(formation.target()) {
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
            if is_new {
                let active_sites = self
                    .active_indirect_calls
                    .get(&formation.function_type())
                    .cloned()
                    .unwrap_or_default();
                for site in active_sites {
                    self.follow_indirect(site, formation.target())?;
                }
            }
        }
        Ok(())
    }

    fn process_indirect_calls(
        &mut self,
        source: MirExecutionNode,
    ) -> Result<(), MirDependencyExtractionError> {
        let sites = self
            .indirect_calls_by_source
            .get(&source)
            .cloned()
            .unwrap_or_default();
        for site in sites {
            self.active_indirect_calls
                .entry(site.function_type())
                .or_default()
                .push(site);
            let targets = self
                .candidates
                .get(&site.function_type())
                .map(|candidates| candidates.keys().copied().collect::<Vec<_>>())
                .unwrap_or_default();
            for target in targets {
                self.follow_indirect(site, target)?;
            }
        }
        Ok(())
    }

    fn follow_indirect(
        &mut self,
        site: MirIndirectCallSite,
        target: CallableId,
    ) -> Result<(), MirDependencyExtractionError> {
        self.follow(MirDependencyEdge::new(
            site.source(),
            MirDependencyTarget::Execution(MirExecutionNode::callable(target)),
            MirDependencyEdgeKind::IndirectCall,
            site.span(),
        ))
    }

    fn follow(
        &mut self,
        dependency: MirDependencyEdge,
    ) -> Result<(), MirDependencyExtractionError> {
        let key = mir_dependency_edge_key(&dependency);
        if self.dependency_keys.insert(key) {
            self.dependencies.push(dependency);
        }
        match dependency.target() {
            MirDependencyTarget::Execution(target) => {
                if !self.reachable.contains(&target) {
                    let source = self.explanations.get(&dependency.source()).ok_or(
                        MirDependencyExtractionError::MissingReachabilityExplanation(
                            dependency.source(),
                        ),
                    )?;
                    let mut path = source.dependencies().to_vec();
                    path.push(dependency);
                    let root = source.root();
                    self.reachable.insert(target);
                    self.pending.push_back(target);
                    self.explanations
                        .insert(target, MirReachabilityExplanation::new(target, root, path));
                }
            }
            MirDependencyTarget::RuntimeEntity(entity) => {
                self.runtime_entities.insert(entity);
            }
            MirDependencyTarget::External(_) | MirDependencyTarget::Intrinsic(_) => {}
        }
        Ok(())
    }

    fn finish(mut self) -> MirReachabilityAnalysis {
        let mut reachable_nodes = self.reachable.into_iter().collect::<Vec<_>>();
        reachable_nodes.sort_by_key(|node| mir_execution_node_key(*node));
        let reachable_callables = reachable_nodes
            .iter()
            .filter_map(|node| match node {
                MirExecutionNode::Callable(callable) => Some(*callable),
                MirExecutionNode::ClassLifecycle { .. }
                | MirExecutionNode::ArrayLifecycle { .. } => None,
            })
            .collect::<Vec<_>>();

        let mut retained_definitions = self
            .program
            .executable_definitions()
            .map(|definition| MirRetainedDefinition::new(definition.callable()))
            .collect::<Vec<_>>();
        retained_definitions
            .sort_by_key(|definition| mir_execution_node_key(definition.execution_node()));
        retained_definitions.dedup();

        self.dependencies.sort_by_key(mir_dependency_edge_key);
        let dependency_count = self.dependencies.len();
        let mut dependencies_by_source = BTreeMap::new();
        for dependency in self.dependencies {
            dependencies_by_source
                .entry(dependency.source())
                .or_insert_with(Vec::new)
                .push(dependency);
        }
        let outgoing = reachable_nodes
            .iter()
            .filter_map(|node| {
                dependencies_by_source
                    .remove(node)
                    .map(|dependencies| MirReachableOutgoingDependencies::new(*node, dependencies))
            })
            .collect();
        let static_accesses = reachable_nodes
            .iter()
            .flat_map(|node| {
                self.static_accesses_by_source
                    .get(node)
                    .into_iter()
                    .flatten()
                    .copied()
            })
            .collect::<Vec<_>>();

        let mut function_values = self
            .candidates
            .into_iter()
            .map(|(function_type, candidates)| {
                let mut targets = candidates
                    .into_values()
                    .map(MirReachableFunctionValueTarget::from_formation)
                    .collect::<Vec<_>>();
                targets.sort_by_key(|target| {
                    mir_execution_node_key(MirExecutionNode::callable(target.callable()))
                });
                MirReachableFunctionValueCandidates::new(function_type, targets)
            })
            .collect::<Vec<_>>();
        function_values.sort_by_key(|candidates| candidates.function_type());

        let runtime_entities = self.runtime_entities.into_iter().collect::<Vec<_>>();
        let virtual_families = runtime_entities
            .iter()
            .filter_map(|entity| match entity {
                MirRuntimeEntity::VirtualFamily(family) => Some(*family),
                MirRuntimeEntity::ClassDispatch(_)
                | MirRuntimeEntity::InterfaceRequirement(_)
                | MirRuntimeEntity::FunctionType(_)
                | MirRuntimeEntity::ArrayLifecycle(_)
                | MirRuntimeEntity::OptionalLifecycle(_)
                | MirRuntimeEntity::OptionalBoxLayout(_)
                | MirRuntimeEntity::StaticStorage(_)
                | MirRuntimeEntity::LiteralBacking(_) => None,
            })
            .collect::<Vec<_>>();
        let interface_requirements = runtime_entities
            .iter()
            .filter_map(|entity| match entity {
                MirRuntimeEntity::InterfaceRequirement(requirement) => Some(*requirement),
                MirRuntimeEntity::ClassDispatch(_)
                | MirRuntimeEntity::VirtualFamily(_)
                | MirRuntimeEntity::FunctionType(_)
                | MirRuntimeEntity::ArrayLifecycle(_)
                | MirRuntimeEntity::OptionalLifecycle(_)
                | MirRuntimeEntity::OptionalBoxLayout(_)
                | MirRuntimeEntity::StaticStorage(_)
                | MirRuntimeEntity::LiteralBacking(_) => None,
            })
            .collect::<Vec<_>>();

        let mut explanations = self.explanations.into_values().collect::<Vec<_>>();
        explanations.sort_by_key(|explanation| mir_execution_node_key(explanation.node()));
        let counts = MirReachabilityCounts {
            roots: self.roots.len(),
            reachable_nodes: reachable_nodes.len(),
            reachable_callables: reachable_callables.len(),
            retained_definitions: retained_definitions.len(),
            dependencies: dependency_count,
            static_accesses: static_accesses.len(),
            runtime_entities: runtime_entities.len(),
            virtual_families: virtual_families.len(),
            interface_requirements: interface_requirements.len(),
            function_value_signatures: function_values.len(),
            function_value_targets: function_values
                .iter()
                .map(|candidates| candidates.targets().len())
                .sum(),
        };
        MirReachabilityAnalysis::from_parts(MirReachabilityAnalysisParts {
            roots: self.roots,
            reachable_nodes,
            reachable_callables,
            retained_definitions,
            outgoing,
            static_accesses,
            function_values,
            runtime_entities,
            virtual_families,
            interface_requirements,
            explanations,
            counts,
        })
    }
}

type MirFormationKey = ((u8, usize, usize, usize), (usize, usize, usize));

fn formation_key(formation: MirCallableAddressFormation) -> MirFormationKey {
    (
        mir_execution_node_key(formation.source()),
        super::mir_span_key(formation.span()),
    )
}
