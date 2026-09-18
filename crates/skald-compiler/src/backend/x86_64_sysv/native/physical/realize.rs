use super::super::{selected::Instruction as SelectedInstruction, Gpr, NativeResources};
use super::{
    model::*,
    operands::{mov, Inputs},
};
use crate::backend::{
    frame::{FramePlan, ReturnAddress},
    placement::{CheckedPlacement, Site, TransferPoint},
    selected::{Payload, SelectedFact, VerifiedSelectedCallable},
};
use std::collections::BTreeMap;
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn realize_native<'a, 'f, 's, 'p>(
    selected: &VerifiedSelectedCallable<'p, SelectedInstruction>,
    placement: &'f CheckedPlacement<'s, 'p, SelectedInstruction>,
    frame: &'a FramePlan<'f, 's, 'p, SelectedInstruction>,
) -> Result<PhysicalDraft<'a, 'f, 's, 'p>, RealizeError> {
    placement
        .require_selected(selected)
        .map_err(|_| RealizeError::WrongSelected)?;
    frame.require_placement(placement)?;
    if frame.bytes() > i32::MAX as usize {
        return Err(RealizeError::Frame(
            crate::backend::frame::FrameError::UnsupportedSize(frame.bytes()),
        ));
    }
    let policy = frame.policy();
    if policy.alignment != 16
        || policy.entry_remainder != 8
        || policy.header_bytes != 8
        || policy.incoming_base != 16
        || policy.return_address
            != (ReturnAddress::Stack {
                offset: 8,
                bytes: 8,
            })
        || policy.materialization.is_some()
    {
        return Err(RealizeError::Frame(
            crate::backend::frame::FrameError::InvalidPolicy,
        ));
    }
    let resources =
        NativeResources::for_profile(selected.draft().context().catalog().plan().profile())
            .map_err(|_| RealizeError::InvalidResource)?;
    let input = Inputs {
        placement,
        frame,
        resources,
    };
    let mut blocks = vec![];
    let mut map = BTreeMap::new();
    let mut start = None;
    selected.visit::<RealizeError>(|fact| {
        match fact {
            SelectedFact::Entry { entry, .. } => start = entry,
            SelectedFact::Block { id, .. } => {
                let physical = BlockId(blocks.len());
                map.insert(id, physical);
                blocks.push(Block {
                    id: physical,
                    origin: BlockOrigin::Selected(id),
                    groups: vec![],
                });
            }
            _ => {}
        }
        Ok(())
    })?;
    let entry = BlockId(blocks.len());
    let mut prologue = vec![
        Instruction::Push(Gpr::Rbp),
        mov(
            64,
            Operand::Register(Register::Gpr(Gpr::Rsp)),
            Operand::Register(Register::Gpr(Gpr::Rbp)),
        ),
    ];
    if frame.bytes() != 0 {
        prologue.push(Instruction::StackSubtract(
            u32::try_from(frame.bytes()).map_err(|_| RealizeError::Overflow)?,
        ));
    }
    let mut groups = vec![Group {
        origin: Origin::Prologue,
        instructions: prologue,
        dependencies: vec![],
    }];
    groups.extend(input.transfers(TransferPoint::Entry)?);
    groups.push(Group {
        origin: Origin::Prologue,
        instructions: vec![Instruction::Jump(
            map[&start.ok_or(RealizeError::InvalidResource)?],
        )],
        dependencies: vec![],
    });
    blocks.push(Block {
        id: entry,
        origin: BlockOrigin::Entry,
        groups,
    });
    selected.visit::<RealizeError>(|fact| {
        match fact {
            SelectedFact::Instruction {
                block,
                ordinal,
                payload,
            } => {
                let site = Site::Instruction { block, ordinal };
                let groups = &mut blocks[map[&block].0].groups;
                groups.extend(input.transfers(TransferPoint::Before(site))?);
                groups.push(input.recipe(site, payload, &[])?);
                groups.extend(input.transfers(TransferPoint::After(site))?);
            }
            SelectedFact::Terminal {
                block,
                payload: Some(payload),
                edges,
            } => {
                let site = Site::Terminal(block);
                let mut successors = vec![];
                for (slot, (target, _)) in edges.iter().enumerate() {
                    let mut groups = input.transfers(TransferPoint::Edge { block, slot })?;
                    if groups.is_empty() {
                        successors.push(map[target]);
                    } else {
                        let id = BlockId(blocks.len());
                        groups.push(Group {
                            origin: Origin::Forward { block, slot },
                            instructions: vec![Instruction::Jump(map[target])],
                            dependencies: vec![],
                        });
                        blocks.push(Block {
                            id,
                            origin: BlockOrigin::Forward { block, slot },
                            groups,
                        });
                        successors.push(id);
                    }
                }
                let groups = &mut blocks[map[&block].0].groups;
                groups.extend(input.transfers(TransferPoint::Before(site))?);
                groups.push(input.recipe(site, payload, &successors)?);
                groups.extend(input.transfers(TransferPoint::After(site))?);
                if payload.describe().flow == crate::backend::selected::Flow::Return {
                    groups.push(Group {
                        origin: Origin::Epilogue(site),
                        instructions: vec![
                            mov(
                                64,
                                Operand::Register(Register::Gpr(Gpr::Rbp)),
                                Operand::Register(Register::Gpr(Gpr::Rsp)),
                            ),
                            Instruction::Pop(Gpr::Rbp),
                            Instruction::Return,
                        ],
                        dependencies: vec![],
                    });
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(PhysicalDraft {
        frame,
        entry,
        blocks,
    })
}
