//! Exact group topology and input-derived recipe/transfer acceptance.
use super::super::{
    super::{selected::Instruction as SelectedInstruction, Gpr, NativeResources},
    model::*,
};
use super::{
    authority::{Authority, Cursor},
    recipes, Reason,
};
use crate::backend::{
    frame::{FramePlan, ReturnAddress},
    graph::SelectedBlockId,
    placement::{CheckedPlacement, Site, TransferPoint},
    selected::{Flow, Payload, SelectedFact, VerifiedSelectedCallable},
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn check(
    draft: &PhysicalDraft<'_, '_, '_, '_>,
    selected: &VerifiedSelectedCallable<'_, SelectedInstruction>,
    placement: &CheckedPlacement<'_, '_, SelectedInstruction>,
    frame: &FramePlan<'_, '_, '_, SelectedInstruction>,
    position: &mut (Option<usize>, Option<usize>),
) -> Result<(), Reason> {
    let p = frame.policy();
    if p.alignment != 16
        || p.entry_remainder != 8
        || p.header_bytes != 8
        || p.incoming_base != 16
        || p.return_address
            != (ReturnAddress::Stack {
                offset: 8,
                bytes: 8,
            })
        || p.materialization.is_some()
        || p.direct_min < i64::from(i32::MIN)
        || p.direct_max > i64::from(i32::MAX)
        || frame.bytes() > i32::MAX as usize
        || frame.bytes() % 16 != 0
    {
        return Err(Reason::Recipe);
    }
    let a = Authority {
        placement,
        frame,
        resources: NativeResources::for_profile(
            selected.draft().context().catalog().plan().profile(),
        )
        .map_err(|_| Reason::Recipe)?,
    };
    let mut body = BTreeMap::<SelectedBlockId, BlockId>::new();
    let mut forward = BTreeMap::new();
    let mut entry = None;
    let mut selected_groups = BTreeMap::new();
    for block in &draft.blocks {
        for (g, group) in block.groups.iter().enumerate() {
            *position = (Some(block.id.0), Some(g));
            if let Origin::Selected(site) = group.origin {
                if selected_groups.insert(site, (block.id, g, group)).is_some() {
                    return Err(Reason::Topology);
                }
            }
        }
        match block.origin {
            BlockOrigin::Entry => {
                if entry.replace(block.id).is_some() {
                    return Err(Reason::Topology);
                }
            }
            BlockOrigin::Selected(id) => {
                if body.insert(id, block.id).is_some() {
                    return Err(Reason::Topology);
                }
            }
            BlockOrigin::Forward { block: id, slot } => {
                if forward.insert((id, slot), block.id).is_some() {
                    return Err(Reason::Topology);
                }
            }
        }
    }
    if entry != Some(draft.entry) {
        return Err(Reason::Topology);
    }
    let mut expected = BTreeMap::<BlockId, Vec<Origin>>::new();
    let mut used = BTreeSet::new();
    let mut selected_entry = None;
    let mut add_transfers = |id: BlockId, point| {
        for index in 0..placement.transfers(point).len() {
            expected
                .entry(id)
                .or_default()
                .push(Origin::Transfer { point, index });
        }
    };
    add_transfers(draft.entry, TransferPoint::Entry);
    // Entry protocol is split around entry transfers, exactly once.
    expected
        .entry(draft.entry)
        .or_default()
        .insert(0, Origin::Prologue);
    expected
        .entry(draft.entry)
        .or_default()
        .push(Origin::Prologue);
    selected.visit::<Reason>(|fact| {
        match fact {
            SelectedFact::Entry { entry, .. } => selected_entry = entry,
            SelectedFact::Block { id, .. } => {
                let id = *body.get(&id).ok_or(Reason::Topology)?;
                used.insert(id);
                expected.entry(id).or_default();
            }
            SelectedFact::Instruction {
                block,
                ordinal,
                payload,
            } => {
                let id = *body.get(&block).ok_or(Reason::Topology)?;
                let site = Site::Instruction { block, ordinal };
                site_groups(&mut expected, id, site, placement);
                let (index, group) = find_selected(&selected_groups, id, site)?;
                *position = (Some(id.0), Some(index));
                dependencies(group, payload)?;
                recipes::check(&a, site, payload, &[], &group.instructions)?;
            }
            SelectedFact::Terminal {
                block,
                payload: Some(payload),
                edges,
            } => {
                let id = *body.get(&block).ok_or(Reason::Topology)?;
                let site = Site::Terminal(block);
                let mut successors = vec![];
                for (slot, (target, _)) in edges.iter().enumerate() {
                    let point = TransferPoint::Edge { block, slot };
                    let target = *body.get(target).ok_or(Reason::Topology)?;
                    if placement.transfers(point).is_empty() {
                        if forward.contains_key(&(block, slot)) {
                            return Err(Reason::Topology);
                        }
                        successors.push(target);
                    } else {
                        let id = *forward.get(&(block, slot)).ok_or(Reason::Topology)?;
                        used.insert(id);
                        let origins = expected.entry(id).or_default();
                        for index in 0..placement.transfers(point).len() {
                            origins.push(Origin::Transfer { point, index });
                        }
                        origins.push(Origin::Forward { block, slot });
                        let last = draft.blocks[id.0].groups.last().ok_or(Reason::Topology)?;
                        if last.instructions != [Instruction::Jump(target)] {
                            return Err(Reason::Recipe);
                        }
                        successors.push(id);
                    }
                }
                site_groups(&mut expected, id, site, placement);
                let (index, group) = find_selected(&selected_groups, id, site)?;
                *position = (Some(id.0), Some(index));
                dependencies(group, payload)?;
                recipes::check(&a, site, payload, &successors, &group.instructions)?;
                if payload.describe().flow == Flow::Return {
                    expected.entry(id).or_default().push(Origin::Epilogue(site));
                    let last = draft.blocks[id.0].groups.last().ok_or(Reason::Recipe)?;
                    let mut c = Cursor::new(&last.instructions);
                    c.movement(
                        64,
                        Operand::Register(Register::Gpr(Gpr::Rbp)),
                        Operand::Register(Register::Gpr(Gpr::Rsp)),
                    )?;
                    c.expect(Instruction::Pop(Gpr::Rbp))?;
                    c.expect(Instruction::Return)?;
                    c.finish()?;
                }
            }
            SelectedFact::Terminal { payload: None, .. } => return Err(Reason::Topology),
            _ => {}
        }
        Ok(())
    })?;
    used.insert(draft.entry);
    if used.len() != draft.blocks.len() {
        return Err(Reason::Topology);
    }
    for block in &draft.blocks {
        let origins = expected.get(&block.id).ok_or(Reason::Topology)?;
        if origins.len() != block.groups.len()
            || !origins
                .iter()
                .zip(&block.groups)
                .all(|(a, b)| *a == b.origin)
        {
            return Err(Reason::Topology);
        }
        for (index, group) in block.groups.iter().enumerate() {
            *position = (Some(block.id.0), Some(index));
            if !matches!(group.origin, Origin::Selected(_)) && !group.dependencies.is_empty() {
                return Err(Reason::Dependency);
            }
            if let Origin::Transfer { point, index } = group.origin {
                let t = &placement.transfers(point)[index];
                let source = a.location(t.source)?;
                let destination = a.location(t.destination)?;
                let bits = t.source_representation.bits();
                let mut c = Cursor::new(&group.instructions);
                if matches!(
                    (source, destination),
                    (Operand::Memory { .. }, Operand::Memory { .. })
                ) {
                    let scratch =
                        Operand::Register(a.register(*t.scratch.first().ok_or(Reason::Recipe)?)?);
                    c.movement(bits, source, scratch)?;
                    c.movement(bits, scratch, destination)?;
                } else {
                    c.movement(bits, source, destination)?;
                }
                c.finish()?;
            }
        }
    }
    let groups = &draft.blocks[draft.entry.0].groups;
    let mut c = Cursor::new(&groups.first().ok_or(Reason::Topology)?.instructions);
    c.expect(Instruction::Push(Gpr::Rbp))?;
    c.movement(
        64,
        Operand::Register(Register::Gpr(Gpr::Rsp)),
        Operand::Register(Register::Gpr(Gpr::Rbp)),
    )?;
    if frame.bytes() != 0 {
        c.expect(Instruction::StackSubtract(frame.bytes() as u32))?;
    }
    c.finish()?;
    let entry = *body
        .get(&selected_entry.ok_or(Reason::Topology)?)
        .ok_or(Reason::Topology)?;
    if groups.last().ok_or(Reason::Topology)?.instructions != [Instruction::Jump(entry)] {
        return Err(Reason::Recipe);
    }
    Ok(())
}
fn site_groups(
    expected: &mut BTreeMap<BlockId, Vec<Origin>>,
    id: BlockId,
    site: Site,
    placement: &CheckedPlacement<'_, '_, SelectedInstruction>,
) {
    let origins = expected.entry(id).or_default();
    for index in 0..placement.transfers(TransferPoint::Before(site)).len() {
        origins.push(Origin::Transfer {
            point: TransferPoint::Before(site),
            index,
        });
    }
    origins.push(Origin::Selected(site));
    for index in 0..placement.transfers(TransferPoint::After(site)).len() {
        origins.push(Origin::Transfer {
            point: TransferPoint::After(site),
            index,
        });
    }
}
fn find_selected<'a>(
    groups: &BTreeMap<Site, (BlockId, usize, &'a Group)>,
    id: BlockId,
    site: Site,
) -> Result<(usize, &'a Group), Reason> {
    let (actual, index, group) = groups.get(&site).ok_or(Reason::Topology)?;
    if *actual != id {
        return Err(Reason::Topology);
    }
    Ok((*index, group))
}

fn dependencies(group: &Group, payload: &SelectedInstruction) -> Result<(), Reason> {
    let declared = payload
        .describe()
        .artifacts
        .iter()
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    if group.dependencies == declared {
        Ok(())
    } else {
        Err(Reason::Dependency)
    }
}
