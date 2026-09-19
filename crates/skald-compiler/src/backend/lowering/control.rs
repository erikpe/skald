use super::{context::Lowerer, LowerError};
use crate::{
    backend::{
        lir::{Edge, Terminator},
        plan::PlanError,
    },
    mir::{BlockId, MirTerminator},
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn edge(
        &self,
        target: BlockId,
    ) -> Edge<crate::backend::lir::ValueHandle<'plan>, crate::backend::lir::BlockHandle<'plan>>
    {
        Edge {
            target: self.blocks[target.index()],
            arguments: vec![],
        }
    }

    pub(super) fn terminator(
        &mut self,
        block: BlockId,
        terminator: &MirTerminator,
    ) -> Result<(), LowerError> {
        let terminator = match terminator {
            MirTerminator::Return { value, .. } => {
                self.pop_trace(block)?;
                Terminator::Return(value.iter().map(|v| self.values[v.index()]).collect())
            }
            MirTerminator::ReturnShared { owner, .. } => {
                let value = self.load_storage(block, *owner)?;
                self.pop_trace(block)?;
                Terminator::Return(vec![value])
            }
            MirTerminator::ReturnOptionalShared { owner, .. } => {
                let value = self.load_storage(block, *owner)?;
                self.pop_trace(block)?;
                Terminator::Return(vec![value])
            }
            MirTerminator::Goto { target, .. } => Terminator::Jump(self.edge(*target)),
            MirTerminator::Branch {
                condition,
                true_target,
                false_target,
                ..
            } => Terminator::Branch {
                condition: self.values[condition.index()],
                true_edge: self.edge(*true_target),
                false_edge: self.edge(*false_target),
            },
            MirTerminator::ShiftCountCheck {
                success_target,
                failure_target,
                ..
            }
            | MirTerminator::IntegerDivisorCheck {
                success_target,
                failure_target,
                ..
            }
            | MirTerminator::PrimitiveCastRangeCheck {
                success_target,
                failure_target,
                ..
            } => self.numeric_check(block, *success_target, *failure_target)?,
            MirTerminator::Terminate { reason, span } => {
                self.report_failure(block, *reason, *span)?
            }
            MirTerminator::CheckedCast {
                binding,
                success_target,
                failure_target,
                ..
            } => return self.checked_cast(block, binding, *success_target, *failure_target),
            MirTerminator::SharedCast {
                cast,
                success_target,
                failure_target,
                ..
            } => return self.checked_shared_cast(block, cast, *success_target, *failure_target),
            optional if self.optional_terminator(block, optional)? => return Ok(()),
            array if self.array_terminator(block, array)? => return Ok(()),
            _ => return Err(PlanError::InvalidDomain.into()),
        };
        self.builder
            .terminate(self.active_blocks[block.index()], terminator)?;
        Ok(())
    }
}
