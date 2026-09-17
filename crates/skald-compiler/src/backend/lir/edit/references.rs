//! Exhaustive stored-reference traversal, including evidence and trace sites.
use super::super::*;
use crate::backend::graph::{
    EditError, LoweredBlockId as B, LoweredObjectId as O, LoweredValueId as V,
};

pub(super) fn call(
    c: &mut Call,
    v: &mut impl FnMut(V) -> Result<V, EditError>,
) -> Result<(), EditError> {
    if let CallTarget::Indirect(id) = &mut c.target {
        *id = v(*id)?;
    }
    for argument in &mut c.arguments {
        argument.value = v(argument.value)?;
    }
    Ok(())
}
fn evidence(
    e: &mut ScalarDomainEvidence,
    v: &mut impl FnMut(V) -> Result<V, EditError>,
    b: &mut impl FnMut(B) -> Result<B, EditError>,
) -> Result<(), EditError> {
    match e {
        ScalarDomainEvidence::SuccessCheck(id) => *id = b(*id)?,
        ScalarDomainEvidence::ExactConstant(id) => *id = v(*id)?,
    }
    Ok(())
}
pub(super) fn site(
    s: &mut TraceSite,
    b: &mut impl FnMut(B) -> Result<B, EditError>,
) -> Result<(), EditError> {
    match s {
        TraceSite::Instruction { block, .. } | TraceSite::Terminator(block) => *block = b(*block)?,
    }
    Ok(())
}
pub(super) fn operation(
    op: &mut Operation,
    v: &mut impl FnMut(V) -> Result<V, EditError>,
    o: &mut impl FnMut(O) -> Result<O, EditError>,
    b: &mut impl FnMut(B) -> Result<B, EditError>,
    include_evidence: bool,
) -> Result<(), EditError> {
    use Operation::*;
    match op {
        Call(c) => call(c, v)?,
        Trace(action) => match action {
            TraceAction::PushFrame { record } | TraceAction::PopFrame { record } => {
                *record = o(*record)?
            }
            TraceAction::ReplaceLocation {
                record, site: s, ..
            } => {
                *record = o(*record)?;
                site(s, b)?;
            }
        },
        Constant(_) | SymbolAddress { .. } => {}
        Unary { value, .. } | Load { address: value, .. } => *value = v(*value)?,
        Binary { left, right, .. } | Compare { left, right, .. } => {
            *left = v(*left)?;
            *right = v(*right)?;
        }
        Divide {
            dividend,
            divisor,
            evidence: e,
            ..
        } => {
            *dividend = v(*dividend)?;
            *divisor = v(*divisor)?;
            if include_evidence {
                evidence(e, v, b)?;
            }
        }
        Shift {
            value,
            count,
            evidence: e,
            ..
        } => {
            *value = v(*value)?;
            *count = v(*count)?;
            if include_evidence {
                evidence(e, v, b)?;
            }
        }
        Convert {
            value, evidence: e, ..
        } => {
            *value = v(*value)?;
            if include_evidence {
                if let Some(e) = e {
                    evidence(e, v, b)?;
                }
            }
        }
        ObjectAddress(id) | Lifetime { object: id, .. } => *id = o(*id)?,
        ByteOffset { base, offset }
        | ScaledIndex {
            base,
            index: offset,
            ..
        } => {
            *base = v(*base)?;
            *offset = v(*offset)?;
        }
        Store { address, value, .. } => {
            *address = v(*address)?;
            *value = v(*value)?;
        }
    }
    Ok(())
}
pub(super) fn edge(
    e: &mut Edge,
    v: &mut impl FnMut(V) -> Result<V, EditError>,
    b: &mut impl FnMut(B) -> Result<B, EditError>,
) -> Result<(), EditError> {
    e.target = b(e.target)?;
    for id in &mut e.arguments {
        *id = v(*id)?;
    }
    Ok(())
}
pub(super) fn terminal(
    t: &mut Terminator,
    v: &mut impl FnMut(V) -> Result<V, EditError>,
    b: &mut impl FnMut(B) -> Result<B, EditError>,
    include_edges: bool,
) -> Result<(), EditError> {
    match t {
        Terminator::Jump(e) => {
            if include_edges {
                edge(e, v, b)?;
            }
        }
        Terminator::Branch {
            condition,
            true_edge,
            false_edge,
        } => {
            *condition = v(*condition)?;
            if include_edges {
                edge(true_edge, v, b)?;
                edge(false_edge, v, b)?;
            }
        }
        Terminator::ScalarCheck {
            relation,
            success,
            failure,
        } => {
            match relation {
                ScalarCheck::NonZeroDivisor { divisor, .. } => *divisor = v(*divisor)?,
                ScalarCheck::ShiftCountBelowWidth { count, .. } => *count = v(*count)?,
                ScalarCheck::FiniteTruncatedF64InIntegerRange { source, .. } => {
                    *source = v(*source)?
                }
            }
            if include_edges {
                edge(success, v, b)?;
                edge(failure, v, b)?;
            }
        }
        Terminator::Return(ids) => {
            for id in ids {
                *id = v(*id)?;
            }
        }
        Terminator::ReportFailure { call: c, .. } | Terminator::NonReturningCall(c) => call(c, v)?,
        Terminator::HardTrap => {}
    }
    Ok(())
}
