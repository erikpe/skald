//! Immutable primitive-assignment replacement plans.

use std::collections::BTreeMap;

use crate::{
    identity::CallableId,
    mir::{
        rewrite::{MirCallableEdit, MirRewriteError},
        MirDefinitionRef, MirInstruction, MirProgram, MirRvalueKind,
    },
};

use super::{FoldCounts, PrimitiveFoldKind};
use crate::passes::pipeline::optimizations::local_constant::{
    solve_local_constants, LocalConstantAnalysisError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct PrimitiveFoldCandidate {
    block: crate::mir::BlockId,
    instruction: usize,
    expected: MirInstruction,
    replacement: MirInstruction,
}

/// All primitive replacements derived from one immutable verified program.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct PrimitiveFoldPlan {
    candidates: BTreeMap<CallableId, Vec<PrimitiveFoldCandidate>>,
    processed_callables: usize,
    counts: FoldCounts,
}

impl PrimitiveFoldPlan {
    pub(super) fn prepare(program: &MirProgram) -> Result<Self, LocalConstantAnalysisError> {
        let mut plan = Self::default();
        for definition in program.executable_definitions() {
            plan.processed_callables = plan.processed_callables.saturating_add(1);
            plan.prepare_definition(definition)?;
        }
        Ok(plan)
    }

    fn prepare_definition(
        &mut self,
        definition: MirDefinitionRef<'_>,
    ) -> Result<(), LocalConstantAnalysisError> {
        let solution = solve_local_constants(definition)?;
        for block in &definition.body().blocks {
            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                let MirInstruction::Assign(assignment) = instruction else {
                    continue;
                };
                let Some(kind) = fold_kind(&assignment.rvalue.kind) else {
                    continue;
                };
                let Some(fact) = solution.fact(assignment.result)? else {
                    continue;
                };
                if fact.constant().ty() != assignment.rvalue.ty {
                    return Err(LocalConstantAnalysisError::DerivedTypeMismatch {
                        identity: fact.identity(),
                        expected: assignment.rvalue.ty,
                        actual: fact.constant().ty(),
                    });
                }

                let mut replacement = instruction.clone();
                let MirInstruction::Assign(replacement_assignment) = &mut replacement else {
                    unreachable!("an assignment clone remains an assignment")
                };
                replacement_assignment.rvalue.kind = fact.constant().into_rvalue_kind();
                self.counts.record(kind, fact.provenance());
                self.candidates
                    .entry(definition.callable())
                    .or_default()
                    .push(PrimitiveFoldCandidate {
                        block: block.id,
                        instruction: instruction_index,
                        expected: instruction.clone(),
                        replacement,
                    });
            }
        }
        Ok(())
    }

    pub(super) fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    pub(super) const fn processed_callables(&self) -> usize {
        self.processed_callables
    }

    pub(super) fn changed_callables(&self) -> usize {
        self.candidates.len()
    }

    pub(super) const fn counts(&self) -> FoldCounts {
        self.counts
    }

    pub(super) fn rewrite_callable(
        &self,
        callable: CallableId,
        edit: &mut MirCallableEdit,
    ) -> Result<(), MirRewriteError> {
        let Some(candidates) = self.candidates.get(&callable) else {
            return Ok(());
        };
        for candidate in candidates {
            edit.replace_instruction(
                candidate.block,
                candidate.instruction,
                &candidate.expected,
                candidate.replacement.clone(),
            )?;
        }
        Ok(())
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
