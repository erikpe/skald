use super::{descriptors, publication, SelectedFailure, SelectedReason, VerifiedSelectedCallable};
use crate::backend::selected::{
    storage::ObjectRole, AbiArea, AbiBindings, AbiLocation, Payload, SelectedDraft,
};
use crate::backend::{
    graph::{check_graph, GraphLocation, GraphView},
    plan::{LayoutDisposition, LirCallableId, TargetProfile},
    RuntimeTracePolicy,
};
use std::collections::BTreeSet;

/// Implemented by concrete target owners; shared checks cannot certify opcode completeness.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) trait TargetVerifier<P: Payload> {
    fn profile(&self) -> TargetProfile;
    fn verify_payload(&self, payload: &P, terminal: bool) -> Result<(), &'static str>;
    fn verify_callable(&self, draft: &SelectedDraft<'_, P>) -> Result<(), &'static str>;
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn verify_selected<'p, P: Payload>(
    draft: SelectedDraft<'p, P>,
    target: &impl TargetVerifier<P>,
) -> Result<VerifiedSelectedCallable<'p, P>, Vec<SelectedFailure>> {
    let mut errors = vec![];
    let failure = |location, reason, origin| SelectedFailure {
        identity: Box::new(draft.identity()),
        location,
        reason,
        origin,
    };
    let context = draft.context;
    let owner = context
        .binding(draft.owner.key())
        .and_then(|binding| binding.require_same_owner(draft.owner).map_err(Into::into));
    if let Err(error) = owner {
        errors.push(failure(
            GraphLocation::Entry,
            SelectedReason::Inventory(error),
            None,
        ));
    }
    if target.profile() != draft.identity().target {
        errors.push(failure(
            GraphLocation::Entry,
            SelectedReason::Context(crate::backend::plan::PlanError::WrongTarget),
            None,
        ));
    }
    match &draft.input {
        Some(receipt) => {
            if let Err(error) = context.catalog.require_plan(receipt.owner().context()) {
                errors.push(failure(
                    GraphLocation::Entry,
                    SelectedReason::Inventory(error),
                    None,
                ));
            }
            if receipt.owner().key() != draft.owner.key() {
                errors.push(failure(
                    GraphLocation::Entry,
                    SelectedReason::Context(crate::backend::plan::PlanError::WrongOwner),
                    None,
                ));
            }
        }
        None if matches!(draft.owner.key(), LirCallableId::TargetThunk(_)) => {}
        None => errors.push(failure(GraphLocation::Entry, SelectedReason::Abi, None)),
    }
    for result in [
        draft.values.require_owner(draft.owner),
        draft.blocks.require_owner(draft.owner),
        draft.objects.require_owner(draft.owner),
    ] {
        if let Err(error) = result {
            errors.push(failure(
                GraphLocation::Entry,
                SelectedReason::Context(error),
                None,
            ));
        }
    }
    let signature = draft.owner.signature();
    match (signature, draft.abi()) {
        (Ok(signature), Some(abi)) => {
            let pointer_bits = (draft.identity().target.data_layout.pointer_bytes * 8) as u16;
            let checked = AbiBindings::new(
                signature,
                pointer_bits,
                &context.resources,
                abi.inputs().to_vec(),
                abi.results().to_vec(),
            );
            if checked.is_err() || draft.inputs.len() != abi.inputs().len() {
                errors.push(failure(GraphLocation::Entry, SelectedReason::Abi, None));
            }
            for (binding, area) in abi
                .inputs()
                .iter()
                .map(|b| (b, AbiArea::Incoming))
                .chain(abi.results().iter().map(|b| (b, AbiArea::Results)))
            {
                if context.require_abi_binding(binding).is_err()
                    || matches!(binding.location, AbiLocation::Slot { area: actual, .. } if actual != area)
                {
                    errors.push(failure(GraphLocation::Entry, SelectedReason::Abi, None));
                }
            }
            for (id, binding) in draft.inputs.iter().zip(abi.inputs()) {
                if !draft
                    .values
                    .get_id(*id)
                    .is_ok_and(|value| value.ty == binding.representation)
                {
                    errors.push(failure(GraphLocation::Entry, SelectedReason::Abi, None));
                }
            }
        }
        _ => errors.push(failure(GraphLocation::Entry, SelectedReason::Abi, None)),
    }
    for (id, value) in draft.values.iter() {
        if context.representation(value.ty).is_err() {
            errors.push(failure(
                GraphLocation::Value(id.index()),
                SelectedReason::Representation,
                value.origin,
            ));
        }
    }
    for (id, object) in draft.objects.iter() {
        let bad = object.layout.alignment == 0
            || !object.layout.alignment.is_power_of_two()
            || object.layout.disposition != LayoutDisposition::Addressable;
        let omitted = matches!(object.role, ObjectRole::Trace)
            && context.catalog.plan().runtime_trace() == RuntimeTracePolicy::Omitted;
        let area_bad = match object.role {
            ObjectRole::Abi(area) => {
                let slots = match area {
                    AbiArea::Incoming => &context.abi_areas.incoming,
                    AbiArea::Outgoing => &context.abi_areas.outgoing,
                    AbiArea::Results => &context.abi_areas.results,
                };
                slots
                    .iter()
                    .try_fold(0usize, |size, repr| {
                        size.checked_add(usize::from(repr.bits()).div_ceil(8))
                    })
                    .is_none_or(|size| size > object.layout.size)
            }
            _ => false,
        };
        if bad || omitted || area_bad {
            errors.push(failure(
                GraphLocation::Object(id.index()),
                SelectedReason::Reference,
                object.origin,
            ));
        }
    }
    if let Err(graph_errors) = check_graph(&draft) {
        errors.extend(graph_errors.into_iter().map(|error| {
            failure(
                error.location,
                SelectedReason::Graph(Box::new(error.clone())),
                error.origin,
            )
        }));
    }
    let mut references = BTreeSet::new();
    for (id, block) in draft.blocks.iter() {
        for (ordinal, payload) in block.instructions.iter().enumerate() {
            let location = GraphLocation::Instruction {
                block: id.index(),
                ordinal,
                operand: 0,
            };
            let before = errors.len();
            for reason in descriptors::check(&draft, payload, None, &mut references) {
                errors.push(failure(location, reason, block.origin));
            }
            if errors.len() == before {
                if let Err(reason) = target.verify_payload(payload, false) {
                    errors.push(failure(
                        location,
                        SelectedReason::Target(reason),
                        block.origin,
                    ));
                }
            }
        }
        if let Some(terminal) = &block.terminal {
            let location = GraphLocation::Terminator {
                block: id.index(),
                operand: 0,
            };
            let before = errors.len();
            for reason in descriptors::check(
                &draft,
                &terminal.payload,
                Some(terminal.edges.len()),
                &mut references,
            ) {
                errors.push(failure(location, reason, block.origin));
            }
            if errors.len() == before {
                if let Err(reason) = target.verify_payload(&terminal.payload, true) {
                    errors.push(failure(
                        location,
                        SelectedReason::Target(reason),
                        block.origin,
                    ));
                }
            }
        }
    }
    if errors.is_empty() {
        if let Err(reason) = target.verify_callable(&draft) {
            errors.push(failure(
                GraphLocation::Entry,
                SelectedReason::Target(reason),
                None,
            ));
        }
    }
    errors.sort_by_key(|error| error.location.sort_key());
    if errors.is_empty() {
        Ok(publication::publish(draft, references))
    } else {
        Err(errors)
    }
}
