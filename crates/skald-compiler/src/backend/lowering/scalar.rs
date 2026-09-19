use super::{context::Lowerer, LowerError};
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
            MirInstruction::Store(store) => {
                let address = self.place_address(block, &store.destination)?;
                let representation = self.place_representation(&store.destination)?;
                self.builder.append(
                    self.blocks[block.index()],
                    Operation::Store {
                        address,
                        value: self.values[store.value.index()],
                        representation,
                    },
                )?;
            }
            MirInstruction::EndFullExpression(end) => self.end_full_expression(block, end)?,
            MirInstruction::Cleanup(cleanup) => self.cleanup(block, cleanup)?,
            MirInstruction::Assign(assign) => {
                if self
                    .guards
                    .get(&block)
                    .is_some_and(|guard| guard.value == assign.result)
                {
                    return Ok(()); // The secured load is defined by the predecessor check.
                }
                if let MirRvalueKind::TypeTest { source, target } = &assign.rvalue.kind {
                    self.type_test(block, assign.result, source, *target)?;
                    return Ok(());
                }
                let operation = self.rvalue(block, &assign.rvalue.kind)?;
                self.builder.append_into(
                    self.blocks[block.index()],
                    operation,
                    &[self.values[assign.result.index()]],
                )?;
            }
            MirInstruction::Call(call) => self.call(block, call)?,
            MirInstruction::Initialize(initialize) => self.initialize(block, initialize)?,
            MirInstruction::CopyConstruct(copy) => self.copy_construct(block, copy)?,
            MirInstruction::CopyAssign(copy) => self.copy_assign(block, copy)?,
            MirInstruction::BindCheckedView(binding) => self.bind_checked_view(block, binding)?,
            MirInstruction::EndCheckedView(_) => {}
            MirInstruction::SharedAllocate(allocation) => {
                self.shared_allocate(block, allocation)?
            }
            MirInstruction::SharedInitialize(initialize) => {
                self.shared_initialize(block, initialize)?
            }
            MirInstruction::SharedPublish(publish) => self.shared_publish(block, publish)?,
            MirInstruction::SharedStatic(static_owner) => {
                self.shared_static(block, static_owner)?
            }
            MirInstruction::SharedAdopt(adopt) => self.shared_adopt(block, adopt)?,
            MirInstruction::SharedCopy(copy) => self.shared_copy(block, copy)?,
            MirInstruction::SharedFieldCopy(copy) => self.shared_field_copy(block, copy)?,
            MirInstruction::SharedCast(cast) => self.shared_cast(block, cast)?,
            MirInstruction::SharedMove(transfer) => self.shared_move(block, transfer)?,
            MirInstruction::SharedRelease(release) => self.shared_release(block, release)?,
            MirInstruction::SharedFieldInitialize(initialize) => {
                self.shared_field_initialize(block, initialize)?
            }
            MirInstruction::SharedFieldReplace(replace) => {
                self.shared_field_replace(block, replace)?
            }
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
            MirRvalueKind::Load(place) => Operation::Load {
                address: self.place_address(block, place)?,
                representation: self.place_representation(place)?,
            },
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
