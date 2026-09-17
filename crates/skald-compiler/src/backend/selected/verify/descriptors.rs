//! Validate borrowed descriptions independently of checked append constructors.
use super::SelectedReason as Reason;
use crate::backend::selected::{
    AbiArea, AbiBinding, AbiBindings, AbiLocation, Bundle, Constraint, Description, Flow,
    OperandRole, Payload, SelectedDraft, Timing,
};
use crate::backend::{
    effects::{Effect, MemoryRegion},
    plan::{ReturnShape, SignatureId},
    RuntimeTracePolicy,
};
use std::collections::BTreeSet;

fn same_location(constraint: Constraint<'_>, location: AbiLocation) -> bool {
    match (constraint, location) {
        (Constraint::Fixed(a), AbiLocation::Fixed(b)) => a == b,
        (Constraint::AbiSlot { area: a, index: i }, AbiLocation::Slot { area: b, index: j }) => {
            a == b && i == j
        }
        _ => false,
    }
}
fn tie_compatible(a: Constraint<'_>, b: Constraint<'_>) -> bool {
    match (a, b) {
        (Constraint::Fixed(a), Constraint::Fixed(b)) => a == b,
        (Constraint::Fixed(a), Constraint::Resources { views, .. })
        | (Constraint::Resources { views, .. }, Constraint::Fixed(a)) => views.contains(&a),
        (
            Constraint::Resources {
                views: a,
                memory: x,
            },
            Constraint::Resources {
                views: b,
                memory: y,
            },
        ) => (x && y) || a.iter().any(|v| b.contains(v)),
        (Constraint::AbiSlot { area: a, index: i }, Constraint::AbiSlot { area: b, index: j }) => {
            a == b && i == j
        }
        _ => false,
    }
}
fn bindings<P>(
    draft: &SelectedDraft<'_, P>,
    desc: &Description<'_>,
    signature: SignatureId,
) -> bool {
    draft
        .context
        .abi_bindings(
            signature,
            desc.abi_inputs.to_vec(),
            desc.abi_results.to_vec(),
        )
        .is_ok()
}
fn operands_bind(
    desc: &Description<'_>,
    role: OperandRole,
    bindings: &[AbiBinding],
    indirect: Option<usize>,
) -> bool {
    let ops: Vec<_> = desc
        .operands
        .iter()
        .enumerate()
        .filter(|(slot, op)| op.role == role && Some(*slot) != indirect)
        .map(|(_, op)| op)
        .collect();
    ops.len() == bindings.len()
        && ops.iter().zip(bindings).all(|(op, b)| {
            op.representation == b.representation && same_location(op.constraint, b.location)
        })
}
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn check<P: Payload>(
    draft: &SelectedDraft<'_, P>,
    payload: &P,
    terminal: Option<usize>,
    references: &mut BTreeSet<crate::backend::plan::ArtifactId>,
) -> Vec<Reason> {
    let desc = payload.describe();
    let ctx = draft.context;
    let mut errors = vec![];
    let flow_ok = match (terminal, desc.flow) {
        (None, Flow::Instruction) => desc.successors == 0,
        (Some(n), Flow::Branch) => n != 0 && desc.successors == n,
        (Some(0), Flow::Return | Flow::Never) => desc.successors == 0,
        _ => false,
    };
    if !flow_ok {
        errors.push(Reason::Flow);
    }
    for op in &*desc.operands {
        if draft.values.get_id(op.value).is_err()
            || draft
                .values
                .get_id(op.value)
                .is_ok_and(|v| v.ty != op.representation)
            || ctx.representation(op.representation).is_err()
        {
            errors.push(Reason::Representation);
        }
        let valid = match op.constraint {
            Constraint::Fixed(view) => ctx
                .resources
                .require_view(
                    view,
                    op.representation.bits(),
                    op.representation.bank(),
                    false,
                )
                .is_ok(),
            Constraint::Resources { views, memory } => {
                (!views.is_empty() || memory)
                    && views.iter().collect::<BTreeSet<_>>().len() == views.len()
                    && views.iter().all(|view| {
                        ctx.resources
                            .require_view(
                                *view,
                                op.representation.bits(),
                                op.representation.bank(),
                                true,
                            )
                            .is_ok()
                    })
            }
            Constraint::AbiSlot { area, index } => ctx
                .abi_areas
                .require_slot(area, index, op.representation)
                .is_ok(),
        };
        if !valid {
            errors.push(Reason::Resource);
        }
        if terminal.is_some() && op.role == OperandRole::Definition {
            errors.push(Reason::Flow);
        }
    }
    let mut inputs = BTreeSet::new();
    let mut outputs = BTreeSet::new();
    for tie in desc.ties {
        match (desc.operands.get(tie.input), desc.operands.get(tie.output)) {
            (Some(a), Some(b))
                if a.role == OperandRole::Use
                    && b.role == OperandRole::Definition
                    && a.value != b.value
                    && a.representation == b.representation
                    && tie_compatible(a.constraint, b.constraint)
                    && inputs.insert(tie.input)
                    && outputs.insert(tie.output) =>
            {
                if a.timing == Timing::Late && b.timing == Timing::Early {
                    errors.push(Reason::Timing);
                }
            }
            _ => errors.push(Reason::Tie),
        }
    }
    let mut clobbers = BTreeSet::new();
    for (timing, unit) in desc.clobbers {
        if ctx.resources.require_unit(*unit).is_err()
            || !clobbers.insert((if *timing == Timing::Early { 0 } else { 1 }, *unit))
        {
            errors.push(Reason::Resource);
        }
    }
    for object in desc.objects {
        if draft.objects.get_id(*object).is_err() {
            errors.push(Reason::Reference);
        }
    }
    for (artifact, category) in desc.artifacts {
        if ctx.extension.artifact(*artifact, *category).is_err() {
            errors.push(Reason::Reference);
        } else {
            references.insert(*artifact);
        }
    }
    for effect in desc.effects.iter() {
        match effect {
            Effect::Read(MemoryRegion::Object(object))
            | Effect::Write(MemoryRegion::Object(object))
                if !desc.objects.contains(object) =>
            {
                errors.push(Reason::Effect)
            }
            Effect::TraceState
                if ctx.extension.parent().parent().runtime_trace()
                    == RuntimeTracePolicy::Omitted
                    || !desc.artifacts.contains(&(
                        crate::backend::plan::ArtifactId::TraceTls,
                        crate::backend::plan::ArtifactCategory::Tls,
                    )) =>
            {
                errors.push(Reason::Effect)
            }
            Effect::Read(MemoryRegion::Static(field))
            | Effect::Write(MemoryRegion::Static(field)) => {
                let artifact = crate::backend::plan::ArtifactId::Data(
                    crate::backend::plan::DataKey::Static(*field),
                );
                if !ctx.extension.parent().parent().is_active_static(*field)
                    || ctx
                        .extension
                        .artifact(artifact, artifact.category())
                        .is_err()
                {
                    errors.push(Reason::Reference);
                } else {
                    references.insert(artifact);
                }
            }
            _ => {}
        }
    }
    if let Some(signature) = desc.call_signature {
        let view = ctx.extension.parent().parent();
        let attribution_ok = match desc.call_attribution {
            Some(crate::backend::lir::CallAttribution::SourceOperation { location, .. }) => {
                location.is_none_or(|location| {
                    matches!(
                        location,
                        crate::backend::plan::ArtifactId::Data(
                            crate::backend::plan::DataKey::TraceLocation(_)
                        )
                    ) && view.runtime_trace() == RuntimeTracePolicy::Enabled
                        && desc.artifacts.contains(&(location, location.category()))
                })
            }
            Some(crate::backend::lir::CallAttribution::InheritedOperation { boundary }) => {
                view.callable(*boundary).is_ok()
            }
            Some(crate::backend::lir::CallAttribution::SourceBodyFromOmittedHelper {
                boundary,
            }) => {
                matches!(boundary, crate::backend::plan::LirCallableId::Helper(_))
                    && view.callable(*boundary).is_ok()
                    && desc.artifacts.iter().any(|(artifact, _)| {
                        matches!(
                            artifact,
                            crate::backend::plan::ArtifactId::Callable(
                                crate::backend::plan::LirCallableId::Source(_)
                            )
                        )
                    })
            }
            Some(
                crate::backend::lir::CallAttribution::NonReporting
                | crate::backend::lir::CallAttribution::HardDefectOnly,
            ) => !desc.effects.contains(Effect::Report),
            Some(crate::backend::lir::CallAttribution::ProcessBoundary) => true,
            None => false,
        };
        if let Some(
            crate::backend::lir::CallAttribution::InheritedOperation { boundary }
            | crate::backend::lir::CallAttribution::SourceBodyFromOmittedHelper { boundary },
        ) = desc.call_attribution
        {
            references.insert(crate::backend::plan::ArtifactId::Callable(*boundary));
        }
        if !attribution_ok {
            errors.push(Reason::Abi);
        }
        if !desc.effects.contains(Effect::Call)
            || !bindings(draft, &desc, signature)
            || !operands_bind(
                &desc,
                OperandRole::Use,
                desc.abi_inputs,
                desc.indirect_target,
            )
            || !operands_bind(&desc, OperandRole::Definition, desc.abi_results, None)
        {
            errors.push(Reason::Abi);
        }
        if desc.abi_inputs.iter().any(
            |b| matches!(b.location, AbiLocation::Slot { area, .. } if area != AbiArea::Outgoing),
        ) {
            errors.push(Reason::Abi);
        }
        if let Ok(handle) = ctx
            .extension
            .parent()
            .parent()
            .signature_id(signature.index())
        {
            if let Ok(sig) = ctx.extension.parent().parent().signature(handle) {
                if (sig.returns == ReturnShape::Never) != (desc.flow == Flow::Never) {
                    errors.push(Reason::Flow);
                }
            }
        }
        if let Some(slot) = desc.indirect_target {
            if !desc.operands.get(slot).is_some_and(|op| {
                op.role == OperandRole::Use
                    && op.representation.kind
                        == crate::backend::selected::RepresentationKind::CodeAddress(signature)
                    && op.timing == Timing::Early
            }) {
                errors.push(Reason::Abi);
            }
        }
    } else {
        if desc.call_attribution.is_some()
            || desc.indirect_target.is_some()
            || desc.effects.contains(Effect::Call)
            || !desc.abi_inputs.is_empty()
        {
            errors.push(Reason::Abi);
        }
        if desc.flow == Flow::Return {
            let valid = draft.owner.signature().is_ok_and(|sig| {
                sig.returns != ReturnShape::Never
                    && AbiBindings::new(
                        sig,
                        (ctx.extension
                            .parent()
                            .parent()
                            .profile()
                            .data_layout
                            .pointer_bytes
                            * 8) as u16,
                        &ctx.resources,
                        draft.abi().map_or(vec![], |a| a.inputs().to_vec()),
                        desc.abi_results.to_vec(),
                    )
                    .is_ok()
            });
            if !valid || !operands_bind(&desc, OperandRole::Use, desc.abi_results, None) {
                errors.push(Reason::Abi);
            }
        } else if !desc.abi_results.is_empty() {
            errors.push(Reason::Abi);
        }
    }
    if desc
        .abi_results
        .iter()
        .any(|b| matches!(b.location, AbiLocation::Slot { area, .. } if area != AbiArea::Results))
    {
        errors.push(Reason::Abi);
    }
    for binding in desc.abi_inputs.iter().chain(desc.abi_results) {
        if ctx.require_abi_binding(binding).is_err() {
            errors.push(Reason::Abi);
        }
    }
    if let Some(Bundle::Bounded { scratch, .. }) = &desc.bundle {
        if terminal.is_some()
            || desc
                .effects
                .iter()
                .any(|e| matches!(e, Effect::Call | Effect::Report | Effect::HardTrap))
        {
            errors.push(Reason::Bundle);
        }
        for scratch in &**scratch {
            if ctx.representation(scratch.representation).is_err()
                || usize::from(scratch.count.get()) > scratch.views.len()
                || scratch.views.iter().collect::<BTreeSet<_>>().len() != scratch.views.len()
                || scratch.views.iter().any(|view| {
                    ctx.resources
                        .require_view(
                            *view,
                            scratch.representation.bits(),
                            scratch.representation.bank(),
                            true,
                        )
                        .is_err()
                })
            {
                errors.push(Reason::Bundle);
            }
        }
    }
    errors
}
