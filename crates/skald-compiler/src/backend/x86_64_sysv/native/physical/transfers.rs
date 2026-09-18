use super::{
    model::*,
    operands::{mov, Inputs},
};
use crate::backend::placement::TransferPoint;
impl Inputs<'_, '_, '_, '_> {
    pub(super) fn transfers(&self, point: TransferPoint) -> Result<Vec<Group>, RealizeError> {
        self.placement
            .transfers(point)
            .iter()
            .enumerate()
            .map(|(index, transfer)| {
                let source = self.location(transfer.source)?;
                let destination = self.location(transfer.destination)?;
                let bits = transfer.source_representation.bits();
                let instructions = if matches!(source, Operand::Memory { .. })
                    && matches!(destination, Operand::Memory { .. })
                {
                    let view = transfer
                        .scratch
                        .first()
                        .ok_or(RealizeError::InvalidResource)?;
                    let scratch = Operand::Register(self.register(*view)?);
                    vec![mov(bits, source, scratch), mov(bits, scratch, destination)]
                } else {
                    vec![mov(bits, source, destination)]
                };
                Ok(Group {
                    origin: Origin::Transfer { point, index },
                    instructions,
                    dependencies: vec![],
                })
            })
            .collect()
    }
}
