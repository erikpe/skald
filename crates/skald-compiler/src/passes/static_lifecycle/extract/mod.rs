//! Exhaustive MIR and implicit-lifecycle graph extraction.

mod access;
mod control;
mod edges;
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
        MirPlaceProjection, MirRvalue, MirRvalueKind, MirSelectedCopyOperation,
        MirSharedCastSource, MirSharedTarget, MirSynthesizedCopy, MirSynthesizedFieldCopy,
        MirTerminator, MirType, PreliminaryMirProgram, PreliminaryMirSharedLifecycleTarget,
    },
    source::Span,
};

use super::model::{
    edge_key, evidence_key, StaticAccessEvidence, StaticAccessKind, StaticArrayLifecycleOperation,
    StaticClassLifecycleOperation, StaticEffectEdge, StaticEffectEdgeKind, StaticEffectNode,
    StaticEffectPhase,
};

#[derive(Default)]
pub(crate) struct NodeDraft {
    pub(crate) direct: Vec<StaticAccessEvidence>,
    pub(crate) edges: Vec<StaticEffectEdge>,
}

pub(crate) struct ExtractedGraph {
    pub(crate) nodes: BTreeMap<StaticEffectNode, NodeDraft>,
}

pub(crate) fn extract(program: &PreliminaryMirProgram) -> ExtractedGraph {
    let mut extractor = Extractor {
        program,
        nodes: BTreeMap::new(),
    };
    extractor.seed_nodes();
    extractor.extract_implicit_lifecycle();
    extractor.extract_bodies();
    extractor.finish()
}

struct Extractor<'mir> {
    program: &'mir PreliminaryMirProgram,
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
        ExtractedGraph { nodes: self.nodes }
    }

    fn seed_nodes(&mut self) {
        for definition in self.program.executable_definitions() {
            self.nodes
                .entry(StaticEffectNode::Callable(definition.callable()))
                .or_default();
        }
        for class in self.program.program().classes.iter() {
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
        for array in self.program.program().array_types.iter() {
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
