//! Bounded instruction-order views over a complete local constant solution.

use std::collections::{BTreeMap, BTreeSet};

use crate::mir::{MirAssignment, MirRvalueKind, ValueId};

use super::{super::primitive_evaluation::PrimitiveConstant, LocalConstantSolution};

/// Constants available after definitions already visited in the current block.
///
/// Algebraic and conservative CFG consumers deliberately retain their
/// reviewed block-local ordering boundary. This view supplies those consumers
/// from the shared convergent solution without evaluating MIR a second time.
pub(in crate::passes::pipeline::optimizations) struct BlockLocalConstantView<'solution> {
    solution: &'solution LocalConstantSolution,
    available: BTreeSet<ValueId>,
    rewritten_literals: BTreeMap<ValueId, PrimitiveConstant>,
}

impl<'solution> BlockLocalConstantView<'solution> {
    pub(in crate::passes::pipeline::optimizations) fn new(
        solution: &'solution LocalConstantSolution,
    ) -> Self {
        Self {
            solution,
            available: BTreeSet::new(),
            rewritten_literals: BTreeMap::new(),
        }
    }

    pub(in crate::passes::pipeline::optimizations) fn begin_block(&mut self) {
        self.available.clear();
        self.rewritten_literals.clear();
    }

    pub(in crate::passes::pipeline::optimizations) fn observe_assignment(
        &mut self,
        assignment: &MirAssignment,
    ) {
        self.available.insert(assignment.result);
        let constant = match assignment.rvalue.kind {
            MirRvalueKind::ConstantI64(value) => Some(PrimitiveConstant::I64(value)),
            MirRvalueKind::ConstantU64(value) => Some(PrimitiveConstant::U64(value)),
            MirRvalueKind::ConstantU8(value) => Some(PrimitiveConstant::U8(value)),
            MirRvalueKind::ConstantBool(value) => Some(PrimitiveConstant::Bool(value)),
            MirRvalueKind::ConstantF64Bits(_)
            | MirRvalueKind::CallableAddress(_)
            | MirRvalueKind::PathCondition(_)
            | MirRvalueKind::Load(_)
            | MirRvalueKind::Unary { .. }
            | MirRvalueKind::Binary { .. }
            | MirRvalueKind::IntegerDivision { .. }
            | MirRvalueKind::Shift { .. }
            | MirRvalueKind::PrimitiveComparison { .. }
            | MirRvalueKind::PrimitiveCast { .. }
            | MirRvalueKind::CheckedF64ToInteger { .. }
            | MirRvalueKind::TypeTest { .. }
            | MirRvalueKind::OptionalPresence { .. }
            | MirRvalueKind::OptionalBoxPresence { .. }
            | MirRvalueKind::ArrayLength { .. } => None,
        };
        if let Some(constant) = constant.filter(|constant| constant.ty() == assignment.rvalue.ty) {
            self.rewritten_literals.insert(assignment.result, constant);
        }
    }

    pub(in crate::passes::pipeline::optimizations) fn constant(
        &self,
        value: ValueId,
    ) -> Option<PrimitiveConstant> {
        self.available
            .contains(&value)
            .then(|| {
                self.rewritten_literals
                    .get(&value)
                    .copied()
                    .or_else(|| self.solution.local_constant(value))
            })
            .flatten()
    }
}
