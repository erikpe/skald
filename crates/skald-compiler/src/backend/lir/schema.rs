//! Local scalar, operand and finite-check schemas. No dominance/effect proofs.

use super::model::{
    BlockHandle, LifetimeDisposition, ObjectHandle, Operation, ScalarCheck, ScalarDomainEvidence,
    ValueHandle,
};
use super::scalar::{integer, integer_width};
use super::{
    BinaryOperation, BuildError, Conversion, DraftChecks, MemoryRepresentation, UnaryOperation,
};
use crate::backend::plan::{ArtifactCategory, LayoutDisposition, PlanError, ScalarType};

#[cfg_attr(not(test), allow(dead_code))]
impl<'p> DraftChecks<'_, 'p> {
    pub(super) fn check_type(&self, ty: ScalarType) -> Result<(), BuildError> {
        let view = self.draft.owner.context();
        match ty {
            ScalarType::CodeAddress(signature) => {
                if !view.profile().capabilities.indirect_calls {
                    return Err(BuildError::Plan(PlanError::UnsupportedCapability));
                }
                view.signature(view.signature_id(signature.index())?)?;
            }
            ScalarType::F64 if !view.profile().capabilities.binary64 => {
                return Err(BuildError::Plan(PlanError::UnsupportedCapability))
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn value(
        &self,
        value: ValueHandle<'p>,
    ) -> Result<(crate::backend::graph::LoweredValueId, ScalarType), BuildError> {
        Ok((value.id(), self.draft.values.get(value)?.ty))
    }
    fn evidence(
        &self,
        evidence: ScalarDomainEvidence<ValueHandle<'p>, BlockHandle<'p>>,
    ) -> Result<ScalarDomainEvidence, BuildError> {
        Ok(match evidence {
            ScalarDomainEvidence::SuccessCheck(block) => {
                self.draft.blocks.get(block)?;
                ScalarDomainEvidence::SuccessCheck(block.id())
            }
            ScalarDomainEvidence::ExactConstant(value) => {
                self.value(value)?;
                ScalarDomainEvidence::ExactConstant(value.id())
            }
        })
    }
    fn memory(&self, representation: MemoryRepresentation) -> Result<(), BuildError> {
        self.check_type(representation.scalar)?;
        representation.check(
            self.draft
                .owner
                .context()
                .profile()
                .data_layout
                .pointer_bytes,
        )
    }
    pub(super) fn normalize(
        &self,
        operation: Operation<ValueHandle<'p>, ObjectHandle<'p>, BlockHandle<'p>>,
    ) -> Result<(Operation, Vec<ScalarType>), BuildError> {
        use Operation::*;
        use ScalarType::*;
        let mut results = Vec::new();
        let normalized = match operation {
            Call(call) => {
                let (call, types) = self.normalize_call(call, false)?;
                results = types;
                Call(call)
            }
            Trace(action) => Trace(self.normalize_trace(action)?),
            Constant(constant) => {
                let ty = constant.scalar_type()?;
                self.check_type(ty)?;
                results.push(ty);
                Constant(constant)
            }
            Unary { operation, value } => {
                let (value, ty) = self.value(value)?;
                let valid = match operation {
                    UnaryOperation::Negate => integer(ty) || ty == F64,
                    UnaryOperation::Complement => integer(ty),
                    UnaryOperation::LogicalNot => ty == Bool,
                };
                if !valid {
                    return Err(BuildError::InvalidScalar);
                }
                results.push(ty);
                Unary { operation, value }
            }
            Binary {
                operation,
                left,
                right,
            } => {
                let (left, ty) = self.value(left)?;
                let (right, other) = self.value(right)?;
                let valid = match operation {
                    BinaryOperation::Add
                    | BinaryOperation::Subtract
                    | BinaryOperation::Multiply => integer(ty) || ty == F64,
                    BinaryOperation::FloatDivide => ty == F64,
                    BinaryOperation::And | BinaryOperation::Or | BinaryOperation::Xor => {
                        integer(ty)
                    }
                };
                if !valid || ty != other {
                    return Err(BuildError::InvalidScalar);
                }
                results.push(ty);
                Binary {
                    operation,
                    left,
                    right,
                }
            }
            Compare {
                predicate,
                left,
                right,
            } => {
                let (left, ty) = self.value(left)?;
                let (right, other) = self.value(right)?;
                if ty != other || (!integer(ty) && ty != F64 && !predicate.is_equality()) {
                    return Err(BuildError::InvalidScalar);
                }
                results.push(Bool);
                Compare {
                    predicate,
                    left,
                    right,
                }
            }
            Divide {
                result,
                dividend,
                divisor,
                evidence,
            } => {
                let (dividend, ty) = self.value(dividend)?;
                let (divisor, other) = self.value(divisor)?;
                if !integer(ty) || ty != other {
                    return Err(BuildError::InvalidScalar);
                }
                results.push(ty);
                Divide {
                    result,
                    dividend,
                    divisor,
                    evidence: self.evidence(evidence)?,
                }
            }
            Shift {
                direction,
                value,
                count,
                evidence,
            } => {
                let (value, ty) = self.value(value)?;
                let (count, count_ty) = self.value(count)?;
                if integer_width(ty).is_none() || count_ty != U64 {
                    return Err(BuildError::InvalidScalar);
                }
                results.push(ty);
                Shift {
                    direction,
                    value,
                    count,
                    evidence: self.evidence(evidence)?,
                }
            }
            Convert {
                conversion,
                value,
                target,
                evidence,
            } => {
                let (value, ty) = self.value(value)?;
                self.check_type(target)?;
                if !conversion.accepts(ty, target) {
                    return Err(BuildError::InvalidScalar);
                }
                if (conversion == Conversion::TruncateFloat) != evidence.is_some() {
                    return Err(BuildError::InvalidEvidence);
                }
                results.push(target);
                Convert {
                    conversion,
                    value,
                    target,
                    evidence: evidence.map(|e| self.evidence(e)).transpose()?,
                }
            }
            SymbolAddress { symbol, ty } => {
                self.check_type(ty)?;
                let view = self.draft.owner.context();
                let declaration = view.artifact(view.artifact_id(symbol)?, symbol.category())?;
                let valid = match ty {
                    DataAddress => matches!(
                        symbol.category(),
                        ArtifactCategory::Data | ArtifactCategory::Tls
                    ),
                    CodeAddress(signature) => declaration.signature == Some(signature),
                    _ => false,
                };
                if !valid {
                    return Err(BuildError::InvalidScalar);
                }
                results.push(ty);
                SymbolAddress { symbol, ty }
            }
            ObjectAddress(object) => {
                if self.draft.objects.get(object)?.layout.disposition
                    != LayoutDisposition::Addressable
                {
                    return Err(BuildError::InvalidObject);
                }
                results.push(DataAddress);
                ObjectAddress(object.id())
            }
            ByteOffset { base, offset } => {
                let (base, ty) = self.value(base)?;
                let (offset, offset_ty) = self.value(offset)?;
                if ty != DataAddress || !matches!(offset_ty, I64 | U64) {
                    return Err(BuildError::InvalidScalar);
                }
                results.push(DataAddress);
                ByteOffset { base, offset }
            }
            ScaledIndex {
                base,
                index,
                stride,
            } => {
                let (base, ty) = self.value(base)?;
                let (index, index_ty) = self.value(index)?;
                if ty != DataAddress || !matches!(index_ty, I64 | U64) {
                    return Err(BuildError::InvalidScalar);
                }
                results.push(DataAddress);
                ScaledIndex {
                    base,
                    index,
                    stride,
                }
            }
            Load {
                address,
                representation,
            } => {
                let (address, ty) = self.value(address)?;
                if ty != DataAddress {
                    return Err(BuildError::InvalidMemory);
                }
                self.memory(representation)?;
                results.push(representation.scalar);
                Load {
                    address,
                    representation,
                }
            }
            Store {
                address,
                value,
                representation,
            } => {
                let (address, ty) = self.value(address)?;
                let (value, value_ty) = self.value(value)?;
                if ty != DataAddress || value_ty != representation.scalar {
                    return Err(BuildError::InvalidMemory);
                }
                self.memory(representation)?;
                Store {
                    address,
                    value,
                    representation,
                }
            }
            Lifetime {
                marker,
                object,
                site,
            } => {
                let record = self.draft.objects.get(object)?;
                if record.layout.disposition != LayoutDisposition::Addressable
                    || !matches!(record.lifetime, LifetimeDisposition::Sites(count) if site < count)
                {
                    return Err(BuildError::InvalidLifetime);
                }
                Lifetime {
                    marker,
                    object: object.id(),
                    site,
                }
            }
        };
        Ok((normalized, results))
    }
    pub(super) fn check_relation(
        &self,
        relation: ScalarCheck<ValueHandle<'p>>,
    ) -> Result<ScalarCheck, BuildError> {
        Ok(match relation {
            ScalarCheck::NonZeroDivisor { ty, divisor } => {
                let (divisor, actual) = self.value(divisor)?;
                if !integer(ty) || ty != actual {
                    return Err(BuildError::InvalidScalar);
                }
                ScalarCheck::NonZeroDivisor { ty, divisor }
            }
            ScalarCheck::ShiftCountBelowWidth { count, width } => {
                let (count, ty) = self.value(count)?;
                if ty != ScalarType::U64 || !matches!(width, 8 | 64) {
                    return Err(BuildError::InvalidScalar);
                }
                ScalarCheck::ShiftCountBelowWidth { count, width }
            }
            ScalarCheck::FiniteTruncatedF64InIntegerRange { source, target } => {
                let (source, ty) = self.value(source)?;
                if ty != ScalarType::F64 || !integer(target) {
                    return Err(BuildError::InvalidScalar);
                }
                ScalarCheck::FiniteTruncatedF64InIntegerRange { source, target }
            }
        })
    }
}
