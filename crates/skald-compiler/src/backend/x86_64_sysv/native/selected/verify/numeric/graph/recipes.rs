//! Check semantic association markers against concrete definitions and edges.
use super::*;
use crate::backend::lir::{BinaryOperation as Alu, Conversion, DivisionResult};
use crate::primitive_comparison::PrimitiveComparisonPredicate as Predicate;
impl Facts<'_> {
    fn cell(&self, value: SelectedValueId, kind: Cell) -> Option<SelectedValueId> {
        match self.defs.get(&value)? {
            Opcode::Numeric(Numeric::Cell { cell, input, .. }) if *cell == kind => {
                Some(input.value)
            }
            _ => None,
        }
    }
    fn alu(&self, value: SelectedValueId, kind: Alu) -> Option<(SelectedValueId, SelectedValueId)> {
        match self.defs.get(&value)? {
            Opcode::Alu {
                operation,
                left,
                right,
                ..
            } if *operation == kind => Some((left.value, right.value)),
            _ => None,
        }
    }
    fn incoming(&self, value: SelectedValueId) -> Option<Vec<(SelectedBlockId, SelectedValueId)>> {
        let (join, ordinal) = self.parameters.get(&value)?;
        let mut result = vec![];
        for (block, (node, edges)) in &self.terms {
            for (to, args) in edges {
                if to == join {
                    if !matches!(node.opcode, Opcode::Jump) {
                        return None;
                    }
                    result.push((*block, *args.get(*ordinal)?));
                }
            }
        }
        Some(result)
    }
    fn integer_upper_arm(
        &self,
        block: SelectedBlockId,
        source: SelectedValueId,
        slot: usize,
    ) -> bool {
        self.terms.iter().any(|(guard, (node, _))| {
            let Opcode::Branch { condition } = node.opcode else {
                return false;
            };
            self.cmp(
                condition.value,
                Predicate::LessThan,
                source,
                Constant::U64(1 << 63),
                false,
            ) && self.protected(*guard, slot, block)
        })
    }
    fn checked(&self, value: SelectedValueId, source: SelectedValueId, target: ScalarType) -> bool {
        matches!(self.defs.get(&value),Some(Opcode::Numeric(Numeric::CheckedTruncate {source:s,target:t,..}))if s.value==source&&*t==target)
    }
    pub(super) fn recipe(
        &self,
        recipe: &Recipe,
        source_site: super::super::super::super::Site,
    ) -> Result<(), &'static str> {
        let valid = match *recipe {
            Recipe::Conversion {
                conversion,
                from,
                to,
                source,
                out,
            } => {
                if !conversion.accepts(from, to)
                    || crate::backend::selected::Representation::from_scalar(from, 64)
                        != Some(source.representation)
                    || crate::backend::selected::Representation::from_scalar(to, 64)
                        != Some(out.representation)
                {
                    return Err("invalid native conversion association");
                }
                self.conversion(conversion, from, to, source.value, out.value)
            }
            Recipe::Division {
                ty,
                result,
                dividend,
                divisor,
                out,
            } => self.division(ty, result, dividend, divisor, out, source_site),
        };
        if valid {
            Ok(())
        } else {
            Err("native numeric correction or result association mismatch")
        }
    }
    fn conversion(
        &self,
        conversion: Conversion,
        from: ScalarType,
        to: ScalarType,
        source: SelectedValueId,
        out: SelectedValueId,
    ) -> bool {
        match conversion {
            Conversion::Identity => self.cell(out, Cell::Copy) == Some(source),
            Conversion::IntegerBits | Conversion::FromBoolean => {
                let width = |ty| {
                    if matches!(ty, ScalarType::U8 | ScalarType::Bool) {
                        8
                    } else {
                        64
                    }
                };
                let cell = match (width(from), width(to)) {
                    (8, 64) => Cell::ZeroExtend,
                    (64, 8) => Cell::Narrow,
                    _ => Cell::Copy,
                };
                self.cell(out, cell) == Some(source)
            }
            Conversion::FloatBits => self.cell(out, Cell::FloatBits) == Some(source),
            Conversion::ToBoolean => {
                let zero = match from {
                    ScalarType::F64 => Constant::F64(0),
                    ScalarType::I64 => Constant::I64(0),
                    ScalarType::U8 => Constant::U8(0),
                    _ => Constant::U64(0),
                };
                self.cell(out, Cell::Copy).is_some_and(|condition| {
                    self.cmp(condition, Predicate::NotEqual, source, zero, false)
                })
            }
            Conversion::ToFloat if from != ScalarType::U64 => {
                self.cell(out, Cell::SignedToFloat).is_some_and(|input| {
                    if from == ScalarType::I64 {
                        input == source
                    } else {
                        self.cell(input, Cell::ZeroExtend) == Some(source)
                    }
                })
            }
            Conversion::ToFloat => self.unsigned_to_float(source, out),
            Conversion::TruncateFloat if to == ScalarType::I64 => self.checked(out, source, to),
            Conversion::TruncateFloat if to == ScalarType::U8 => self
                .cell(out, Cell::Narrow)
                .is_some_and(|input| self.checked(input, source, to)),
            Conversion::TruncateFloat => self.float_to_unsigned(source, out),
            Conversion::PointerBits => false,
        }
    }
    fn unsigned_to_float(&self, source: SelectedValueId, out: SelectedValueId) -> bool {
        let Some(arms) = self.incoming(out) else {
            return false;
        };
        arms.len() == 2 && arms.iter().all(|(block, value)| {
            if self.integer_upper_arm(*block, source, 0) {
                return self.cell(*value, Cell::SignedToFloat) == Some(source);
            }
            if !self.integer_upper_arm(*block, source, 1) { return false; }
            let Some((converted, two)) = self.alu(*value, Alu::Multiply) else { return false; };
            if self.constant(two) != Some(Constant::F64(0x4000_0000_0000_0000)) { return false; }
            let Some(rounded) = self.cell(converted, Cell::SignedToFloat) else { return false; };
            let Some((half, lowbit)) = self.alu(rounded, Alu::Or) else { return false; };
            matches!(self.defs.get(&half), Some(Opcode::Numeric(Numeric::ShiftOne {input, ..})) if input.value == source)
                && self.alu(lowbit, Alu::And).is_some_and(|(input, one)|
                    input == source && self.constant(one) == Some(Constant::U64(1)))
        })
    }
    fn float_to_unsigned(&self, source: SelectedValueId, out: SelectedValueId) -> bool {
        let Some(arms) = self.incoming(out) else {
            return false;
        };
        arms.len() == 2
            && arms.iter().all(|(block, value)| {
                if self.upper_arm(*block, source, 0) {
                    return self.checked(*value, source, ScalarType::U64);
                }
                if !self.upper_arm(*block, source, 1) {
                    return false;
                }
                self.alu(*value, Alu::Xor).is_some_and(|(truncated, high)| {
                    self.checked(truncated, source, ScalarType::U64)
                        && self.constant(high) == Some(Constant::U64(1 << 63))
                })
            })
    }
    fn floor_condition(
        &self,
        condition: SelectedValueId,
        remainder: SelectedValueId,
        dividend: SelectedValueId,
        divisor: SelectedValueId,
    ) -> bool {
        let Some((nonzero, different)) = self.and(condition) else {
            return false;
        };
        self.cmp(
            nonzero,
            Predicate::NotEqual,
            remainder,
            Constant::I64(0),
            true,
        ) && matches!(self.defs.get(&different), Some(Opcode::IntegerCompare {
                predicate: Predicate::LessThan, signed: true, left, right, ..
            }) if self.constant(right.value) == Some(Constant::I64(0))
                && self.alu(left.value, Alu::Xor) == Some((dividend, divisor)))
    }
    fn division(
        &self,
        ty: ScalarType,
        result: DivisionResult,
        dividend: super::super::super::super::ValueRef,
        divisor: super::super::super::super::ValueRef,
        out: super::super::super::super::ValueRef,
        source_site: super::super::super::super::Site,
    ) -> bool {
        if !matches!(ty, ScalarType::I64 | ScalarType::U64 | ScalarType::U8)
            || [dividend, divisor, out].iter().any(|v| {
                crate::backend::selected::Representation::from_scalar(ty, 64)
                    != Some(v.representation)
            })
        {
            return false;
        }
        let Some(arms) = self.incoming(out.value) else {
            return false;
        };
        let signed = ty == ScalarType::I64;
        if arms.len() != if signed { 3 } else { 1 } {
            return false;
        }
        let pair = self.defs.values().find_map(|op| match op {
            Opcode::Numeric(Numeric::Divide {
                signed: s,
                low,
                divisor: b,
                quotient,
                remainder,
                domain,
                overflow,
                ..
            }) if *s == signed
                && self.source_sites.get(&quotient.value) == Some(&source_site)
                && self.widened(low.value, dividend.value)
                && self.widened(b.value, divisor.value)
                && domain.relation
                    == (ScalarCheck::NonZeroDivisor {
                        ty,
                        divisor: divisor.value,
                    }) =>
            {
                if let crate::backend::lir::ScalarDomainEvidence::SuccessCheck(guard) =
                    domain.evidence
                {
                    let expected = if result == DivisionResult::Quotient {
                        crate::backend::failure::FailureMessage::IntegerDivisionByZero
                    } else {
                        crate::backend::failure::FailureMessage::IntegerRemainderByZero
                    };
                    if self.failure_reason(guard) != Some(expected) {
                        return None;
                    }
                }
                Some((quotient.value, remainder.value, *overflow))
            }
            _ => None,
        });
        let Some((q, r, overflow)) = pair else {
            return false;
        };
        let raw = if result == DivisionResult::Quotient {
            q
        } else {
            r
        };
        if !signed {
            return arms.iter().all(|(_, v)| {
                if ty == ScalarType::U8 {
                    self.cell(*v, Cell::Narrow) == Some(raw)
                } else {
                    *v == raw
                }
            });
        }
        let Some(overflow) = overflow else {
            return false;
        };
        let correction = self.terms.iter().find_map(|(block, (node, _))| {
            let Opcode::Branch { condition } = node.opcode else {
                return None;
            };
            self.floor_condition(condition.value, r, dividend.value, divisor.value)
                .then_some(*block)
        });
        let Some(correction) = correction else {
            return false;
        };
        arms.iter().all(|(block, value)| {
            if self.protected(overflow, 0, *block) {
                self.constant(*value)
                    == Some(if result == DivisionResult::Quotient {
                        Constant::I64(i64::MIN)
                    } else {
                        Constant::I64(0)
                    })
            } else if self.protected(correction, 1, *block) {
                *value == raw
            } else if self.protected(correction, 0, *block) {
                if result == DivisionResult::Quotient {
                    self.alu(*value, Alu::Subtract).is_some_and(|(input, one)| {
                        input == q && self.constant(one) == Some(Constant::I64(1))
                    })
                } else {
                    self.alu(*value, Alu::Add) == Some((r, divisor.value))
                }
            } else {
                false
            }
        })
    }
}
