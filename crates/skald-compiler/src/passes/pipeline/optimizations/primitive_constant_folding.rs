//! Block-local folding of exact integer and boolean primitive constants.

use crate::mir::{
    rewrite::{MirCallableEdit, MirRewriteError},
    MirDefinitionRef, MirInstruction,
};

use super::{
    super::{
        execution::{
            MirPassData, MirPassFailure, MirPassMeasurement, MirProofPassCapability,
            MirProofPassOutcome,
        },
        policy::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
        MirPassIdentity, MirPassStage,
    },
    primitive_facts::{PrimitiveConstantFacts, PrimitiveFoldKind},
};

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(2);
const NAME: &str = "primitive-constant-folding";
const DESCRIPTION: &str = "Folds exact block-local primitive MIR constants.";
const FOLDED_UNARY: &str = "folded unary assignments";
const FOLDED_BINARY: &str = "folded binary assignments";
const FOLDED_COMPARISONS: &str = "folded comparison assignments";
const FOLDED_CASTS: &str = "folded cast assignments";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, MirPassStage::ProofRich, NAME, DESCRIPTION),
    MirPassImplementation::proof_rich(IDENTITY, transform),
);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FoldCounts {
    unary: usize,
    binary: usize,
    comparisons: usize,
    casts: usize,
}

impl FoldCounts {
    const fn has_changes(self) -> bool {
        self.unary != 0 || self.binary != 0 || self.comparisons != 0 || self.casts != 0
    }

    fn record(&mut self, kind: PrimitiveFoldKind) {
        let count = match kind {
            PrimitiveFoldKind::Unary => &mut self.unary,
            PrimitiveFoldKind::Binary => &mut self.binary,
            PrimitiveFoldKind::Comparison => &mut self.comparisons,
            PrimitiveFoldKind::Cast => &mut self.casts,
        };
        *count = count.saturating_add(1);
    }

    fn add(&mut self, other: Self) {
        self.unary = self.unary.saturating_add(other.unary);
        self.binary = self.binary.saturating_add(other.binary);
        self.comparisons = self.comparisons.saturating_add(other.comparisons);
        self.casts = self.casts.saturating_add(other.casts);
    }
}

fn transform(capability: MirProofPassCapability) -> Result<MirProofPassOutcome, MirPassFailure> {
    let mut processed_callables = 0;
    let mut has_candidate = false;
    for definition in capability.verified().program().executable_definitions() {
        processed_callables += 1;
        if !has_candidate {
            has_candidate = definition_has_candidate(definition);
        }
    }

    if !has_candidate {
        return capability.unchanged_with(pass_data(processed_callables, 0, FoldCounts::default()));
    }

    let mut changed_callables = 0;
    let mut folds = FoldCounts::default();
    let rewritten = capability.rewrite(|_, edit| {
        let callable_folds = fold_callable(edit)?;
        if callable_folds.has_changes() {
            changed_callables += 1;
            folds.add(callable_folds);
        }
        Ok(())
    })?;

    rewritten.finish(pass_data(0, changed_callables, folds))
}

fn definition_has_candidate(definition: MirDefinitionRef<'_>) -> bool {
    let mut facts = PrimitiveConstantFacts::default();
    for block in &definition.body().blocks {
        facts.begin_block();
        for instruction in &block.instructions {
            let MirInstruction::Assign(assignment) = instruction else {
                continue;
            };
            if facts.observe_assignment(assignment).is_some() {
                return true;
            }
        }
    }
    false
}

fn fold_callable(edit: &mut MirCallableEdit) -> Result<FoldCounts, MirRewriteError> {
    let blocks = edit.block_order().to_vec();
    let mut facts = PrimitiveConstantFacts::default();
    let mut folds = FoldCounts::default();

    for block in blocks {
        facts.begin_block();
        edit.rewrite_block_instructions(block, |instructions| {
            instructions
                .iter()
                .cloned()
                .map(|mut instruction| {
                    let MirInstruction::Assign(assignment) = &mut instruction else {
                        return instruction;
                    };
                    let Some(fold) = facts.observe_assignment(assignment) else {
                        return instruction;
                    };
                    debug_assert_eq!(fold.constant().ty(), assignment.rvalue.ty);
                    assignment.rvalue.kind = fold.constant().into_rvalue_kind();
                    folds.record(fold.kind());
                    instruction
                })
                .collect()
        })?;
    }

    Ok(folds)
}

fn pass_data(
    processed_callables: usize,
    changed_callables: usize,
    folds: FoldCounts,
) -> MirPassData {
    let data = if changed_callables == 0 {
        MirPassData::processed(processed_callables)
    } else {
        MirPassData::changed(changed_callables)
    };
    data.with_measurement(MirPassMeasurement::count(FOLDED_UNARY, count(folds.unary)))
        .with_measurement(MirPassMeasurement::count(
            FOLDED_BINARY,
            count(folds.binary),
        ))
        .with_measurement(MirPassMeasurement::count(
            FOLDED_COMPARISONS,
            count(folds.comparisons),
        ))
        .with_measurement(MirPassMeasurement::count(FOLDED_CASTS, count(folds.casts)))
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "primitive_constant_folding/tests.rs"]
mod tests;
