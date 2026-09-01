//! Deterministic coupled closure over executable work and static fields.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    identity::StaticFieldId,
    mir::{MirExecutionNode, PreliminaryMirProgram, PreliminaryMirStaticField},
    passes::reachability::{
        resolve_entry_execution, resolve_static_field_destruction_dependencies, MirDependencyEdge,
        MirDependencyExtraction, MirDependencyTarget, MirFunctionValueCoupling,
    },
};

use super::{
    StaticActivationAnalysis, StaticActivationAnalysisError, StaticActivationAnalysisParts,
    StaticActivationEdge, StaticActivationExecution, StaticActivationField, StaticActivationNode,
    StaticActivationRoot, StaticActivationWitness,
};

pub(super) fn analyze_static_activation_from_dependencies(
    program: &PreliminaryMirProgram,
    dependencies: &MirDependencyExtraction,
) -> Result<StaticActivationAnalysis, StaticActivationAnalysisError> {
    ActivationSolver::new(program, dependencies)?.solve()
}

struct ActivationSolver<'mir> {
    program: &'mir PreliminaryMirProgram,
    root: StaticActivationRoot,
    fields: BTreeMap<StaticFieldId, PreliminaryMirStaticField>,
    dependencies_by_source: BTreeMap<MirExecutionNode, Vec<MirDependencyEdge>>,
    static_accesses_by_source:
        BTreeMap<MirExecutionNode, Vec<crate::passes::reachability::MirStaticAccess>>,
    function_values: MirFunctionValueCoupling,
    reachable_execution: BTreeSet<MirExecutionNode>,
    active_fields: BTreeSet<StaticFieldId>,
    pending_execution: BTreeSet<MirExecutionNode>,
    pending_fields: BTreeSet<StaticFieldId>,
    witnesses: BTreeMap<super::model::StaticActivationNodeKey, StaticActivationWitness>,
    edges: Vec<StaticActivationEdge>,
    edge_keys: BTreeSet<super::model::StaticActivationEdgeKey>,
}

impl<'mir> ActivationSolver<'mir> {
    fn new(
        program: &'mir PreliminaryMirProgram,
        extraction: &MirDependencyExtraction,
    ) -> Result<Self, StaticActivationAnalysisError> {
        let (entry, span) = resolve_entry_execution(program.program())?;
        let mut dependencies_by_source = BTreeMap::new();
        for dependency in extraction.dependencies() {
            dependencies_by_source
                .entry(dependency.edge().source())
                .or_insert_with(Vec::new)
                .push(*dependency.edge());
        }
        let mut static_accesses_by_source = BTreeMap::new();
        for access in extraction.static_accesses() {
            static_accesses_by_source
                .entry(access.source())
                .or_insert_with(Vec::new)
                .push(*access);
        }
        let function_values = MirFunctionValueCoupling::new(extraction);
        let fields = program
            .static_fields()
            .map(|field| (field.field, *field))
            .collect();
        let root = StaticActivationRoot::new(entry, span);
        let mut reachable_execution = BTreeSet::new();
        reachable_execution.insert(entry);
        let mut pending_execution = BTreeSet::new();
        pending_execution.insert(entry);
        let mut witnesses = BTreeMap::new();
        witnesses.insert(
            super::static_activation_node_key(StaticActivationNode::execution(entry)),
            StaticActivationWitness::new(root, Vec::new()),
        );
        Ok(Self {
            program,
            root,
            fields,
            dependencies_by_source,
            static_accesses_by_source,
            function_values,
            reachable_execution,
            active_fields: BTreeSet::new(),
            pending_execution,
            pending_fields: BTreeSet::new(),
            witnesses,
            edges: Vec::new(),
            edge_keys: BTreeSet::new(),
        })
    }

    fn solve(mut self) -> Result<StaticActivationAnalysis, StaticActivationAnalysisError> {
        loop {
            if let Some(source) = take_first(&mut self.pending_execution) {
                self.process_execution(source)?;
                continue;
            }
            if let Some(field) = take_first(&mut self.pending_fields) {
                self.process_field(field)?;
                continue;
            }
            return self.finish();
        }
    }

    fn process_execution(
        &mut self,
        source: MirExecutionNode,
    ) -> Result<(), StaticActivationAnalysisError> {
        for dependency in self
            .dependencies_by_source
            .get(&source)
            .cloned()
            .unwrap_or_default()
        {
            if let MirDependencyTarget::Execution(target) = dependency.target() {
                self.follow_execution(StaticActivationEdge::execution_dependency(
                    source,
                    target,
                    dependency.kind(),
                    dependency.span(),
                ))?;
            }
        }
        for access in self
            .static_accesses_by_source
            .get(&source)
            .cloned()
            .unwrap_or_default()
        {
            if !access.is_lifecycle_owned() {
                self.activate_field(StaticActivationEdge::static_access(
                    source,
                    access.target(),
                    access.kind(),
                    access.region().static_effect_phase(),
                    access.span(),
                ))?;
            }
        }
        self.process_function_values(source)
    }

    fn process_field(&mut self, field: StaticFieldId) -> Result<(), StaticActivationAnalysisError> {
        let declaration = self.fields.get(&field).copied().ok_or(
            crate::passes::reachability::MirDependencyExtractionError::UnknownStaticField(field),
        )?;
        if let Some(initializer) = declaration.initializer {
            let body = self.program.static_initializer(initializer).ok_or(
                StaticActivationAnalysisError::UnknownStaticInitializer(initializer),
            )?;
            self.follow_execution(StaticActivationEdge::initializer(
                field,
                MirExecutionNode::callable(initializer.into()),
                body.span,
            ))?;
        }
        for dependency in resolve_static_field_destruction_dependencies(self.program, field)? {
            if let MirDependencyTarget::Execution(target) = dependency.target() {
                self.follow_execution(StaticActivationEdge::destruction(
                    field,
                    target,
                    dependency.kind(),
                    declaration.span,
                ))?;
            }
        }
        Ok(())
    }

    fn process_function_values(
        &mut self,
        source: MirExecutionNode,
    ) -> Result<(), StaticActivationAnalysisError> {
        for dependency in self.function_values.reach(source) {
            let MirDependencyTarget::Execution(target) = dependency.target() else {
                unreachable!("function-value coupling returned a non-execution edge");
            };
            self.follow_execution(StaticActivationEdge::execution_dependency(
                dependency.source(),
                target,
                dependency.kind(),
                dependency.span(),
            ))?;
        }
        Ok(())
    }

    fn follow_execution(
        &mut self,
        edge: StaticActivationEdge,
    ) -> Result<(), StaticActivationAnalysisError> {
        self.record_edge(edge);
        let StaticActivationNode::Execution(target) = edge.target() else {
            return Ok(());
        };
        if self.reachable_execution.insert(target) {
            let witness = self.extended_witness(edge)?;
            self.witnesses.insert(
                super::static_activation_node_key(StaticActivationNode::execution(target)),
                witness,
            );
            self.pending_execution.insert(target);
        }
        Ok(())
    }

    fn activate_field(
        &mut self,
        edge: StaticActivationEdge,
    ) -> Result<(), StaticActivationAnalysisError> {
        self.record_edge(edge);
        let StaticActivationNode::Field(field) = edge.target() else {
            return Ok(());
        };
        if !self.fields.contains_key(&field) {
            return Err(
                crate::passes::reachability::MirDependencyExtractionError::UnknownStaticField(
                    field,
                )
                .into(),
            );
        }
        if self.active_fields.insert(field) {
            let witness = self.extended_witness(edge)?;
            self.witnesses.insert(
                super::static_activation_node_key(StaticActivationNode::field(field)),
                witness,
            );
            self.pending_fields.insert(field);
        }
        Ok(())
    }

    fn record_edge(&mut self, edge: StaticActivationEdge) {
        if self
            .edge_keys
            .insert(super::static_activation_edge_key(&edge))
        {
            self.edges.push(edge);
        }
    }

    fn extended_witness(
        &self,
        edge: StaticActivationEdge,
    ) -> Result<StaticActivationWitness, StaticActivationAnalysisError> {
        let source = self
            .witnesses
            .get(&super::static_activation_node_key(edge.source()))
            .ok_or(StaticActivationAnalysisError::MissingWitness(edge.source()))?;
        let mut path = source.edges().to_vec();
        path.push(edge);
        Ok(StaticActivationWitness::new(self.root, path))
    }

    fn finish(self) -> Result<StaticActivationAnalysis, StaticActivationAnalysisError> {
        let mut active_fields = Vec::new();
        for field in &self.active_fields {
            let witness = self
                .witnesses
                .get(&super::static_activation_node_key(
                    StaticActivationNode::field(*field),
                ))
                .cloned()
                .ok_or(StaticActivationAnalysisError::MissingWitness(
                    StaticActivationNode::field(*field),
                ))?;
            active_fields.push(StaticActivationField::new(*field, witness));
        }
        let inactive_fields = self
            .fields
            .keys()
            .filter(|field| !self.active_fields.contains(field))
            .copied()
            .collect();
        let mut reachable_execution = Vec::new();
        for node in &self.reachable_execution {
            let witness = self
                .witnesses
                .get(&super::static_activation_node_key(
                    StaticActivationNode::execution(*node),
                ))
                .cloned()
                .ok_or(StaticActivationAnalysisError::MissingWitness(
                    StaticActivationNode::execution(*node),
                ))?;
            reachable_execution.push(StaticActivationExecution::new(*node, witness));
        }
        Ok(StaticActivationAnalysis::from_parts(
            StaticActivationAnalysisParts {
                active_fields,
                inactive_fields,
                reachable_execution,
                edges: self.edges,
            },
        ))
    }
}

fn take_first<T: Copy + Ord>(values: &mut BTreeSet<T>) -> Option<T> {
    let first = values.iter().next().copied()?;
    values.remove(&first);
    Some(first)
}
