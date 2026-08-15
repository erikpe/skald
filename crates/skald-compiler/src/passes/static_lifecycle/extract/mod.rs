//! Exhaustive MIR and implicit-lifecycle graph extraction.

mod access;
mod control;
mod edges;
mod function_values;
mod instruction;
mod lifecycle;

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    identity::{CallableId, CopyAssignmentId, CopyConstructorId},
    mir::{
        MirAliasAccess, MirArgument, MirArrayAssignElement, MirArrayCopyElement,
        MirArrayDefaultElement, MirArrayDestroyElement, MirArrayInstruction, MirCallReceiver,
        MirCallTarget, MirClassOptionalSource, MirCopyCapability, MirDefinitionRef,
        MirDestructionStep, MirInstruction, MirIoOperation, MirMethodCallTarget, MirObjectOrigin,
        MirObjectView, MirOptionalSharedSource, MirOptionalSource, MirPlace, MirPlaceBase,
        MirPlaceProjection, MirProgram, MirRvalue, MirRvalueKind, MirSelectedCopyOperation,
        MirSharedCastSource, MirSharedTarget, MirStaticInitializerBody, MirSynthesizedCopy,
        MirSynthesizedFieldCopy, MirTerminator, MirType, PreliminaryMirProgram,
        PreliminaryMirSharedLifecycleTarget,
    },
    source::Span,
};

use super::model::{
    edge_key, evidence_key, StaticAccessEvidence, StaticAccessKind, StaticArrayLifecycleOperation,
    StaticClassLifecycleOperation, StaticEffectEdge, StaticEffectEdgeKind, StaticEffectNode,
    StaticEffectPhase, StaticFunctionValueCandidates,
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
    extract_parts(program.program(), program.static_initializer_bodies())
}

pub(crate) fn extract_final(
    program: &MirProgram,
    initializers: &[MirStaticInitializerBody],
) -> ExtractedGraph {
    extract_parts(program, initializers)
}

fn extract_parts(
    program: &MirProgram,
    initializers: &[MirStaticInitializerBody],
) -> ExtractedGraph {
    let function_value_candidates = function_values::collect(program, initializers);
    let mut extractor = Extractor {
        program,
        initializers,
        function_value_candidates,
        nodes: BTreeMap::new(),
    };
    extractor.seed_nodes();
    extractor.extract_implicit_lifecycle();
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
    fn function_value_targets(
        &self,
        function_type: crate::identity::FunctionTypeId,
    ) -> Vec<CallableId> {
        self.function_value_candidates
            .binary_search_by_key(&function_type, |candidates| candidates.function_type)
            .ok()
            .into_iter()
            .flat_map(|index| &self.function_value_candidates[index].targets)
            .map(|target| target.callable)
            .collect()
    }

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

    fn seed_nodes(&mut self) {
        for definition in self
            .program
            .definitions
            .iter()
            .map(MirDefinitionRef::Function)
            .chain(
                self.program
                    .member_definitions
                    .iter()
                    .map(MirDefinitionRef::Member),
            )
            .chain(self.initializers.iter().map(MirDefinitionRef::from))
        {
            self.nodes
                .entry(StaticEffectNode::Callable(definition.callable()))
                .or_default();
        }
        for class in self.program.classes.iter() {
            for operation in [
                StaticClassLifecycleOperation::CopyConstructor,
                StaticClassLifecycleOperation::CopyAssignment,
                StaticClassLifecycleOperation::CompleteFinalizer,
            ] {
                self.nodes
                    .entry(StaticEffectNode::class(class.id, operation))
                    .or_default();
            }
        }
        for array in self.program.array_types.iter() {
            for operation in [
                StaticArrayLifecycleOperation::Default,
                StaticArrayLifecycleOperation::Copy,
                StaticArrayLifecycleOperation::Assignment,
                StaticArrayLifecycleOperation::Destruction,
            ] {
                self.nodes
                    .entry(StaticEffectNode::array(array.id, operation))
                    .or_default();
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
