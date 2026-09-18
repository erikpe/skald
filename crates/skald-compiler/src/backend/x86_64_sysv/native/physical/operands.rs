use super::super::selected::Instruction as SelectedInstruction;
use super::{super::NativeResources, model::*};
use crate::backend::{
    frame::{Base, FramePlan},
    placement::{Assignment, CheckedPlacement, Location, Site},
    selected::ViewId,
};

pub(super) struct Inputs<'a, 'f, 's, 'p> {
    pub placement: &'f CheckedPlacement<'s, 'p, SelectedInstruction>,
    pub frame: &'a FramePlan<'f, 's, 'p, SelectedInstruction>,
    pub resources: NativeResources,
}
impl Inputs<'_, '_, '_, '_> {
    pub(super) fn register(&self, view: ViewId) -> Result<Register, RealizeError> {
        for gpr in super::super::Gpr::ALL {
            for bits in [8, 16, 32, 64] {
                if self.resources.gpr(gpr, bits).ok() == Some(view) {
                    return Ok(Register::Gpr(gpr));
                }
            }
        }
        for index in 0..16 {
            for bits in [64, 128] {
                if self.resources.xmm(index, bits).ok() == Some(view) {
                    return Ok(Register::Xmm(index as u8));
                }
            }
        }
        Err(RealizeError::InvalidResource)
    }
    pub(super) fn location(&self, location: Location) -> Result<Operand, RealizeError> {
        if let Location::Resource(view) = location {
            return Ok(Operand::Register(self.register(view)?));
        }
        let region = self
            .frame
            .location(location)?
            .ok_or(RealizeError::InvalidResource)?;
        Ok(Operand::Memory {
            base: match region.base {
                Base::Frame => super::super::Gpr::Rbp,
                Base::Stack => super::super::Gpr::Rsp,
            },
            displacement: i32::try_from(region.offset).map_err(|_| RealizeError::Overflow)?,
        })
    }
    pub(super) fn operand(&self, site: Site, slot: usize) -> Result<Operand, RealizeError> {
        self.location(
            self.placement
                .assignment(Assignment::Operand { site, slot })
                .ok_or(RealizeError::MissingOperand(site, slot))?,
        )
    }
    pub(super) fn reg(&self, site: Site, slot: usize) -> Result<Register, RealizeError> {
        match self.operand(site, slot)? {
            Operand::Register(r) => Ok(r),
            _ => Err(RealizeError::UnsupportedRecipe(site)),
        }
    }
    pub(super) fn scratch(&self, site: Site, slot: usize) -> Result<Register, RealizeError> {
        match self.placement.assignment(Assignment::Scratch {
            site,
            group: 0,
            slot,
        }) {
            Some(Location::Resource(view)) => self.register(view),
            _ => Err(RealizeError::UnsupportedRecipe(site)),
        }
    }
}
pub(super) fn mov(bits: u16, source: Operand, destination: Operand) -> Instruction {
    let float = |op| matches!(op, Operand::Register(Register::Xmm(_)));
    let integer = |op| matches!(op, Operand::Register(Register::Gpr(_)));
    let kind = if (float(source) && integer(destination)) || (integer(source) && float(destination))
    {
        MoveKind::Bits
    } else if float(source) || float(destination) {
        MoveKind::Float
    } else {
        MoveKind::Integer
    };
    Instruction::Move {
        kind,
        bits,
        source,
        destination,
    }
}
