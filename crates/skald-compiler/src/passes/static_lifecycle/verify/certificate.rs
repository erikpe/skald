//! Soundness verification for static-effect and lifetime certificates.

use std::collections::{BTreeMap, BTreeSet};

use crate::mir::{
    MirStaticFieldInitialization, MirVerificationError, StaticAccessEvidence, StaticEffectNode,
    StaticEffectSummary, StaticLifetimeDependency, StaticLifetimePhase,
};

use super::{
    super::{
        extract,
        roots::{destruction_roots, is_lifecycle_destination_or_published_self},
    },
    program_error, LifecycleMirView,
};

pub(super) fn verify(program: LifecycleMirView<'_>, errors: &mut Vec<MirVerificationError>) {
    let extracted = extract::extract_final(program.program, program.initializers);
    let analysis = program.lifecycle.certificate().effects();
    let mut summaries = BTreeMap::new();
    for summary in analysis.summaries() {
        if summaries.insert(summary.node, summary).is_some() {
            program_error(
                errors,
                format!("duplicate static-effect summary for {:?}", summary.node),
            );
        }
    }
    if summaries.len() != extracted.nodes.len() {
        program_error(
            errors,
            "static-effect summaries do not cover every MIR effect node",
        );
    }

    for (node, draft) in &extracted.nodes {
        let Some(summary) = summaries.get(node).copied() else {
            program_error(
                errors,
                format!("missing static-effect summary for {node:?}"),
            );
            continue;
        };
        if summary.direct_effects != draft.direct {
            program_error(
                errors,
                format!("direct effects for {node:?} do not match MIR"),
            );
        }
        if summary.possible_targets != draft.edges {
            program_error(
                errors,
                format!("possible call targets for {node:?} do not match MIR"),
            );
        }
        verify_summary(program, summary, &summaries, errors);
    }
    for node in summaries.keys() {
        if !extracted.nodes.contains_key(node) {
            program_error(
                errors,
                format!("static-effect summary names foreign node {node:?}"),
            );
        }
    }

    verify_dependencies(program, &summaries, errors);
}

fn verify_summary(
    program: LifecycleMirView<'_>,
    summary: &StaticEffectSummary,
    summaries: &BTreeMap<StaticEffectNode, &StaticEffectSummary>,
    errors: &mut Vec<MirVerificationError>,
) {
    for direct in &summary.direct_effects {
        if direct.lifecycle_owned
            && (!matches!(summary.node, StaticEffectNode::Callable(crate::identity::CallableId::StaticInitializer(initializer)) if initializer.field() == direct.field)
                || direct.phase != crate::mir::StaticEffectPhase::InitializerBeforePublication
                || direct.access != crate::mir::StaticAccessKind::Initialize)
        {
            program_error(
                errors,
                format!(
                    "summary for {:?} contains an invalid lifecycle-owned destination access",
                    summary.node
                ),
            );
        }
        if !summary
            .effects
            .iter()
            .any(|effect| same_fact(effect, direct))
        {
            program_error(
                errors,
                format!(
                    "summary for {:?} omits one direct static effect",
                    summary.node
                ),
            );
        }
    }
    for edge in &summary.possible_targets {
        if edge.source != summary.node {
            program_error(
                errors,
                format!(
                    "possible target edge has the wrong source for {:?}",
                    summary.node
                ),
            );
            continue;
        }
        let Some(target) = summaries.get(&edge.target).copied() else {
            program_error(
                errors,
                format!("possible target edge names foreign node {:?}", edge.target),
            );
            continue;
        };
        for effect in &target.effects {
            if !summary.effects.iter().any(|candidate| {
                candidate.field == effect.field
                    && candidate.access == effect.access
                    && candidate.phase == edge.phase
                    && candidate.lifecycle_owned == effect.lifecycle_owned
            }) {
                program_error(
                    errors,
                    format!(
                        "summary for {:?} is not closed over target {:?}",
                        summary.node, edge.target
                    ),
                );
            }
        }
    }
    for effect in &summary.effects {
        if program.program.static_field(effect.field).is_none() {
            program_error(
                errors,
                format!(
                    "summary for {:?} names foreign static field {}",
                    summary.node, effect.field
                ),
            );
        }
        if !witness_is_valid(summary.node, effect, summaries) {
            program_error(
                errors,
                format!(
                    "summary for {:?} contains invalid effect evidence",
                    summary.node
                ),
            );
        }
    }
}

fn witness_is_valid(
    root: StaticEffectNode,
    effect: &StaticAccessEvidence,
    summaries: &BTreeMap<StaticEffectNode, &StaticEffectSummary>,
) -> bool {
    let mut node = root;
    let mut root_phase = None;
    for edge in &effect.witness {
        let Some(summary) = summaries.get(&node) else {
            return false;
        };
        if edge.source != node || !summary.possible_targets.contains(edge) {
            return false;
        }
        root_phase.get_or_insert(edge.phase);
        node = edge.target;
    }
    let Some(summary) = summaries.get(&node) else {
        return false;
    };
    summary.direct_effects.iter().any(|direct| {
        direct.field == effect.field
            && direct.access == effect.access
            && direct.lifecycle_owned == effect.lifecycle_owned
            && direct.span == effect.span
            && effect.phase == root_phase.unwrap_or(direct.phase)
    })
}

fn verify_dependencies(
    program: LifecycleMirView<'_>,
    summaries: &BTreeMap<StaticEffectNode, &StaticEffectSummary>,
    errors: &mut Vec<MirVerificationError>,
) {
    let fields = program
        .lifecycle
        .definitions()
        .iter()
        .map(|field| (field.field, CertificateField::from(*field)))
        .collect::<BTreeMap<_, _>>();
    let positions = program
        .lifecycle
        .plan()
        .activation()
        .iter()
        .copied()
        .enumerate()
        .map(|(index, field)| (field, index))
        .collect::<BTreeMap<_, _>>();
    let mut pairs = BTreeSet::new();
    for dependency in program.lifecycle.certificate().dependencies() {
        if !pairs.insert((dependency.prerequisite, dependency.dependent)) {
            program_error(
                errors,
                "static lifetime certificate contains a duplicate edge",
            );
        }
        verify_dependency(program, dependency, &fields, &positions, summaries, errors);
    }

    for field in fields.values() {
        if let Some(initializer) = field.initializer {
            let root = StaticEffectNode::callable(initializer.into());
            if let Some(summary) = summaries.get(&root) {
                for effect in &summary.effects {
                    if !is_lifecycle_destination_or_published_self(field.field, effect)
                        && !pairs.contains(&(effect.field, field.field))
                    {
                        program_error(
                            errors,
                            format!(
                                "lifetime certificate omits initialization edge {} -> {}",
                                effect.field, field.field
                            ),
                        );
                    }
                }
            }
        }
        for root in destruction_roots(program.program, field.ty) {
            if let Some(summary) = summaries.get(&root) {
                for effect in &summary.effects {
                    if !pairs.contains(&(effect.field, field.field)) {
                        program_error(
                            errors,
                            format!(
                                "lifetime certificate omits destruction edge {} -> {}",
                                effect.field, field.field
                            ),
                        );
                    }
                }
            }
        }
    }
}

fn verify_dependency(
    program: LifecycleMirView<'_>,
    dependency: &StaticLifetimeDependency,
    fields: &BTreeMap<crate::identity::StaticFieldId, CertificateField>,
    positions: &BTreeMap<crate::identity::StaticFieldId, usize>,
    summaries: &BTreeMap<StaticEffectNode, &StaticEffectSummary>,
    errors: &mut Vec<MirVerificationError>,
) {
    let (Some(prerequisite), Some(dependent)) = (
        fields.get(&dependency.prerequisite),
        fields.get(&dependency.dependent),
    ) else {
        program_error(errors, "static lifetime edge names a foreign field");
        return;
    };
    if positions.get(&dependency.prerequisite) >= positions.get(&dependency.dependent) {
        program_error(
            errors,
            format!(
                "static lifetime edge {} -> {} violates activation order",
                dependency.prerequisite, dependency.dependent
            ),
        );
    }
    let evidence = &dependency.evidence;
    let expected_target_span = program
        .program
        .static_field(prerequisite.field)
        .map(|field| field.span);
    if evidence.root != dependent.field
        || evidence.root_span != dependent.span
        || evidence.target != prerequisite.field
        || Some(evidence.target_span) != expected_target_span
    {
        program_error(
            errors,
            "static lifetime edge has inconsistent field evidence",
        );
    }
    let legitimate_root = match evidence.phase {
        StaticLifetimePhase::Initialization => dependent.initializer.is_some_and(|initializer| {
            evidence.root_effect == StaticEffectNode::callable(initializer.into())
        }),
        StaticLifetimePhase::Destruction => {
            destruction_roots(program.program, dependent.ty).contains(&evidence.root_effect)
        }
    };
    if !legitimate_root {
        program_error(errors, "static lifetime edge has an invalid lifecycle root");
        return;
    }
    let Some(summary) = summaries.get(&evidence.root_effect) else {
        program_error(errors, "static lifetime edge root has no effect summary");
        return;
    };
    if !summary.effects.iter().any(|effect| {
        effect.field == evidence.target
            && effect.access == evidence.access
            && effect.phase == evidence.effect_phase
            && effect.span == evidence.access_span
            && effect.witness == evidence.witness
    }) {
        program_error(
            errors,
            "static lifetime edge is not justified by its root summary",
        );
    }
}

#[derive(Clone, Copy)]
struct CertificateField {
    field: crate::identity::StaticFieldId,
    ty: crate::mir::MirType,
    initializer: Option<crate::identity::StaticInitializerId>,
    span: crate::source::Span,
}

impl From<crate::mir::MirStaticLifecycleDefinition> for CertificateField {
    fn from(definition: crate::mir::MirStaticLifecycleDefinition) -> Self {
        Self {
            field: definition.field,
            ty: definition.ty,
            initializer: match definition.initialization {
                MirStaticFieldInitialization::ZeroDefault => None,
                MirStaticFieldInitialization::Explicit(initializer) => Some(initializer),
            },
            span: definition.span,
        }
    }
}

fn same_fact(left: &StaticAccessEvidence, right: &StaticAccessEvidence) -> bool {
    left.field == right.field
        && left.access == right.access
        && left.phase == right.phase
        && left.lifecycle_owned == right.lifecycle_owned
}
