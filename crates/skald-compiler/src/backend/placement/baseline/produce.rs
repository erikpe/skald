//! Produce an untrusted draft, then cross the same independent checking boundary
//! as a register allocator or a manually constructed placement.
use super::super::transfers::{copy, resolve, storage};
use super::super::{
    check::{check_placement, CheckedPlacement},
    model::*,
    requirements::*,
    target::PlacementTarget,
};
use super::operands;
use crate::backend::{
    graph::SelectedValueId,
    selected::{
        AbiArea, AbiLocation, BankKind, Flow, OperandRole, Payload, Representation,
        RepresentationKind, SelectedFact, VerifiedSelectedCallable,
    },
};
use std::collections::BTreeMap;

pub(in crate::backend) fn place_baseline<'s, 'p, P: Payload>(
    selected: &'s VerifiedSelectedCallable<'p, P>,
    target: &impl PlacementTarget,
) -> Result<CheckedPlacement<'s, 'p, P>, CheckFailure> {
    check_placement(produce_baseline(selected, target)?, target)
}

pub(in crate::backend) fn produce_baseline<'s, 'p, P: Payload>(
    selected: &'s VerifiedSelectedCallable<'p, P>,
    target: &impl PlacementTarget,
) -> Result<PlacementDraft<'s, 'p, P>, CheckFailure> {
    let mut draft = PlacementDraft::new(selected);
    let context = selected.draft().context();
    if context.catalog().plan().profile() != target.profile() {
        return Err(CheckFailure::new(
            CheckLocation::Entry,
            CheckReason::WrongTarget,
        ));
    }
    // Check target footprint capacity before overlap queries. Selected publication
    // validates slot types, whereas the target owns their relative byte layout.
    selected.visit(|fact| {
        if let SelectedFact::Resources { abi_areas, .. } = fact {
            for (&signature, areas) in abi_areas {
                for (area, slots) in [
                    (AbiArea::Incoming, &areas.incoming),
                    (AbiArea::Outgoing, &areas.outgoing),
                    (AbiArea::Results, &areas.results),
                ] {
                    for (index, &representation) in slots.iter().enumerate() {
                        target
                            .slot_footprint(signature, area, index, representation)
                            .map_err(|reason| CheckFailure::new(CheckLocation::Entry, reason))?;
                    }
                }
            }
        }
        Ok(())
    })?;
    let signature = context
        .binding(selected.receipt().key())
        .expect("verified owner")
        .signature_id();
    let mut homes = BTreeMap::<SelectedValueId, (Location, Representation)>::new();
    let mut parameters = BTreeMap::new();
    selected
        .visit::<()>(|fact| {
            match fact {
                SelectedFact::Value {
                    id, representation, ..
                } => {
                    let home = Location::Storage(draft.storage(storage(
                        representation,
                        StoragePurpose::Home(id),
                        StorageLifetime::WholeCallable,
                    )));
                    homes.insert(id, (home, representation));
                }
                SelectedFact::Block {
                    id,
                    parameters: ids,
                    ..
                } => {
                    parameters.insert(id, ids.to_vec());
                }
                _ => {}
            }
            Ok(())
        })
        .expect("infallible visitor");
    let mut saved = vec![];
    let mut preserved = target.preserved_views();
    preserved.sort_unstable();
    for view in preserved {
        let resource = context
            .resources
            .views()
            .find(|(id, _)| *id == view)
            .expect("target preserved view")
            .1;
        let bank = context
            .resources
            .banks()
            .find(|(id, _)| *id == resource.bank)
            .expect("target bank")
            .1;
        let rep = Representation::new(
            if bank == BankKind::Float {
                RepresentationKind::Float
            } else {
                RepresentationKind::Bits
            },
            resource.bits,
        )
        .expect("nonzero target width");
        let home = Location::Storage(draft.storage(storage(
            rep,
            StoragePurpose::CalleeSave(view),
            StorageLifetime::WholeCallable,
        )));
        let save = copy(
            TransferValue::Preserved(view),
            Location::Resource(view),
            home,
            rep,
        );
        resolve(&mut draft, target, TransferPoint::Entry, vec![save.clone()])?;
        saved.push(copy(save.value, home, save.source, rep));
    }
    selected.visit(|fact| {
        match fact {
            SelectedFact::Entry { inputs, abi, .. } => {
                let mut moves = vec![];
                for (slot, (&id, binding)) in inputs
                    .iter()
                    .zip(abi.expect("verified ABI").inputs())
                    .enumerate()
                {
                    let location = match binding.location {
                        AbiLocation::Fixed(v) => Location::Resource(v),
                        AbiLocation::Slot { area, index } => Location::Abi {
                            signature,
                            area,
                            index,
                        },
                    };
                    draft
                        .assign(Assignment::Input(slot), location)
                        .expect("unique input");
                    moves.push(copy(
                        TransferValue::Selected(id),
                        location,
                        homes[&id].0,
                        homes[&id].1,
                    ));
                }
                resolve(&mut draft, target, TransferPoint::Entry, moves)?;
            }
            SelectedFact::Block {
                id,
                parameters: ids,
                ..
            } => {
                for (slot, value) in ids.iter().enumerate() {
                    draft
                        .assign(Assignment::Parameter { block: id, slot }, homes[value].0)
                        .expect("unique parameter");
                }
            }
            SelectedFact::Instruction {
                block,
                ordinal,
                payload,
            } => node(
                &mut draft,
                target,
                &homes,
                &saved,
                Site::Instruction { block, ordinal },
                payload,
                signature,
            )?,
            SelectedFact::Terminal {
                block,
                payload,
                edges,
            } => {
                node(
                    &mut draft,
                    target,
                    &homes,
                    &saved,
                    Site::Terminal(block),
                    payload.expect("verified terminal"),
                    signature,
                )?;
                for (edge, (successor, args)) in edges.iter().enumerate() {
                    let mut moves = vec![];
                    for (slot, (&arg, &parameter)) in
                        args.iter().zip(&parameters[successor]).enumerate()
                    {
                        draft
                            .assign(
                                Assignment::EdgeArgument { block, edge, slot },
                                homes[&arg].0,
                            )
                            .expect("unique edge argument");
                        moves.push(copy(
                            TransferValue::Selected(arg),
                            homes[&arg].0,
                            homes[&parameter].0,
                            homes[&arg].1,
                        ));
                    }
                    resolve(
                        &mut draft,
                        target,
                        TransferPoint::Edge { block, slot: edge },
                        moves,
                    )?;
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(draft)
}
fn node<P: Payload>(
    draft: &mut PlacementDraft<'_, '_, P>,
    target: &impl PlacementTarget,
    homes: &BTreeMap<SelectedValueId, (Location, Representation)>,
    saved: &[Transfer],
    site: Site,
    payload: &P,
    signature: crate::backend::plan::SignatureId,
) -> Result<(), CheckFailure> {
    let d = payload.describe();
    let locations = operands::assign(
        draft,
        target,
        &d,
        site,
        d.call_signature.unwrap_or(signature),
        homes,
    )
    .map_err(|mut failure| {
        failure.origin = payload.span();
        failure
    })?;
    let mut before = vec![];
    let mut after = vec![];
    for (op, location) in d.operands.iter().zip(locations) {
        let home = homes[&op.value].0;
        let transfer = if op.role == OperandRole::Use {
            copy(
                TransferValue::Selected(op.value),
                home,
                location,
                op.representation,
            )
        } else {
            copy(
                TransferValue::Selected(op.value),
                location,
                home,
                op.representation,
            )
        };
        if op.role == OperandRole::Use {
            before.push(transfer);
        } else {
            after.push(transfer);
        }
    }
    if d.flow == Flow::Return {
        before.extend_from_slice(saved);
    }
    // Multiple uses of the identical value at one view are one physical load.
    before.sort_by_key(|t| (t.destination, t.source, t.value));
    before.dedup_by(|a, b| {
        a.destination == b.destination
            && a.source == b.source
            && a.value == b.value
            && a.destination_representation == b.destination_representation
    });
    resolve(draft, target, TransferPoint::Before(site), before).map_err(|mut failure| {
        failure.origin = payload.span();
        failure
    })?;
    if matches!(site, Site::Instruction { .. }) {
        resolve(draft, target, TransferPoint::After(site), after).map_err(|mut failure| {
            failure.origin = payload.span();
            failure
        })?;
    } else if !after.is_empty() {
        return Err(CheckFailure::new(
            CheckLocation::Site(site),
            CheckReason::Constraint,
        ));
    }
    Ok(())
}
