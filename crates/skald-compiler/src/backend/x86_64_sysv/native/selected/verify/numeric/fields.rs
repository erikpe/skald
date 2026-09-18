//! Closed encoding legality, independent of selection.
use super::super::super::{
    numeric::{Cell, Numeric},
    ValueRef,
};
use crate::backend::{plan::ScalarType, selected::RepresentationKind as Kind};
pub(in crate::backend::x86_64_sysv::native::selected::verify) fn check(
    n: &Numeric,
) -> Result<bool, &'static str> {
    let bits = |v: ValueRef, width| {
        v.representation.kind == Kind::Bits && v.representation.bits() == width
    };
    let float = |v: ValueRef| v.representation.kind == Kind::Float && v.representation.bits() == 64;
    let (valid, flags) = match *n {
        Numeric::Boundary(_) => (true, false),
        Numeric::Cell { cell, input, out } => (
            match cell {
                Cell::Copy => input.representation == out.representation,
                Cell::ZeroExtend => bits(input, 8) && bits(out, 64),
                Cell::Narrow => bits(input, 64) && bits(out, 8),
                Cell::SignedToFloat => bits(input, 64) && float(out),
                Cell::FloatBits => {
                    (bits(input, 64) && float(out)) || (float(input) && bits(out, 64))
                }
            },
            false,
        ),
        Numeric::CheckedTruncate {
            input,
            out,
            source,
            target,
            ..
        } => (
            float(input)
                && float(source)
                && bits(out, 64)
                && matches!(target, ScalarType::I64 | ScalarType::U64 | ScalarType::U8),
            false,
        ),
        Numeric::Dividend { low, high, .. } => (bits(low, 64) && bits(high, 64), false),
        Numeric::Divide {
            low,
            high,
            divisor,
            quotient,
            remainder,
            ..
        } => (
            [low, high, divisor, quotient, remainder]
                .iter()
                .all(|v| bits(*v, 64)),
            true,
        ),
        Numeric::Shift {
            input,
            count,
            out,
            direction,
            ..
        } => (
            input.representation == out.representation
                && bits(count, 8)
                && matches!(input.representation.bits(), 8 | 64)
                && input.representation.kind == Kind::Bits
                && (!matches!(
                    direction,
                    crate::backend::lir::ShiftDirection::ArithmeticRight
                ) || bits(input, 64)),
            true,
        ),
        Numeric::ShiftOne { input, out } => (bits(input, 64) && bits(out, 64), true),
    };
    if valid {
        Ok(flags)
    } else {
        Err("illegal native numeric cell")
    }
}
