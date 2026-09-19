//! Checked MIR diamonds become obligations over the exact secured lower value.
use super::{
    context::{scalar_type, Lowerer},
    memory::local,
    LowerError,
};
use crate::{
    backend::{
        lir::{
            Conversion, DivisionResult, Operation, ScalarCheck, ScalarDomainEvidence,
            ShiftDirection,
        },
        plan::PlanError,
        planning::AdmittedProgram,
    },
    mir::{
        BlockId, MirDefinitionRef, MirInstruction, MirIntegerDivisionKind, MirIntegerType,
        MirPrimitiveCastKind, MirRvalueKind, MirShiftDirection, MirTerminator, StorageId, ValueId,
    },
};
use std::collections::BTreeMap;

pub(super) struct Guard {
    pub(super) check: BlockId,
    pub(super) storage: StorageId,
    pub(super) value: ValueId,
}

/// Final MIR verifies an exclusive success block and the exact secured load.
/// Move that load to the check rather than checking memory and reloading a new
/// value: the lower checker deliberately does not trust mutable carrier identity.
pub(super) fn guards(
    definition: MirDefinitionRef<'_>,
) -> Result<BTreeMap<BlockId, Guard>, LowerError> {
    let mut guards = BTreeMap::new();
    for block in &definition.body().blocks {
        let (storage, success) = match block.terminator.as_ref() {
            Some(MirTerminator::IntegerDivisorCheck {
                check,
                success_target,
                ..
            }) => (check.divisor, *success_target),
            Some(MirTerminator::ShiftCountCheck {
                check,
                success_target,
                ..
            }) => (check.count, *success_target),
            Some(MirTerminator::PrimitiveCastRangeCheck {
                check,
                success_target,
                ..
            }) => (check.source, *success_target),
            _ => continue,
        };
        let value = definition.body().blocks[success.index()]
            .instructions
            .iter()
            .find_map(|instruction| {
                let MirInstruction::Assign(assign) = instruction else {
                    return None;
                };
                match &assign.rvalue.kind {
                    MirRvalueKind::Load(place) if local(place).ok() == Some(storage) => {
                        Some(assign.result)
                    }
                    _ => None,
                }
            })
            .ok_or(PlanError::InvalidDomain)?;
        if guards
            .insert(
                success,
                Guard {
                    check: block.id,
                    storage,
                    value,
                },
            )
            .is_some()
        {
            return Err(PlanError::InvalidDomain.into());
        }
    }
    Ok(guards)
}

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn guarded_operation(
        &self,
        block: BlockId,
        kind: &MirRvalueKind,
    ) -> Result<super::scalar::ScalarOperation<'plan>, LowerError> {
        let value = |id: ValueId| self.values[id.index()];
        let evidence = || {
            self.guards
                .get(&block)
                .map(|guard| ScalarDomainEvidence::SuccessCheck(self.blocks[guard.check.index()]))
                .ok_or(LowerError::Plan(PlanError::InvalidDomain))
        };
        Ok(match kind {
            MirRvalueKind::IntegerDivision {
                operation,
                dividend,
                divisor,
            } => Operation::Divide {
                result: match operation.kind {
                    MirIntegerDivisionKind::Quotient => DivisionResult::Quotient,
                    MirIntegerDivisionKind::Remainder => DivisionResult::Remainder,
                },
                dividend: value(*dividend),
                divisor: value(*divisor),
                evidence: evidence()?,
            },
            MirRvalueKind::Shift {
                operation,
                left,
                count,
            } => Operation::Shift {
                direction: match (operation.direction, operation.left) {
                    (MirShiftDirection::Left, _) => ShiftDirection::Left,
                    (MirShiftDirection::Right, MirIntegerType::I64) => {
                        ShiftDirection::ArithmeticRight
                    }
                    (MirShiftDirection::Right, MirIntegerType::U64 | MirIntegerType::U8) => {
                        ShiftDirection::LogicalRight
                    }
                },
                value: value(*left),
                count: value(*count),
                evidence: evidence()?,
            },
            MirRvalueKind::PrimitiveCast { operation, operand } => Operation::Convert {
                conversion: match operation.kind() {
                    MirPrimitiveCastKind::Identity => Conversion::Identity,
                    MirPrimitiveCastKind::IntegerBits => Conversion::IntegerBits,
                    MirPrimitiveCastKind::ToBool => Conversion::ToBoolean,
                    MirPrimitiveCastKind::ToF64 => Conversion::ToFloat,
                    MirPrimitiveCastKind::FromBool => Conversion::FromBoolean,
                    MirPrimitiveCastKind::BitReinterpretation => Conversion::FloatBits,
                    MirPrimitiveCastKind::CheckedF64ToInteger => {
                        return Err(PlanError::InvalidDomain.into())
                    }
                },
                value: value(*operand),
                target: scalar_type(self.admitted, operation.result_type())?,
                evidence: None,
            },
            MirRvalueKind::CheckedF64ToInteger { relation, operand } => Operation::Convert {
                conversion: Conversion::TruncateFloat,
                value: value(*operand),
                target: scalar_type(self.admitted, relation.result_type())?,
                evidence: Some(evidence()?),
            },
            _ => return Err(PlanError::InvalidDomain.into()),
        })
    }
    pub(super) fn numeric_check(
        &mut self,
        block: BlockId,
        success: BlockId,
        failure: BlockId,
    ) -> Result<
        crate::backend::lir::Terminator<
            crate::backend::lir::ValueHandle<'plan>,
            crate::backend::lir::BlockHandle<'plan>,
        >,
        LowerError,
    > {
        let guard = self.guards.get(&success).ok_or(PlanError::InvalidDomain)?;
        if guard.check != block {
            return Err(PlanError::InvalidDomain.into());
        }
        let (storage, value) = (guard.storage, self.values[guard.value.index()]);
        let address = self.address(block, storage)?;
        self.builder.append_into(
            self.active_blocks[block.index()],
            Operation::Load {
                address,
                representation: self.representation(storage)?,
            },
            &[value],
        )?;
        let relation = check_relation(
            self.admitted,
            self.definition.body().blocks[block.index()]
                .terminator
                .as_ref()
                .unwrap(),
            value,
        )?;
        Ok(crate::backend::lir::Terminator::ScalarCheck {
            relation,
            success: self.edge(success),
            failure: self.edge(failure),
        })
    }
}

fn check_relation<'p>(
    admitted: &AdmittedProgram<'_>,
    terminator: &MirTerminator,
    value: crate::backend::lir::ValueHandle<'p>,
) -> Result<ScalarCheck<crate::backend::lir::ValueHandle<'p>>, LowerError> {
    Ok(match terminator {
        MirTerminator::IntegerDivisorCheck { check, .. } => ScalarCheck::NonZeroDivisor {
            ty: scalar_type(admitted, check.operation.operand_type())?,
            divisor: value,
        },
        MirTerminator::ShiftCountCheck { check, .. } => ScalarCheck::ShiftCountBelowWidth {
            count: value,
            width: check.operation.width() as u8,
        },
        MirTerminator::PrimitiveCastRangeCheck { check, .. } => {
            ScalarCheck::FiniteTruncatedF64InIntegerRange {
                source: value,
                target: scalar_type(admitted, check.relation.result_type())?,
            }
        }
        _ => return Err(PlanError::InvalidDomain.into()),
    })
}
