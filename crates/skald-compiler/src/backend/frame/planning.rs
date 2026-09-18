use super::{layout, model::*};
use crate::backend::{
    placement::{Assignment, CheckedPlacement, Location, Site},
    selected::{AbiArea, ObjectRole, Payload, SelectedFact},
};
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::backend) fn plan_frame<'f, 's, 'p, P: Payload>(
    placement: &'f CheckedPlacement<'s, 'p, P>,
    policy: FramePolicy,
) -> Result<FramePlan<'f, 's, 'p, P>, FrameError> {
    if !policy.alignment.is_power_of_two()
        || policy.entry_remainder >= policy.alignment
        || policy.header_bytes % policy.alignment != policy.entry_remainder
        || policy.direct_min > policy.direct_max
        || policy
            .materialization
            .is_some_and(|(lo, hi, steps)| lo > hi || steps < 2)
    {
        return Err(FrameError::InvalidPolicy);
    }
    let context = placement.selected().draft().context();
    let pointer_bytes = context.catalog().plan().profile().data_layout.pointer_bytes;
    if policy.header_bytes < pointer_bytes
        || policy.incoming_base < policy.header_bytes
        || i64::try_from(policy.header_bytes).is_err()
        || i64::try_from(policy.incoming_base).is_err()
    {
        return Err(FrameError::InvalidPolicy);
    }
    let (return_offset, return_bytes) = match policy.return_address {
        ReturnAddress::Stack { offset, bytes } => {
            if offset < policy.header_bytes
                || offset
                    .checked_add(bytes)
                    .is_none_or(|end| end > policy.incoming_base)
            {
                return Err(FrameError::InvalidPolicy);
            }
            (offset, bytes)
        }
        ReturnAddress::Link {
            view,
            offset,
            bytes,
        } => {
            if offset < pointer_bytes
                || offset
                    .checked_add(bytes)
                    .is_none_or(|end| end > policy.header_bytes)
                || context
                    .resources
                    .require_view(
                        view,
                        (pointer_bytes * 8) as u16,
                        crate::backend::selected::BankKind::Integer,
                        false,
                    )
                    .is_err()
            {
                return Err(FrameError::InvalidPolicy);
            }
            (offset, bytes)
        }
    };
    if return_bytes != pointer_bytes || return_offset % pointer_bytes != 0 {
        return Err(FrameError::InvalidPolicy);
    }
    let mut cursor = 0;
    let mut regions = BTreeMap::new();
    let mut areas = BTreeSet::new();
    let mut abi_objects = vec![];
    placement.selected().visit(|fact| {
        match fact {
            SelectedFact::Object {
                id, layout, role, ..
            } => match role {
                ObjectRole::Abi { signature, area } => {
                    areas.insert((signature, area));
                    abi_objects.push((id, signature, area));
                }
                _ => {
                    regions.insert(
                        Key::Object(id),
                        layout::local(&mut cursor, layout.size, layout.alignment, policy)?,
                    );
                }
            },
            SelectedFact::Instruction { payload, .. }
            | SelectedFact::Terminal {
                payload: Some(payload),
                ..
            } => {
                if let Some(sig) = payload.describe().call_signature {
                    areas.insert((sig, AbiArea::Outgoing));
                    areas.insert((sig, AbiArea::Results));
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    for (id, storage) in placement.storage().iter().enumerate() {
        regions.insert(
            Key::Storage(id),
            layout::local(&mut cursor, storage.bytes, storage.alignment, policy)?,
        );
    }
    for (_, location) in placement.assignments() {
        area_location(location, &mut areas);
    }
    for point in placement.transfer_points() {
        for transfer in placement.transfers(point) {
            area_location(transfer.source, &mut areas);
            area_location(transfer.destination, &mut areas);
        }
    }
    let mut outgoing_bytes = 0;
    let mut results_bytes = 0;
    for &(sig, area) in &areas {
        let shape = context
            .abi_layout(sig, area)
            .ok_or(FrameError::MissingAbiLayout(sig, area))?;
        if shape.alignment > policy.alignment {
            return Err(FrameError::UnsupportedAlignment(shape.alignment));
        }
        match area {
            AbiArea::Outgoing => outgoing_bytes = outgoing_bytes.max(shape.bytes),
            AbiArea::Results => results_bytes = results_bytes.max(shape.bytes),
            AbiArea::Incoming => {}
        }
    }
    // Separate result area, shared by signatures; no object/storage lifetime reuse.
    outgoing_bytes = layout::align(outgoing_bytes, policy.alignment)?;
    let body = cursor
        .checked_add(outgoing_bytes)
        .and_then(|n| n.checked_add(results_bytes))
        .ok_or(FrameError::Overflow)?;
    let bytes = layout::align(body, policy.alignment)?;
    if bytes > policy.max_frame {
        return Err(FrameError::UnsupportedSize(bytes));
    }
    for &(sig, area) in &areas {
        let shape = context
            .abi_layout(sig, area)
            .ok_or(FrameError::MissingAbiLayout(sig, area))?;
        let (base, offset) = match area {
            AbiArea::Incoming => (Base::Frame, policy.incoming_base),
            AbiArea::Outgoing => (Base::Stack, 0),
            AbiArea::Results => (Base::Stack, outgoing_bytes),
        };
        if offset % shape.alignment != 0 {
            return Err(FrameError::UnsupportedAlignment(shape.alignment));
        }
        regions.insert(
            Key::Abi(sig, area),
            Region {
                base,
                offset: i64::try_from(offset).map_err(|_| FrameError::Overflow)?,
                bytes: shape.bytes,
                alignment: shape.alignment,
            },
        );
    }
    for (id, sig, area) in abi_objects {
        regions.insert(Key::Object(id), regions[&Key::Abi(sig, area)]);
    }
    let mut plan = FramePlan {
        placement,
        policy,
        bytes,
        outgoing_bytes,
        regions,
        object_accesses: BTreeMap::new(),
    };
    // Actual transfers cannot borrow an instruction's bundle scratch.
    for point in placement.transfer_points() {
        for transfer in placement.transfers(point) {
            plan.check_location(transfer.source)?;
            plan.check_location(transfer.destination)?;
        }
    }
    for (assignment, location) in placement.assignments() {
        if matches!(assignment, Assignment::Operand { .. }) {
            plan.check_location(location)?;
        }
    }
    placement.selected().visit(|fact| {
        match fact {
            SelectedFact::Instruction {
                block,
                ordinal,
                payload,
            } => plan.objects(Site::Instruction { block, ordinal }, payload)?,
            SelectedFact::Terminal {
                block,
                payload: Some(payload),
                ..
            } => plan.objects(Site::Terminal(block), payload)?,
            _ => {}
        }
        Ok(())
    })?;
    Ok(plan)
}
fn area_location(
    location: Location,
    areas: &mut BTreeSet<(crate::backend::plan::SignatureId, AbiArea)>,
) {
    if let Location::Abi {
        signature, area, ..
    } = location
    {
        areas.insert((signature, area));
    }
}
