use super::{context::Lowerer, memory::local, LowerError, PendingFeature};
use crate::{
    backend::{
        lir::{
            BinaryOperation, BlockHandle, Constant, LifetimeMarker, ObjectHandle, Operation,
            UnaryOperation, ValueHandle,
        },
        plan::{ArtifactId, LirCallableId, PlanError, ScalarType},
    },
    mir::{
        BlockId, MirBinaryOperation, MirInstruction, MirIntegerBitwiseOperation, MirRvalueKind,
        MirUnaryOperation,
    },
};

pub(super) type ScalarOperation<'p> = Operation<ValueHandle<'p>, ObjectHandle<'p>, BlockHandle<'p>>;

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn instruction(
        &mut self,
        block: BlockId,
        instruction: &MirInstruction,
    ) -> Result<(), LowerError> {
        match instruction {
            MirInstruction::StorageLive(live) => {
                self.lifetime(block, live.storage, LifetimeMarker::Start)?
            }
            MirInstruction::StorageDead(dead) => {
                self.lifetime(block, dead.storage, LifetimeMarker::End)?
            }
            MirInstruction::Store(store) => self.store(
                block,
                local(&store.destination)?,
                self.values[store.value.index()],
            )?,
            MirInstruction::EndFullExpression(end) if end.temporaries.is_empty() => {}
            MirInstruction::Assign(assign) => {
                if self
                    .guards
                    .get(&block)
                    .is_some_and(|guard| guard.value == assign.result)
                {
                    return Ok(()); // The secured load is defined by the predecessor check.
                }
                let operation = self.rvalue(block, &assign.rvalue.kind)?;
                self.builder.append_into(
                    self.blocks[block.index()],
                    operation,
                    &[self.values[assign.result.index()]],
                )?;
            }
            MirInstruction::Call(_) => return Err(self.pending(PendingFeature::Calls)),
            _ => return Err(PlanError::InvalidDomain.into()),
        }
        Ok(())
    }
    fn rvalue(
        &mut self,
        block: BlockId,
        kind: &MirRvalueKind,
    ) -> Result<ScalarOperation<'plan>, LowerError> {
        Ok(match kind {
            MirRvalueKind::ConstantI64(value) => Operation::Constant(Constant::I64(*value)),
            MirRvalueKind::ConstantU64(value) => Operation::Constant(Constant::U64(*value)),
            MirRvalueKind::ConstantU8(value) => Operation::Constant(Constant::U8(*value)),
            MirRvalueKind::ConstantBool(value) => Operation::Constant(Constant::Bool(*value)),
            MirRvalueKind::ConstantF64Bits(bits) => Operation::Constant(Constant::F64(*bits)),
            MirRvalueKind::CallableAddress(address) => Operation::SymbolAddress {
                symbol: ArtifactId::Callable(LirCallableId::Source(address.target)),
                ty: ScalarType::CodeAddress(
                    self.admitted
                        .function_type(address.function_type)
                        .ok_or(PlanError::UnknownDeclaration)?,
                ),
            },
            MirRvalueKind::Load(place) => {
                let storage = local(place)?;
                Operation::Load {
                    address: self.address(block, storage)?,
                    representation: self.representation(storage)?,
                }
            }
            MirRvalueKind::PathCondition(condition) => Operation::Load {
                address: self.address(block, condition.activation)?,
                representation: self.representation(condition.activation)?,
            },
            MirRvalueKind::Unary { operation, operand } => Operation::Unary {
                operation: unary(*operation),
                value: self.values[operand.index()],
            },
            MirRvalueKind::Binary {
                operation,
                left,
                right,
            } => Operation::Binary {
                operation: binary(*operation),
                left: self.values[left.index()],
                right: self.values[right.index()],
            },
            MirRvalueKind::PrimitiveComparison {
                operation,
                left,
                right,
            } => Operation::Compare {
                predicate: operation.predicate,
                left: self.values[left.index()],
                right: self.values[right.index()],
            },
            MirRvalueKind::IntegerDivision { .. }
            | MirRvalueKind::Shift { .. }
            | MirRvalueKind::PrimitiveCast { .. }
            | MirRvalueKind::CheckedF64ToInteger { .. } => {
                return self.guarded_operation(block, kind)
            }
            _ => return Err(PlanError::InvalidDomain.into()),
        })
    }
    pub(super) fn pending(&self, feature: PendingFeature) -> LowerError {
        LowerError::Pending {
            callable: LirCallableId::Source(self.definition.callable()),
            feature,
        }
    }
}

fn unary(operation: MirUnaryOperation) -> UnaryOperation {
    match operation {
        MirUnaryOperation::NegateI64 | MirUnaryOperation::NegateF64 => UnaryOperation::Negate,
        MirUnaryOperation::LogicalNotBool => UnaryOperation::LogicalNot,
        MirUnaryOperation::BitwiseComplement(_) => UnaryOperation::Complement,
    }
}
fn binary(operation: MirBinaryOperation) -> BinaryOperation {
    match operation {
        MirBinaryOperation::AddI64
        | MirBinaryOperation::AddU64
        | MirBinaryOperation::AddU8
        | MirBinaryOperation::AddF64 => BinaryOperation::Add,
        MirBinaryOperation::SubtractI64
        | MirBinaryOperation::SubtractU64
        | MirBinaryOperation::SubtractU8
        | MirBinaryOperation::SubtractF64 => BinaryOperation::Subtract,
        MirBinaryOperation::MultiplyI64
        | MirBinaryOperation::MultiplyU64
        | MirBinaryOperation::MultiplyU8
        | MirBinaryOperation::MultiplyF64 => BinaryOperation::Multiply,
        MirBinaryOperation::DivideF64 => BinaryOperation::FloatDivide,
        MirBinaryOperation::IntegerBitwise { operation, .. } => match operation {
            MirIntegerBitwiseOperation::And => BinaryOperation::And,
            MirIntegerBitwiseOperation::Or => BinaryOperation::Or,
            MirIntegerBitwiseOperation::Xor => BinaryOperation::Xor,
        },
    }
}
