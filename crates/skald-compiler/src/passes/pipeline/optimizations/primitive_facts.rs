//! Instruction-ordered block-local primitive constant facts.

use std::collections::BTreeMap;

use crate::mir::{MirAssignment, MirRvalueKind, ValueId};

use super::primitive_evaluation::{evaluate_rvalue, PrimitiveConstant, PrimitiveEvaluation};

/// The supported operation family responsible for one constant replacement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PrimitiveFoldKind {
    Unary,
    Binary,
    Comparison,
    Cast,
}

/// Exact replacement discovered while extending one block's facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PrimitiveFold {
    kind: PrimitiveFoldKind,
    constant: PrimitiveConstant,
}

impl PrimitiveFold {
    pub(super) const fn kind(self) -> PrimitiveFoldKind {
        self.kind
    }

    pub(super) const fn constant(self) -> PrimitiveConstant {
        self.constant
    }
}

/// Constants established by assignments preceding the current instruction.
///
/// The owner must call [`Self::begin_block`] before scanning each block. Facts
/// are never inferred from storage, calls, proof records, or other blocks.
#[derive(Debug, Default)]
pub(super) struct PrimitiveConstantFacts {
    constants: BTreeMap<ValueId, PrimitiveConstant>,
}

impl PrimitiveConstantFacts {
    pub(super) fn begin_block(&mut self) {
        self.constants.clear();
    }

    /// Observes one assignment after evaluating its rvalue from facts that
    /// were available strictly before this definition.
    pub(super) fn observe_assignment(
        &mut self,
        assignment: &MirAssignment,
    ) -> Option<PrimitiveFold> {
        let evaluation = evaluate_rvalue(&assignment.rvalue.kind, |value| self.constant(value));
        let PrimitiveEvaluation::Constant(constant) = evaluation else {
            return None;
        };

        // Verified MIR already establishes this equality. Keeping the check
        // here prevents malformed facts from becoming optimization authority.
        if constant.ty() != assignment.rvalue.ty {
            return None;
        }
        self.constants.insert(assignment.result, constant);

        fold_kind(&assignment.rvalue.kind).map(|kind| PrimitiveFold { kind, constant })
    }

    pub(super) fn constant(&self, value: ValueId) -> Option<PrimitiveConstant> {
        self.constants.get(&value).copied()
    }
}

const fn fold_kind(kind: &MirRvalueKind) -> Option<PrimitiveFoldKind> {
    match kind {
        MirRvalueKind::Unary { .. } => Some(PrimitiveFoldKind::Unary),
        MirRvalueKind::Binary { .. } => Some(PrimitiveFoldKind::Binary),
        MirRvalueKind::PrimitiveComparison { .. } => Some(PrimitiveFoldKind::Comparison),
        MirRvalueKind::PrimitiveCast { .. } => Some(PrimitiveFoldKind::Cast),
        MirRvalueKind::ConstantI64(_)
        | MirRvalueKind::ConstantU64(_)
        | MirRvalueKind::ConstantU8(_)
        | MirRvalueKind::ConstantF64Bits(_)
        | MirRvalueKind::ConstantBool(_)
        | MirRvalueKind::CallableAddress(_)
        | MirRvalueKind::PathCondition(_)
        | MirRvalueKind::Load(_)
        | MirRvalueKind::IntegerDivision { .. }
        | MirRvalueKind::Shift { .. }
        | MirRvalueKind::CheckedF64ToInteger { .. }
        | MirRvalueKind::TypeTest { .. }
        | MirRvalueKind::OptionalPresence { .. }
        | MirRvalueKind::OptionalBoxPresence { .. }
        | MirRvalueKind::ArrayLength { .. } => None,
    }
}

#[cfg(test)]
#[path = "primitive_facts/tests.rs"]
mod tests;
