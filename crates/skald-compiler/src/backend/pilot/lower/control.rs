use super::{context::Lowerer, LowerError, PendingFeature};
use crate::{
    backend::{
        lir::{Edge, Terminator},
        plan::PlanError,
    },
    mir::{BlockId, MirTerminator},
};

impl Lowerer<'_, '_> {
    pub(super) fn terminator(
        &mut self,
        block: BlockId,
        terminator: &MirTerminator,
    ) -> Result<(), LowerError> {
        let edge = |target: BlockId| Edge {
            target: self.blocks[target.index()],
            arguments: vec![],
        };
        let terminator = match terminator {
            MirTerminator::Return { value, .. } => {
                Terminator::Return(value.iter().map(|v| self.values[v.index()]).collect())
            }
            MirTerminator::Goto { target, .. } => Terminator::Jump(edge(*target)),
            MirTerminator::Branch {
                condition,
                true_target,
                false_target,
                ..
            } => Terminator::Branch {
                condition: self.values[condition.index()],
                true_edge: edge(*true_target),
                false_edge: edge(*false_target),
            },
            MirTerminator::ShiftCountCheck { .. }
            | MirTerminator::IntegerDivisorCheck { .. }
            | MirTerminator::PrimitiveCastRangeCheck { .. }
            | MirTerminator::Terminate { .. } => {
                return Err(self.pending(PendingFeature::GuardedNumeric))
            }
            _ => return Err(PlanError::InvalidDomain.into()),
        };
        self.builder
            .terminate(self.blocks[block.index()], terminator)?;
        Ok(())
    }
}
