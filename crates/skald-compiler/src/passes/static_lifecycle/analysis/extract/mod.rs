//! Exhaustive MIR and implicit-lifecycle analysis graph extraction.

mod access;
mod control;
mod instruction;

use std::collections::{BTreeMap, BTreeSet};

use crate::passes::reachability::{
    extract_final_dependency_parts, extract_preliminary_dependencies, MirDependencyEdgeKind,
    MirDependencyExtraction, MirDependencyRegion, MirDependencyTarget,
};
use crate::{
    identity::CallableId,
    mir::{
        MirAliasAccess, MirArgument, MirArrayInstruction, MirCallReceiver, MirClassOptionalSource,
        MirDefinitionRef, MirInstruction, MirIoOperation, MirObjectOrigin, MirObjectView,
        MirOptionalSharedSource, MirOptionalSource, MirPlace, MirPlaceBase, MirProgram, MirRvalue,
        MirRvalueKind, MirSharedCastSource, MirStaticInitializerBody, MirTerminator,
        PreliminaryMirProgram,
    },
    source::Span,
};

use super::model::{
    edge_key, evidence_key, StaticAccessEvidence, StaticAccessKind, StaticEffectEdge,
    StaticEffectEdgeKind, StaticEffectNode, StaticEffectPhase, StaticFunctionValueCandidates,
    StaticFunctionValueTarget,
};

#[derive(Default)]
pub(crate) struct NodeDraft {
    pub(crate) direct: Vec<StaticAccessEvidence>,
    pub(crate) edges: Vec<StaticEffectEdge>,
}

pub(crate) struct ExtractedGraph {
    pub(crate) function_value_candidates: Vec<StaticFunctionValueCandidates>,
    pub(crate) nodes: BTreeMap<StaticEffectNode, NodeDraft>,
}

pub(crate) fn extract(program: &PreliminaryMirProgram) -> ExtractedGraph {
    let dependencies = extract_preliminary_dependencies(program)
        .expect("verified preliminary MIR must have valid dependency identities");
    extract_parts(
        program.program(),
        program.static_initializer_bodies(),
        dependencies,
    )
}

pub(crate) fn extract_final(
    program: &MirProgram,
    initializers: &[MirStaticInitializerBody],
) -> ExtractedGraph {
    let dependencies = extract_final_dependency_parts(program, initializers)
        .expect("verified final MIR must have valid dependency identities");
    extract_parts(program, initializers, dependencies)
}

fn extract_parts(
    program: &MirProgram,
    initializers: &[MirStaticInitializerBody],
    dependencies: MirDependencyExtraction,
) -> ExtractedGraph {
    let function_value_candidates = collect_function_value_candidates(&dependencies);
    let mut extractor = Extractor {
        program,
        initializers,
        function_value_candidates,
        nodes: BTreeMap::new(),
    };
    extractor.seed_nodes(&dependencies);
    extractor.install_dependencies(&dependencies);
    extractor.extract_bodies();
    extractor.finish()
}

struct Extractor<'mir> {
    program: &'mir MirProgram,
    initializers: &'mir [MirStaticInitializerBody],
    function_value_candidates: Vec<StaticFunctionValueCandidates>,
    nodes: BTreeMap<StaticEffectNode, NodeDraft>,
}

impl Extractor<'_> {
    fn finish(mut self) -> ExtractedGraph {
        for draft in self.nodes.values_mut() {
            draft.direct.sort_by_key(evidence_key);
            draft
                .direct
                .dedup_by(|left, right| evidence_key(left) == evidence_key(right));
            draft.edges.sort_by_key(edge_key);
            draft.edges.dedup_by(|left, right| {
                left.target == right.target
                    && left.kind == right.kind
                    && left.phase == right.phase
                    && left.span == right.span
            });
        }
        ExtractedGraph {
            function_value_candidates: self.function_value_candidates,
            nodes: self.nodes,
        }
    }

    fn seed_nodes(&mut self, dependencies: &MirDependencyExtraction) {
        for node in dependencies.nodes() {
            self.nodes.entry(*node).or_default();
        }
    }

    fn install_dependencies(&mut self, dependencies: &MirDependencyExtraction) {
        for dependency in dependencies.dependencies() {
            let edge = dependency.edge();
            let MirDependencyTarget::Execution(target) = edge.target() else {
                continue;
            };
            let Some(kind) = static_edge_kind(edge.kind()) else {
                continue;
            };
            self.add_edge(
                edge.source(),
                target,
                kind,
                static_phase(dependency.region()),
                edge.span(),
            );
        }
        for site in dependencies.indirect_calls() {
            for target in dependencies.all_indirect_targets(site.function_type()) {
                self.add_edge(
                    site.source(),
                    StaticEffectNode::callable(target),
                    StaticEffectEdgeKind::IndirectCall,
                    static_phase(site.region()),
                    site.span(),
                );
            }
        }
    }

    fn add_edge(
        &mut self,
        source: StaticEffectNode,
        target: StaticEffectNode,
        kind: StaticEffectEdgeKind,
        phase: StaticEffectPhase,
        span: Span,
    ) {
        debug_assert!(
            self.nodes.contains_key(&target),
            "all graph targets must be seeded"
        );
        self.nodes
            .get_mut(&source)
            .expect("seeded source node")
            .edges
            .push(StaticEffectEdge {
                source,
                target,
                kind,
                phase,
                span,
            });
    }
}

fn collect_function_value_candidates(
    dependencies: &MirDependencyExtraction,
) -> Vec<StaticFunctionValueCandidates> {
    let mut candidates =
        BTreeMap::<crate::identity::FunctionTypeId, BTreeMap<CallableId, Span>>::new();
    for formation in dependencies.callable_addresses() {
        let targets = candidates.entry(formation.function_type()).or_default();
        targets
            .entry(formation.target())
            .and_modify(|span| {
                if super::model::span_key(formation.span()) < super::model::span_key(*span) {
                    *span = formation.span();
                }
            })
            .or_insert(formation.span());
    }
    candidates
        .into_iter()
        .map(|(function_type, targets)| StaticFunctionValueCandidates {
            function_type,
            targets: targets
                .into_iter()
                .map(
                    |(callable, first_reference_span)| StaticFunctionValueTarget {
                        callable,
                        first_reference_span,
                    },
                )
                .collect(),
        })
        .collect()
}

const fn static_phase(region: MirDependencyRegion) -> StaticEffectPhase {
    match region {
        MirDependencyRegion::Ordinary => StaticEffectPhase::Ordinary,
        MirDependencyRegion::StaticInitializerBeforePublication => {
            StaticEffectPhase::InitializerBeforePublication
        }
        MirDependencyRegion::StaticInitializerAfterPublication => {
            StaticEffectPhase::InitializerAfterPublication
        }
        MirDependencyRegion::Copy => StaticEffectPhase::Copy,
        MirDependencyRegion::Destruction => StaticEffectPhase::Destruction,
        MirDependencyRegion::ArrayLifecycle => StaticEffectPhase::ArrayLifecycle,
    }
}

const fn static_edge_kind(kind: MirDependencyEdgeKind) -> Option<StaticEffectEdgeKind> {
    Some(match kind {
        MirDependencyEdgeKind::DirectCall => StaticEffectEdgeKind::DirectCall,
        MirDependencyEdgeKind::StaticCall => StaticEffectEdgeKind::StaticCall,
        MirDependencyEdgeKind::DirectMethodCall => StaticEffectEdgeKind::DirectMethodCall,
        MirDependencyEdgeKind::VirtualDispatch => StaticEffectEdgeKind::VirtualDispatch,
        MirDependencyEdgeKind::InterfaceDispatch => StaticEffectEdgeKind::InterfaceDispatch,
        MirDependencyEdgeKind::CallableAddressRetention
        | MirDependencyEdgeKind::RuntimeEntityReference => return None,
        MirDependencyEdgeKind::IndirectCall => StaticEffectEdgeKind::IndirectCall,
        MirDependencyEdgeKind::Initializer => StaticEffectEdgeKind::Initializer,
        MirDependencyEdgeKind::CopyConstructor => StaticEffectEdgeKind::CopyConstructor,
        MirDependencyEdgeKind::CopyAssignment => StaticEffectEdgeKind::CopyAssignment,
        MirDependencyEdgeKind::UserCopyBody => StaticEffectEdgeKind::UserCopyBody,
        MirDependencyEdgeKind::BaseCopy => StaticEffectEdgeKind::BaseCopy,
        MirDependencyEdgeKind::FieldCopy => StaticEffectEdgeKind::FieldCopy,
        MirDependencyEdgeKind::CompleteFinalizer => StaticEffectEdgeKind::CompleteFinalizer,
        MirDependencyEdgeKind::UserDestructor => StaticEffectEdgeKind::UserDestructor,
        MirDependencyEdgeKind::FieldFinalizer => StaticEffectEdgeKind::FieldFinalizer,
        MirDependencyEdgeKind::BaseFinalizer => StaticEffectEdgeKind::BaseFinalizer,
        MirDependencyEdgeKind::SharedFinalizer => StaticEffectEdgeKind::SharedFinalizer,
        MirDependencyEdgeKind::TemporaryCleanup => StaticEffectEdgeKind::TemporaryCleanup,
        MirDependencyEdgeKind::OptionalLifecycle => StaticEffectEdgeKind::OptionalCleanup,
        MirDependencyEdgeKind::ArrayDefault => StaticEffectEdgeKind::ArrayDefault,
        MirDependencyEdgeKind::ArrayCopy => StaticEffectEdgeKind::ArrayCopy,
        MirDependencyEdgeKind::ArrayAssignment => StaticEffectEdgeKind::ArrayAssignment,
        MirDependencyEdgeKind::ArrayDestruction => StaticEffectEdgeKind::ArrayDestruction,
    })
}
