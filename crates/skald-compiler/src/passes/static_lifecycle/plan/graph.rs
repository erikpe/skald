//! Static-lifetime graph construction and deterministic ordering.

use std::{cmp::Ordering, collections::BTreeMap};

use crate::{
    identity::StaticFieldId,
    mir::{MirSharedTarget, MirType, PreliminaryMirProgram, PreliminaryMirSharedLifecycleTarget},
    passes::graph::strongly_connected_components,
};

use super::{
    super::{
        model::{edge_key, span_key},
        StaticAccessEvidence, StaticAccessKind, StaticArrayLifecycleOperation,
        StaticClassLifecycleOperation, StaticEffectAnalysis, StaticEffectEdge, StaticEffectNode,
        StaticEffectPhase,
    },
    model::{
        StaticLifecyclePlan, StaticLifetimeDependency, StaticLifetimeEvidence, StaticLifetimePhase,
    },
};

pub(crate) struct LifetimeGraph {
    fields: Vec<StaticFieldId>,
    dependencies: Vec<StaticLifetimeDependency>,
    adjacency: Vec<Vec<usize>>,
}

#[derive(Clone, Copy)]
struct LifetimeRoot {
    field: StaticFieldId,
    span: crate::source::Span,
    phase: StaticLifetimePhase,
    effect: StaticEffectNode,
}

impl LifetimeGraph {
    pub(crate) fn build(program: &PreliminaryMirProgram, effects: &StaticEffectAnalysis) -> Self {
        let fields = program
            .static_fields()
            .map(|field| field.field)
            .collect::<Vec<_>>();
        let field_indices = fields
            .iter()
            .copied()
            .enumerate()
            .map(|(index, field)| (field, index))
            .collect::<BTreeMap<_, _>>();
        let mut by_edge = BTreeMap::<(usize, usize), StaticLifetimeDependency>::new();

        for field in program.static_fields() {
            if let Some(initializer) = field.initializer {
                let root = StaticEffectNode::callable(initializer.into());
                let summary = effects
                    .summary(root)
                    .expect("verified static initializer must have an effect summary");
                for effect in &summary.effects {
                    if is_lifecycle_destination_or_published_self(field.field, effect) {
                        continue;
                    }
                    insert_dependency(
                        program,
                        &field_indices,
                        &mut by_edge,
                        LifetimeRoot {
                            field: field.field,
                            span: field.span,
                            phase: StaticLifetimePhase::Initialization,
                            effect: root,
                        },
                        effect,
                    );
                }
            }

            for root in destruction_roots(program, field.ty) {
                let summary = effects
                    .summary(root)
                    .expect("verified destruction root must have an effect summary");
                for effect in &summary.effects {
                    insert_dependency(
                        program,
                        &field_indices,
                        &mut by_edge,
                        LifetimeRoot {
                            field: field.field,
                            span: field.span,
                            phase: StaticLifetimePhase::Destruction,
                            effect: root,
                        },
                        effect,
                    );
                }
            }
        }

        let dependencies = by_edge.into_values().collect::<Vec<_>>();
        let mut adjacency = vec![Vec::new(); fields.len()];
        for dependency in &dependencies {
            adjacency[field_indices[&dependency.prerequisite]]
                .push(field_indices[&dependency.dependent]);
        }
        for targets in &mut adjacency {
            targets.sort_unstable();
            targets.dedup();
        }

        Self {
            fields,
            dependencies,
            adjacency,
        }
    }

    pub(crate) fn dependencies(&self) -> &[StaticLifetimeDependency] {
        &self.dependencies
    }

    pub(crate) fn cyclic_components(&self) -> Vec<Vec<usize>> {
        let components = strongly_connected_components(&self.adjacency);
        let mut grouped = BTreeMap::<usize, Vec<usize>>::new();
        for (node, component) in components.into_iter().enumerate() {
            grouped.entry(component).or_default().push(node);
        }
        let mut cyclic = grouped
            .into_values()
            .filter(|nodes| {
                nodes.len() > 1
                    || nodes
                        .first()
                        .is_some_and(|node| self.adjacency[*node].contains(node))
            })
            .collect::<Vec<_>>();
        cyclic.sort_by_key(|nodes| nodes[0]);
        cyclic
    }

    pub(crate) fn representative_cycle(&self, component: &[usize]) -> Vec<usize> {
        let start = component[0];
        if self.adjacency[start].contains(&start) {
            return vec![start, start];
        }

        let in_component = component
            .iter()
            .copied()
            .map(|node| (node, ()))
            .collect::<BTreeMap<_, _>>();
        let mut queue = std::collections::VecDeque::from([(start, vec![start])]);
        let mut visited = vec![false; self.fields.len()];
        visited[start] = true;
        while let Some((node, path)) = queue.pop_front() {
            for target in &self.adjacency[node] {
                if !in_component.contains_key(target) {
                    continue;
                }
                if *target == start {
                    let mut cycle = path;
                    cycle.push(start);
                    return cycle;
                }
                if !std::mem::replace(&mut visited[*target], true) {
                    let mut next = path.clone();
                    next.push(*target);
                    queue.push_back((*target, next));
                }
            }
        }
        unreachable!("a nontrivial strongly connected component must contain a cycle")
    }

    pub(crate) fn dependency(
        &self,
        prerequisite: usize,
        dependent: usize,
    ) -> &StaticLifetimeDependency {
        let prerequisite = self.fields[prerequisite];
        let dependent = self.fields[dependent];
        self.dependencies
            .iter()
            .find(|edge| edge.prerequisite == prerequisite && edge.dependent == dependent)
            .expect("cycle edge must retain dependency evidence")
    }

    pub(crate) fn plan(&self) -> StaticLifecyclePlan {
        let mut indegrees = vec![0usize; self.fields.len()];
        for targets in &self.adjacency {
            for target in targets {
                indegrees[*target] += 1;
            }
        }
        let mut ready = indegrees
            .iter()
            .enumerate()
            .filter_map(|(index, indegree)| (*indegree == 0).then_some(index))
            .collect::<std::collections::BTreeSet<_>>();
        let mut activation = Vec::with_capacity(self.fields.len());
        while let Some(node) = ready.pop_first() {
            activation.push(self.fields[node]);
            for target in &self.adjacency[node] {
                indegrees[*target] -= 1;
                if indegrees[*target] == 0 {
                    ready.insert(*target);
                }
            }
        }
        debug_assert_eq!(
            activation.len(),
            self.fields.len(),
            "planned graph is acyclic"
        );
        StaticLifecyclePlan::new(activation)
    }
}

fn is_lifecycle_destination_or_published_self(
    root: StaticFieldId,
    effect: &StaticAccessEvidence,
) -> bool {
    effect.field == root
        && (effect.phase == StaticEffectPhase::InitializerAfterPublication
            || (effect.phase == StaticEffectPhase::InitializerBeforePublication
                && effect.access == StaticAccessKind::Initialize))
}

fn insert_dependency(
    program: &PreliminaryMirProgram,
    field_indices: &BTreeMap<StaticFieldId, usize>,
    by_edge: &mut BTreeMap<(usize, usize), StaticLifetimeDependency>,
    root: LifetimeRoot,
    effect: &StaticAccessEvidence,
) {
    let prerequisite = field_indices[&effect.field];
    let dependent = field_indices[&root.field];
    let target_span = program
        .program()
        .static_field(effect.field)
        .expect("effect target must be a declared static field")
        .span;
    let candidate = StaticLifetimeDependency {
        prerequisite: effect.field,
        dependent: root.field,
        evidence: StaticLifetimeEvidence {
            root: root.field,
            root_span: root.span,
            phase: root.phase,
            root_effect: root.effect,
            target: effect.field,
            target_span,
            access: effect.access,
            effect_phase: effect.phase,
            access_span: effect.span,
            witness: effect.witness.clone(),
        },
    };
    match by_edge.entry((prerequisite, dependent)) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(candidate);
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            if compare_evidence(&candidate.evidence, &entry.get().evidence).is_lt() {
                entry.insert(candidate);
            }
        }
    }
}

fn destruction_roots(program: &PreliminaryMirProgram, ty: MirType) -> Vec<StaticEffectNode> {
    match ty {
        MirType::Class(class) | MirType::OptionalClass(class) => vec![StaticEffectNode::class(
            class,
            StaticClassLifecycleOperation::CompleteFinalizer,
        )],
        MirType::Shared(target) | MirType::OptionalShared(target) => {
            shared_destruction_roots(program, target)
        }
        MirType::Array(array) => vec![StaticEffectNode::array(
            array,
            StaticArrayLifecycleOperation::Destruction,
        )],
        MirType::I64
        | MirType::U64
        | MirType::U8
        | MirType::F64
        | MirType::Bool
        | MirType::OptionalPrimitive(_) => Vec::new(),
        MirType::Interface(_) | MirType::Obj | MirType::Unit => {
            unreachable!("verified static fields always have a storable type")
        }
    }
}

fn shared_destruction_roots(
    program: &PreliminaryMirProgram,
    target: MirSharedTarget,
) -> Vec<StaticEffectNode> {
    program
        .shared_lifecycle_targets(target)
        .into_iter()
        .map(|target| match target {
            PreliminaryMirSharedLifecycleTarget::Class(class) => {
                StaticEffectNode::class(class, StaticClassLifecycleOperation::CompleteFinalizer)
            }
            PreliminaryMirSharedLifecycleTarget::Array(array) => {
                StaticEffectNode::array(array, StaticArrayLifecycleOperation::Destruction)
            }
        })
        .collect()
}

fn compare_evidence(left: &StaticLifetimeEvidence, right: &StaticLifetimeEvidence) -> Ordering {
    left.phase
        .cmp(&right.phase)
        .then_with(|| left.witness.len().cmp(&right.witness.len()))
        .then_with(|| compare_witnesses(&left.witness, &right.witness))
        .then_with(|| left.root_effect.cmp(&right.root_effect))
        .then_with(|| left.access.cmp(&right.access))
        .then_with(|| left.effect_phase.cmp(&right.effect_phase))
        .then_with(|| span_key(left.access_span).cmp(&span_key(right.access_span)))
}

fn compare_witnesses(left: &[StaticEffectEdge], right: &[StaticEffectEdge]) -> Ordering {
    left.iter()
        .map(|edge| (edge_key(edge), edge.phase))
        .cmp(right.iter().map(|edge| (edge_key(edge), edge.phase)))
}
