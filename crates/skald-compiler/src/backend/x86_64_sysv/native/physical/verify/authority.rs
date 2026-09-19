//! Decode checked authorities without using the realizer's location resolver.
use super::super::super::{selected::Instruction as SelectedInstruction, Gpr, NativeResources};
use super::super::model::*;
use super::Reason;
use crate::backend::{
    frame::{Base, FramePlan},
    placement::{Assignment, CheckedPlacement, Location, Site},
    selected::ViewId,
};
pub(super) struct Authority<'a, 'f, 's, 'p> {
    pub placement: &'f CheckedPlacement<'s, 'p, SelectedInstruction>,
    pub frame: &'a FramePlan<'f, 's, 'p, SelectedInstruction>,
    pub resources: NativeResources,
}
impl Authority<'_, '_, '_, '_> {
    pub fn register(&self, view: ViewId) -> Result<Register, Reason> {
        let catalog = &self.placement.selected().draft().context().resources;
        // View identities must belong to the selected canonical native catalog.
        for reg in Gpr::ALL {
            for bits in [8, 16, 32, 64] {
                if self.resources.gpr(reg, bits).ok() == Some(view)
                    && catalog.view_units(view).is_ok()
                {
                    return Ok(Register::Gpr(reg));
                }
            }
        }
        for i in 0..16 {
            for bits in [64, 128] {
                if self.resources.xmm(i, bits).ok() == Some(view)
                    && catalog.view_units(view).is_ok()
                {
                    return Ok(Register::Xmm(i as u8));
                }
            }
        }
        Err(Reason::Recipe)
    }
    pub fn location(&self, location: Location) -> Result<Operand, Reason> {
        if let Location::Resource(view) = location {
            return self.register(view).map(Operand::Register);
        }
        let region = self
            .frame
            .location(location)
            .map_err(|_| Reason::Recipe)?
            .ok_or(Reason::Recipe)?;
        Ok(Operand::Memory {
            base: match region.base {
                Base::Frame => Gpr::Rbp,
                Base::Stack => Gpr::Rsp,
            },
            displacement: i32::try_from(region.offset).map_err(|_| Reason::Recipe)?,
        })
    }
    pub fn operand(&self, site: Site, slot: usize) -> Result<Operand, Reason> {
        self.location(
            self.placement
                .assignment(Assignment::Operand { site, slot })
                .ok_or(Reason::Recipe)?,
        )
    }
    pub fn reg(&self, site: Site, slot: usize) -> Result<Register, Reason> {
        match self.operand(site, slot)? {
            Operand::Register(r) => Ok(r),
            _ => Err(Reason::Recipe),
        }
    }
    pub fn scratch(&self, site: Site, slot: usize) -> Result<Register, Reason> {
        match self.placement.assignment(Assignment::Scratch {
            site,
            group: 0,
            slot,
        }) {
            Some(Location::Resource(v)) => self.register(v),
            _ => Err(Reason::Recipe),
        }
    }
}
/// A checking cursor, not an expansion producer. Every concrete step is consumed.
pub(super) struct Cursor<'a> {
    code: &'a [Instruction],
    next: usize,
}
impl<'a> Cursor<'a> {
    pub fn new(code: &'a [Instruction]) -> Self {
        Self { code, next: 0 }
    }
    pub fn expect(&mut self, expected: Instruction) -> Result<(), Reason> {
        if self.code.get(self.next) != Some(&expected) {
            return Err(Reason::Recipe);
        }
        self.next += 1;
        Ok(())
    }
    pub fn movement(
        &mut self,
        bits: u16,
        source: Operand,
        destination: Operand,
    ) -> Result<(), Reason> {
        let Some(Instruction::Move {
            kind,
            bits: actual,
            source: s,
            destination: d,
        }) = self.code.get(self.next)
        else {
            return Err(Reason::Recipe);
        };
        if *actual != bits || *s != source || *d != destination {
            return Err(Reason::Recipe);
        }
        let expected = match (source, destination) {
            (Operand::Register(Register::Xmm(_)), Operand::Register(Register::Gpr(_)))
            | (Operand::Register(Register::Gpr(_)), Operand::Register(Register::Xmm(_))) => {
                MoveKind::Bits
            }
            (Operand::Register(Register::Xmm(_)), _) | (_, Operand::Register(Register::Xmm(_))) => {
                MoveKind::Float
            }
            _ => MoveKind::Integer,
        };
        if *kind != expected {
            return Err(Reason::Recipe);
        }
        self.next += 1;
        Ok(())
    }
    pub fn finish(self) -> Result<(), Reason> {
        if self.next == self.code.len() {
            Ok(())
        } else {
            Err(Reason::Recipe)
        }
    }
}
