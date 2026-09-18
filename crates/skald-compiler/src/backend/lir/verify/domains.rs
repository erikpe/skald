//! Finite secured-value obligations, including check-rooted unreachable regions.
use super::super::*;
use crate::backend::graph::{LoweredBlockId, LoweredValueId};
use crate::backend::plan::ScalarType;
use std::collections::BTreeMap;
pub(super) struct GuardPaths {
    edges: Vec<Vec<usize>>,
    entry: usize,
    cache: BTreeMap<(usize, Option<usize>), Vec<bool>>,
}
impl GuardPaths {
    pub fn new(session: &crate::backend::graph::GraphSession<'_, CallableDraft<'_>>) -> Self {
        let draft = session.owner();
        Self {
            edges: (0..draft.blocks.iter().len())
                .map(|block| session.successors(block).unwrap().to_vec())
                .collect(),
            entry: draft.entry.unwrap().index(),
            cache: BTreeMap::new(),
        }
    }
    fn reaches(&mut self, root: usize, removed: Option<usize>, target: usize) -> bool {
        let edges = &self.edges;
        self.cache.entry((root, removed)).or_insert_with(|| {
            let mut seen = vec![false; edges.len()];
            let mut work = vec![root];
            while let Some(b) = work.pop() {
                if std::mem::replace(&mut seen[b], true) {
                    continue;
                }
                for (slot, t) in edges[b].iter().enumerate() {
                    if removed != Some(b) || slot != 0 {
                        work.push(*t);
                    }
                }
            }
            seen
        })[target]
    }
    pub fn evidence(
        &mut self,
        draft: &CallableDraft<'_>,
        block: LoweredBlockId,
        relation: ScalarCheck,
        evidence: &ScalarDomainEvidence,
    ) -> bool {
        let operand = match relation {
            ScalarCheck::NonZeroDivisor { divisor, .. } => divisor,
            ScalarCheck::ShiftCountBelowWidth { count, .. } => count,
            ScalarCheck::FiniteTruncatedF64InIntegerRange { source, .. } => source,
        };
        match evidence {
            ScalarDomainEvidence::ExactConstant(value) => {
                *value == operand
                    && constant(draft, *value).is_some_and(|c| satisfies(c, &relation))
            }
            ScalarDomainEvidence::SuccessCheck(check) => {
                let Some(Terminator::ScalarCheck {
                    relation: actual, ..
                }) = draft
                    .blocks
                    .get_id(*check)
                    .ok()
                    .and_then(|b| b.terminator.as_ref())
                else {
                    return false;
                };
                if *actual != relation || *check == block {
                    return false;
                }
                let root = check.index();
                let target = block.index();
                self.reaches(root, None, target)
                    && !self.reaches(root, Some(root), target)
                    && (!self.reaches(self.entry, None, target)
                        || (!self.reaches(self.entry, Some(root), target)
                            && self.reaches(self.entry, None, root)))
            }
        }
    }
}
pub(super) fn constant(draft: &CallableDraft<'_>, value: LoweredValueId) -> Option<Constant> {
    let Definition::InstructionResult {
        instruction,
        ordinal: 0,
    } = draft.values.get_id(value).ok()?.definition?
    else {
        return None;
    };
    match draft
        .blocks
        .get_id(instruction.block)
        .ok()?
        .instructions
        .get(instruction.ordinal)?
        .operation
    {
        Operation::Constant(c) => Some(c),
        _ => None,
    }
}
fn satisfies(constant: Constant, relation: &ScalarCheck) -> bool {
    match relation {
        ScalarCheck::NonZeroDivisor { ty, .. } => {
            constant.scalar_type().ok() == Some(*ty)
                && match constant {
                    Constant::I64(v) => v != 0,
                    Constant::U64(v) => v != 0,
                    Constant::U8(v) => v != 0,
                    _ => false,
                }
        }
        ScalarCheck::ShiftCountBelowWidth { width, .. } => {
            matches!(constant,Constant::U64(v)if v<*width as u64)
        }
        ScalarCheck::FiniteTruncatedF64InIntegerRange { target, .. } => {
            let Constant::F64(bits) = constant else {
                return false;
            };
            use skald_binary64::{Binary64, IntegerConversion};
            let value = Binary64::from_bits(bits);
            match target {
                ScalarType::I64 => matches!(value.truncating_to_i64(), IntegerConversion::Value(_)),
                ScalarType::U64 => matches!(value.truncating_to_u64(), IntegerConversion::Value(_)),
                ScalarType::U8 => matches!(value.truncating_to_u8(), IntegerConversion::Value(_)),
                _ => false,
            }
        }
    }
}
pub(super) fn obligation(
    op: &Operation,
    draft: &CallableDraft<'_>,
) -> Option<(ScalarCheck, ScalarDomainEvidence)> {
    match op {
        Operation::Divide {
            dividend,
            divisor,
            evidence,
            ..
        } => Some((
            ScalarCheck::NonZeroDivisor {
                ty: draft.values.get_id(*dividend).ok()?.ty,
                divisor: *divisor,
            },
            evidence.clone(),
        )),
        Operation::Shift {
            value,
            count,
            evidence,
            ..
        } => Some((
            ScalarCheck::ShiftCountBelowWidth {
                count: *count,
                width: super::super::scalar::integer_width(draft.values.get_id(*value).ok()?.ty)?,
            },
            evidence.clone(),
        )),
        Operation::Convert {
            conversion: Conversion::TruncateFloat,
            value,
            target,
            evidence: Some(evidence),
        } => Some((
            ScalarCheck::FiniteTruncatedF64InIntegerRange {
                source: *value,
                target: *target,
            },
            evidence.clone(),
        )),
        _ => None,
    }
}
