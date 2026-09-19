//! Language failures are explicit typed reporter calls with source attribution.
use super::{context::Lowerer, LowerError};
use crate::{
    backend::{
        failure::FailureMessage,
        lir::{
            BlockHandle, Call, CallArgument, CallTarget, Constant, Operation, Terminator,
            ValueHandle,
        },
        plan::{ArtifactId, ComponentRole, DataKey, PlanError, RuntimeService, ScalarType},
    },
    mir::{BlockId, MirTerminationReason},
    source::Span,
};
impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn report_failure(
        &mut self,
        block: BlockId,
        reason: MirTerminationReason,
        origin: Span,
    ) -> Result<Terminator<ValueHandle<'plan>, BlockHandle<'plan>>, LowerError> {
        let reason = match reason {
            MirTerminationReason::IntegerDivisionByZero => FailureMessage::IntegerDivisionByZero,
            MirTerminationReason::IntegerRemainderByZero => FailureMessage::IntegerRemainderByZero,
            MirTerminationReason::ShiftCountOutOfRange => FailureMessage::ShiftCountOutOfRange,
            MirTerminationReason::PrimitiveCastOutOfRange => {
                FailureMessage::PrimitiveCastOutOfRange
            }
            MirTerminationReason::ObjectCastFailure => FailureMessage::ObjectCastFailure,
            MirTerminationReason::OptionalAccessFailure => FailureMessage::OptionalAccessFailure,
            MirTerminationReason::OptionalGuardOverflow => FailureMessage::OptionalGuardOverflow,
            MirTerminationReason::OptionalPinnedMutation => FailureMessage::OptionalPinnedMutation,
            _ => return Err(PlanError::InvalidDomain.into()),
        };
        let target = ArtifactId::Runtime(RuntimeService::Panic);
        let signature = self
            .plan()
            .artifact(self.plan().artifact_id(target)?, target.category())?
            .signature
            .ok_or(PlanError::InvalidSignature)?;
        let source_block = block;
        let block = self.active_blocks[block.index()];
        let message = self.builder.append(
            block,
            Operation::SymbolAddress {
                symbol: ArtifactId::Data(DataKey::FailureMessage(reason)),
                ty: ScalarType::DataAddress,
            },
        )?[0];
        let length = self.builder.append(
            block,
            Operation::Constant(Constant::U64(reason.bytes().len() as u64)),
        )?[0];
        let attribution = self.attribution(source_block, origin, true)?;
        Ok(Terminator::ReportFailure {
            reason,
            call: Call {
                target: CallTarget::Direct(target),
                signature,
                arguments: vec![
                    CallArgument {
                        role: ComponentRole::RuntimeParameter(0),
                        value: message,
                    },
                    CallArgument {
                        role: ComponentRole::RuntimeParameter(1),
                        value: length,
                    },
                ],
                attribution,
            },
        })
    }
}
