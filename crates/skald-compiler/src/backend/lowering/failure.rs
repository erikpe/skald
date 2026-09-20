//! Language failures are explicit typed reporter calls with source attribution.
use super::{context::Lowerer, LowerError};
use crate::{
    backend::{
        failure::FailureMessage,
        lir::{
            BlockHandle, Call, CallArgument, CallTarget, Constant, MemoryRepresentation, Operation,
            Terminator, ValueHandle,
        },
        plan::{ArtifactId, ComponentRole, DataKey, PlanError, RuntimeService, ScalarType},
    },
    mir::{BlockId, MirPlace, MirTerminationReason},
    source::Span,
};
impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn report_dynamic_failure(
        &mut self,
        block: BlockId,
        message: &MirPlace,
        origin: Span,
    ) -> Result<Terminator<ValueHandle<'plan>, BlockHandle<'plan>>, LowerError> {
        let item = self
            .plan()
            .semantic()
            .string
            .ok_or(PlanError::UnknownDeclaration)?;
        let object = self.place_address(block, message)?;
        let storage =
            self.load_field(block, object, item.storage_field, ScalarType::DataAddress)?;
        let array = self
            .plan()
            .semantic()
            .array(item.storage_array)
            .ok_or(PlanError::UnknownDeclaration)?;
        let bytes = self.byte_offset(block, storage, array.shared_element_offset)?;
        let start = self.load_field(block, object, item.start_field, ScalarType::I64)?;
        let bytes = self.append(
            block,
            Operation::ByteOffset {
                base: bytes,
                offset: start,
            },
        )?[0];
        let length = self.load_field(block, object, item.length_field, ScalarType::U64)?;
        let target = ArtifactId::Runtime(RuntimeService::Panic);
        let signature = self
            .plan()
            .artifact(self.plan().artifact_id(target)?, target.category())?
            .signature
            .ok_or(PlanError::InvalidSignature)?;
        Ok(Terminator::NonReturningCall(Call {
            target: CallTarget::Direct(target),
            signature,
            arguments: vec![
                CallArgument {
                    role: ComponentRole::RuntimeParameter(0),
                    value: bytes,
                },
                CallArgument {
                    role: ComponentRole::RuntimeParameter(1),
                    value: length,
                },
            ],
            attribution: self.attribution(block, origin, true)?,
        }))
    }

    fn load_field(
        &mut self,
        block: BlockId,
        object: ValueHandle<'plan>,
        field: crate::identity::FieldId,
        scalar: ScalarType,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let field = self
            .plan()
            .semantic()
            .field(field)
            .ok_or(PlanError::UnknownDeclaration)?;
        let layout = self
            .plan()
            .layout(self.plan().layout_id(field.layout.index())?)?;
        let address = self.byte_offset(block, object, field.offset)?;
        Ok(self.append(
            block,
            Operation::Load {
                address,
                representation: MemoryRepresentation {
                    scalar,
                    bytes: layout.size,
                    alignment: layout.alignment,
                },
            },
        )?[0])
    }

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
            MirTerminationReason::ArrayAllocationFailure => FailureMessage::ArrayAllocationFailure,
            MirTerminationReason::ArrayIndexOutOfBounds => FailureMessage::ArrayIndexOutOfBounds,
            MirTerminationReason::ArrayInvalidSliceBounds => {
                FailureMessage::ArrayInvalidSliceBounds
            }
            MirTerminationReason::ArraySliceLengthMismatch => {
                FailureMessage::ArraySliceLengthMismatch
            }
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
