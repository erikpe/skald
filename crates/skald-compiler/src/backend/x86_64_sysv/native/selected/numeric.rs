//! Concrete numeric cells. Correction control flow belongs to selection.
use super::ValueRef;
use crate::backend::{
    graph::{SelectedBlockId, SelectedValueId},
    lir::{Conversion, DivisionResult, ScalarCheck, ScalarDomainEvidence, ShiftDirection},
    plan::ScalarType,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum Cell {
    Copy,
    ZeroExtend,
    Narrow,
    SignedToFloat,
    FloatBits,
}
#[derive(Clone, Debug)]
pub(in crate::backend) struct Domain {
    pub relation: ScalarCheck<SelectedValueId>,
    pub evidence: ScalarDomainEvidence<SelectedValueId, SelectedBlockId>,
}
/// Zero-code semantic association, independently checked against the actual graph.
#[derive(Clone, Debug)]
pub(in crate::backend) enum Recipe {
    Division {
        ty: ScalarType,
        result: DivisionResult,
        dividend: ValueRef,
        divisor: ValueRef,
        out: ValueRef,
    },
    Conversion {
        conversion: Conversion,
        from: ScalarType,
        to: ScalarType,
        source: ValueRef,
        out: ValueRef,
    },
}
impl Recipe {
    pub fn values_mut(&mut self) -> Vec<&mut ValueRef> {
        match self {
            Self::Division {
                dividend,
                divisor,
                out,
                ..
            } => vec![dividend, divisor, out],
            Self::Conversion { source, out, .. } => vec![source, out],
        }
    }
}
#[derive(Clone, Debug)]
pub(in crate::backend) enum Numeric {
    Boundary(Recipe),
    Cell {
        cell: Cell,
        input: ValueRef,
        out: ValueRef,
    },
    Dividend {
        signed: bool,
        low: ValueRef,
        high: ValueRef,
    },
    Divide {
        signed: bool,
        low: ValueRef,
        high: ValueRef,
        divisor: ValueRef,
        quotient: ValueRef,
        remainder: ValueRef,
        domain: Box<Domain>,
        overflow: Option<SelectedBlockId>,
    },
    Shift {
        direction: ShiftDirection,
        input: ValueRef,
        count: ValueRef,
        out: ValueRef,
        domain: Box<Domain>,
    },
    ShiftOne {
        input: ValueRef,
        out: ValueRef,
    },
    // The check is on the original source even in an unsigned correction arm.
    CheckedTruncate {
        input: ValueRef,
        out: ValueRef,
        source: ValueRef,
        target: ScalarType,
        domain: Box<Domain>,
    },
}
impl Numeric {
    pub fn operands(&self) -> Vec<(ValueRef, bool)> {
        match *self {
            Self::Boundary(_) => vec![],
            Self::Cell { input, out, .. }
            | Self::ShiftOne { input, out }
            | Self::CheckedTruncate { input, out, .. } => vec![(input, false), (out, true)],
            Self::Dividend { low, high, .. } => vec![(low, false), (high, true)],
            Self::Divide {
                low,
                high,
                divisor,
                quotient,
                remainder,
                ..
            } => vec![
                (low, false),
                (high, false),
                (divisor, false),
                (quotient, true),
                (remainder, true),
            ],
            Self::Shift {
                input, count, out, ..
            } => vec![(input, false), (count, false), (out, true)],
        }
    }
    pub fn operands_mut(&mut self) -> Vec<(&mut ValueRef, bool)> {
        match self {
            Self::Boundary(_) => vec![],
            Self::Cell { input, out, .. }
            | Self::ShiftOne { input, out }
            | Self::CheckedTruncate { input, out, .. } => vec![(input, false), (out, true)],
            Self::Dividend { low, high, .. } => vec![(low, false), (high, true)],
            Self::Divide {
                low,
                high,
                divisor,
                quotient,
                remainder,
                ..
            } => vec![
                (low, false),
                (high, false),
                (divisor, false),
                (quotient, true),
                (remainder, true),
            ],
            Self::Shift {
                input, count, out, ..
            } => vec![(input, false), (count, false), (out, true)],
        }
    }
    pub fn domain(&self) -> Option<&Domain> {
        match self {
            Self::Divide { domain, .. }
            | Self::Shift { domain, .. }
            | Self::CheckedTruncate { domain, .. } => Some(domain),
            _ => None,
        }
    }
}

pub(super) fn remap_metadata(
    opcode: &mut super::Opcode,
    maps: &crate::backend::selected::SelectedRemap<'_>,
) -> Result<(), crate::backend::graph::EditError> {
    let relation =
        |r: &mut ScalarCheck<SelectedValueId>| -> Result<(), crate::backend::graph::EditError> {
            let v = match r {
                ScalarCheck::NonZeroDivisor { divisor, .. } => divisor,
                ScalarCheck::ShiftCountBelowWidth { count, .. } => count,
                ScalarCheck::FiniteTruncatedF64InIntegerRange { source, .. } => source,
            };
            *v = maps.values.get(*v)?;
            Ok(())
        };
    match opcode {
        super::Opcode::CheckBranch { relation: r, .. } => relation(r)?,
        super::Opcode::Numeric(n) => {
            if let Numeric::Boundary(recipe) = n {
                for v in recipe.values_mut() {
                    v.value = maps.values.get(v.value)?;
                }
            }
            if let Numeric::CheckedTruncate { source, .. } = n {
                source.value = maps.values.get(source.value)?;
            }
            if let Numeric::Divide {
                overflow: Some(block),
                ..
            } = n
            {
                *block = maps.blocks.get(*block)?;
            }
            let domain = match n {
                Numeric::Divide { domain, .. }
                | Numeric::Shift { domain, .. }
                | Numeric::CheckedTruncate { domain, .. } => Some(domain),
                _ => None,
            };
            if let Some(domain) = domain {
                relation(&mut domain.relation)?;
                match &mut domain.evidence {
                    ScalarDomainEvidence::ExactConstant(v) => *v = maps.values.get(*v)?,
                    ScalarDomainEvidence::SuccessCheck(b) => *b = maps.blocks.get(*b)?,
                }
            }
        }
        _ => {}
    }
    Ok(())
}
