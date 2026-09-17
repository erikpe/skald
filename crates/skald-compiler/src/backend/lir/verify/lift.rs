//! Rebind compact stored references through checked owner arenas.
use super::super::*;
use crate::backend::graph::{LoweredBlockId, LoweredObjectId, LoweredValueId};
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct References<'a, 'p> {
    pub draft: &'a CallableDraft<'p>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> References<'_, 'p> {
    pub fn value(&self, id: LoweredValueId) -> Result<ValueHandle<'p>, BuildError> {
        Ok(self.draft.values.handle_id(id)?)
    }
    pub fn block(&self, id: LoweredBlockId) -> Result<BlockHandle<'p>, BuildError> {
        Ok(self.draft.blocks.handle_id(id)?)
    }
    pub fn object(&self, id: LoweredObjectId) -> Result<ObjectHandle<'p>, BuildError> {
        Ok(self.draft.objects.handle_id(id)?)
    }
    pub fn evidence(
        &self,
        e: &ScalarDomainEvidence,
    ) -> Result<ScalarDomainEvidence<ValueHandle<'p>, BlockHandle<'p>>, BuildError> {
        Ok(match e {
            ScalarDomainEvidence::SuccessCheck(b) => {
                ScalarDomainEvidence::SuccessCheck(self.block(*b)?)
            }
            ScalarDomainEvidence::ExactConstant(v) => {
                ScalarDomainEvidence::ExactConstant(self.value(*v)?)
            }
        })
    }
    pub fn call(&self, c: &Call) -> Result<Call<ValueHandle<'p>>, BuildError> {
        Ok(Call {
            target: match c.target {
                CallTarget::Direct(a) => CallTarget::Direct(a),
                CallTarget::Indirect(v) => CallTarget::Indirect(self.value(v)?),
            },
            signature: c.signature,
            arguments: c
                .arguments
                .iter()
                .map(|a| {
                    Ok(CallArgument {
                        role: a.role,
                        value: self.value(a.value)?,
                    })
                })
                .collect::<Result<_, BuildError>>()?,
            attribution: c.attribution.clone(),
        })
    }
    pub fn relation(&self, r: &ScalarCheck) -> Result<ScalarCheck<ValueHandle<'p>>, BuildError> {
        Ok(match r {
            ScalarCheck::NonZeroDivisor { ty, divisor } => ScalarCheck::NonZeroDivisor {
                ty: *ty,
                divisor: self.value(*divisor)?,
            },
            ScalarCheck::ShiftCountBelowWidth { count, width } => {
                ScalarCheck::ShiftCountBelowWidth {
                    count: self.value(*count)?,
                    width: *width,
                }
            }
            ScalarCheck::FiniteTruncatedF64InIntegerRange { source, target } => {
                ScalarCheck::FiniteTruncatedF64InIntegerRange {
                    source: self.value(*source)?,
                    target: *target,
                }
            }
        })
    }
    pub fn operation(
        &self,
        op: &Operation,
    ) -> Result<Operation<ValueHandle<'p>, ObjectHandle<'p>, BlockHandle<'p>>, BuildError> {
        use Operation::*;
        Ok(match op {
            Call(c) => Call(self.call(c)?),
            Constant(c) => Constant(*c),
            Unary { operation, value } => Unary {
                operation: *operation,
                value: self.value(*value)?,
            },
            Binary {
                operation,
                left,
                right,
            } => Binary {
                operation: *operation,
                left: self.value(*left)?,
                right: self.value(*right)?,
            },
            Compare {
                predicate,
                left,
                right,
            } => Compare {
                predicate: *predicate,
                left: self.value(*left)?,
                right: self.value(*right)?,
            },
            Divide {
                result,
                dividend,
                divisor,
                evidence,
            } => Divide {
                result: *result,
                dividend: self.value(*dividend)?,
                divisor: self.value(*divisor)?,
                evidence: self.evidence(evidence)?,
            },
            Shift {
                direction,
                value,
                count,
                evidence,
            } => Shift {
                direction: *direction,
                value: self.value(*value)?,
                count: self.value(*count)?,
                evidence: self.evidence(evidence)?,
            },
            Convert {
                conversion,
                value,
                target,
                evidence,
            } => Convert {
                conversion: *conversion,
                value: self.value(*value)?,
                target: *target,
                evidence: evidence.as_ref().map(|e| self.evidence(e)).transpose()?,
            },
            SymbolAddress { symbol, ty } => SymbolAddress {
                symbol: *symbol,
                ty: *ty,
            },
            ObjectAddress(o) => ObjectAddress(self.object(*o)?),
            ByteOffset { base, offset } => ByteOffset {
                base: self.value(*base)?,
                offset: self.value(*offset)?,
            },
            ScaledIndex {
                base,
                index,
                stride,
            } => ScaledIndex {
                base: self.value(*base)?,
                index: self.value(*index)?,
                stride: *stride,
            },
            Load {
                address,
                representation,
            } => Load {
                address: self.value(*address)?,
                representation: *representation,
            },
            Store {
                address,
                value,
                representation,
            } => Store {
                address: self.value(*address)?,
                value: self.value(*value)?,
                representation: *representation,
            },
            Lifetime {
                marker,
                object,
                site,
            } => Lifetime {
                marker: *marker,
                object: self.object(*object)?,
                site: *site,
            },
            Trace(a) => Trace(match a {
                TraceAction::PushFrame { record } => TraceAction::PushFrame {
                    record: self.object(*record)?,
                },
                TraceAction::PopFrame { record } => TraceAction::PopFrame {
                    record: self.object(*record)?,
                },
                TraceAction::ReplaceLocation {
                    record,
                    location,
                    site,
                } => TraceAction::ReplaceLocation {
                    record: self.object(*record)?,
                    location: *location,
                    site: match site {
                        TraceSite::Instruction { block, ordinal } => TraceSite::Instruction {
                            block: self.block(*block)?,
                            ordinal: *ordinal,
                        },
                        TraceSite::Terminator(b) => TraceSite::Terminator(self.block(*b)?),
                    },
                },
            }),
        })
    }
}
