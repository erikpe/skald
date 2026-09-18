//! Concrete definitions, checks and edge protection for one inspection session.
use super::*;
type Edges = Vec<(SelectedBlockId, Vec<SelectedValueId>)>;
type Terminal<'a> = (&'a Instruction, Edges);
pub(super) struct Facts<'a> {
    pub(super) defs: BTreeMap<SelectedValueId, &'a Opcode>,
    pub(super) terms: BTreeMap<SelectedBlockId, Terminal<'a>>,
    pub(super) entry: Option<SelectedBlockId>,
    pub(super) guards: BTreeMap<SelectedBlockId, SelectedBlockId>,
    pub(super) parameters: BTreeMap<SelectedValueId, (SelectedBlockId, usize)>,
    pub(super) sites: BTreeMap<SelectedValueId, SelectedBlockId>,
    pub(super) source_sites: BTreeMap<SelectedValueId, Site>,
    pub(super) representations: BTreeMap<SelectedValueId, crate::backend::selected::Representation>,
}
impl Facts<'_> {
    pub(super) fn failure_reason(
        &self,
        guard: SelectedBlockId,
    ) -> Option<crate::backend::failure::FailureMessage> {
        let guard = self.guards.get(&guard).copied().unwrap_or(guard);
        let (_, edges) = self.terms.get(&guard)?;
        let (mut block, _) = edges.get(1)?;
        let mut seen = BTreeSet::new();
        while seen.insert(block) {
            let (node, edges) = self.terms.get(&block)?;
            match node.opcode {
                Opcode::Failure { reason, .. } => return Some(reason),
                Opcode::Jump if edges.len() == 1 => block = edges[0].0,
                _ => return None,
            }
        }
        None
    }
    pub(super) fn constant(&self, v: SelectedValueId) -> Option<Constant> {
        match self.defs.get(&v)? {
            Opcode::Constant { constant, .. } => Some(*constant),
            _ => None,
        }
    }
    pub(super) fn reaches(
        &self,
        root: SelectedBlockId,
        target: SelectedBlockId,
        removed: Option<(SelectedBlockId, usize)>,
    ) -> bool {
        let mut seen = BTreeSet::new();
        let mut work = vec![root];
        while let Some(block) = work.pop() {
            if !seen.insert(block) {
                continue;
            }
            if block == target {
                return true;
            }
            if let Some((_, edges)) = self.terms.get(&block) {
                for (slot, (to, _)) in edges.iter().enumerate() {
                    if removed != Some((block, slot)) {
                        work.push(*to)
                    }
                }
            }
        }
        false
    }
    pub(super) fn protected(
        &self,
        guard: SelectedBlockId,
        slot: usize,
        target: SelectedBlockId,
    ) -> bool {
        let Some(entry) = self.entry else {
            return false;
        };
        guard != target
            && self.reaches(guard, target, None)
            && !self.reaches(guard, target, Some((guard, slot)))
            && (!self.reaches(entry, target, None)
                || (!self.reaches(entry, target, Some((guard, slot)))
                    && self.reaches(entry, guard, None)))
    }
    pub(super) fn cmp(
        &self,
        v: SelectedValueId,
        p: crate::primitive_comparison::PrimitiveComparisonPredicate,
        source: SelectedValueId,
        c: Constant,
        signed: bool,
    ) -> bool {
        match self.defs.get(&v) {
            Some(Opcode::IntegerCompare {
                predicate,
                left,
                right,
                signed: s,
                ..
            }) => {
                *predicate == p
                    && *s == signed
                    && left.value == source
                    && self.constant(right.value) == Some(c)
            }
            Some(Opcode::FloatCompare {
                predicate,
                left,
                right,
                ..
            }) => *predicate == p && left.value == source && self.constant(right.value) == Some(c),
            _ => false,
        }
    }
    pub(super) fn and(&self, v: SelectedValueId) -> Option<(SelectedValueId, SelectedValueId)> {
        match self.defs.get(&v)? {
            Opcode::Alu {
                operation: crate::backend::lir::BinaryOperation::And,
                left,
                right,
                ..
            } => Some((left.value, right.value)),
            _ => None,
        }
    }
    pub(super) fn check_condition(
        &self,
        condition: SelectedValueId,
        r: &ScalarCheck<SelectedValueId>,
    ) -> bool {
        use crate::primitive_comparison::PrimitiveComparisonPredicate::*;
        match *r {
            ScalarCheck::NonZeroDivisor { ty, divisor } => self.cmp(
                condition,
                NotEqual,
                divisor,
                match ty {
                    ScalarType::I64 => Constant::I64(0),
                    ScalarType::U64 => Constant::U64(0),
                    ScalarType::U8 => Constant::U8(0),
                    _ => return false,
                },
                false,
            ),
            ScalarCheck::ShiftCountBelowWidth { count, width } => {
                matches!(width, 8 | 64)
                    && self.cmp(
                        condition,
                        LessThan,
                        count,
                        Constant::U64(u64::from(width)),
                        false,
                    )
            }
            ScalarCheck::FiniteTruncatedF64InIntegerRange { source, target } => {
                let (lo, hi, p) = match target {
                    ScalarType::I64 => (0xc3e0_0000_0000_0000, 0x43e0_0000_0000_0000, GreaterEqual),
                    ScalarType::U64 => (0xbff0_0000_0000_0000, 0x43f0_0000_0000_0000, GreaterThan),
                    ScalarType::U8 => (0xbff0_0000_0000_0000, 0x4070_0000_0000_0000, GreaterThan),
                    _ => return false,
                };
                self.and(condition).is_some_and(|(a, b)| {
                    self.cmp(a, p, source, Constant::F64(lo), false)
                        && self.cmp(b, LessThan, source, Constant::F64(hi), false)
                })
            }
        }
    }
    pub(super) fn domain(&self, block: SelectedBlockId, d: &Domain) -> bool {
        let source = match d.relation {
            ScalarCheck::NonZeroDivisor { divisor, .. } => divisor,
            ScalarCheck::ShiftCountBelowWidth { count, .. } => count,
            ScalarCheck::FiniteTruncatedF64InIntegerRange { source, .. } => source,
        };
        match d.evidence {
            ScalarDomainEvidence::ExactConstant(v) => {
                v == source
                    && self.constant(v).is_some_and(|c| match (&d.relation, c) {
                        (ScalarCheck::NonZeroDivisor { ty, .. }, c) => match (*ty, c) {
                            (ScalarType::I64, Constant::I64(n)) => n != 0,
                            (ScalarType::U64, Constant::U64(n)) => n != 0,
                            (ScalarType::U8, Constant::U8(n)) => n != 0,
                            _ => false,
                        },
                        (ScalarCheck::ShiftCountBelowWidth { width, .. }, Constant::U64(n)) => {
                            n < u64::from(*width)
                        }
                        (
                            ScalarCheck::FiniteTruncatedF64InIntegerRange { target, .. },
                            Constant::F64(bits),
                        ) => {
                            use skald_binary64::{Binary64, IntegerConversion};
                            let n = Binary64::from_bits(bits);
                            match target {
                                ScalarType::I64 => {
                                    matches!(n.truncating_to_i64(), IntegerConversion::Value(_))
                                }
                                ScalarType::U64 => {
                                    matches!(n.truncating_to_u64(), IntegerConversion::Value(_))
                                }
                                ScalarType::U8 => {
                                    matches!(n.truncating_to_u8(), IntegerConversion::Value(_))
                                }
                                _ => false,
                            }
                        }
                        _ => false,
                    })
            }
            ScalarDomainEvidence::SuccessCheck(guard) => {
                let guard = self.guards.get(&guard).copied().unwrap_or(guard);
                self.terms.get(&guard).is_some_and(|(node, _)| {
                    matches!(&node.opcode, Opcode::CheckBranch {relation, ..} if *relation == d.relation)
                }) && self.protected(guard, 0, block)
            }
        }
    }
    pub(super) fn widened(&self, native: SelectedValueId, source: SelectedValueId) -> bool {
        native == source
            || matches!(self.defs.get(&native),Some(Opcode::Numeric(Numeric::Cell {cell:Cell::ZeroExtend,input,..}))if input.value==source)
    }
    pub(super) fn upper_arm(
        &self,
        block: SelectedBlockId,
        source: SelectedValueId,
        slot: usize,
    ) -> bool {
        use crate::primitive_comparison::PrimitiveComparisonPredicate::LessThan;
        self.terms.iter().any(|(guard, (node, _))| {
            let Opcode::Branch { condition } = node.opcode else {
                return false;
            };
            self.cmp(
                condition.value,
                LessThan,
                source,
                Constant::F64(0x43e0_0000_0000_0000),
                false,
            ) && self.protected(*guard, slot, block)
        })
    }
}
