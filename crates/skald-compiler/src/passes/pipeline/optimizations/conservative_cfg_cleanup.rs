//! Proof-aware ordinary-branch folding and unreachable-block cleanup.

use std::collections::BTreeSet;

use crate::mir::{
    rewrite::{local_cfg_facts_for_definition, MirCallableEdit, MirRewriteError},
    BlockId, MirDefinitionRef, MirInstruction, MirTerminator,
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
    primitive_evaluation::PrimitiveConstant,
    primitive_facts::PrimitiveConstantFacts,
};

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(4);
const NAME: &str = "conservative-cfg-cleanup";
const DESCRIPTION: &str = "Folds ordinary branches and removes unprotected unreachable MIR blocks.";
const CONSTANT_BRANCHES: &str = "folded constant branches";
const SAME_TARGET_BRANCHES: &str = "folded same-target branches";
const REMOVED_BLOCKS: &str = "removed blocks";
const REMOVED_VALUES: &str = "removed value declarations";
const PROTECTED_UNREACHABLE_BLOCKS: &str = "retained protected unreachable blocks";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, MirPassStage::ProofRich, NAME, DESCRIPTION),
    MirPassImplementation::proof_rich(IDENTITY, transform),
);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CleanupCounts {
    constant_branches: usize,
    same_target_branches: usize,
    removed_blocks: usize,
    removed_values: usize,
    protected_unreachable_blocks: usize,
}

impl CleanupCounts {
    const fn has_changes(self) -> bool {
        self.constant_branches != 0 || self.same_target_branches != 0 || self.removed_blocks != 0
    }

    fn add(&mut self, other: Self) {
        self.constant_branches = self
            .constant_branches
            .saturating_add(other.constant_branches);
        self.same_target_branches = self
            .same_target_branches
            .saturating_add(other.same_target_branches);
        self.removed_blocks = self.removed_blocks.saturating_add(other.removed_blocks);
        self.removed_values = self.removed_values.saturating_add(other.removed_values);
        self.protected_unreachable_blocks = self
            .protected_unreachable_blocks
            .saturating_add(other.protected_unreachable_blocks);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BranchRewriteKind {
    Constant,
    SameTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BranchRewrite {
    block: BlockId,
    target: BlockId,
    span: crate::source::Span,
    kind: BranchRewriteKind,
}

fn transform(capability: MirProofPassCapability) -> Result<MirProofPassOutcome, MirPassFailure> {
    let mut processed_callables = 0;
    let mut protected_unreachable_blocks = 0usize;
    let mut has_candidate = false;

    for definition in capability.verified().program().executable_definitions() {
        processed_callables += 1;
        let scan = scan_definition(definition).map_err(MirPassFailure::Rewrite)?;
        if scan.has_candidate {
            has_candidate = true;
            break;
        }
        protected_unreachable_blocks =
            protected_unreachable_blocks.saturating_add(scan.protected_unreachable_blocks);
    }

    if !has_candidate {
        let counts = CleanupCounts {
            protected_unreachable_blocks,
            ..CleanupCounts::default()
        };
        return capability.unchanged_with(pass_data(processed_callables, 0, counts));
    }

    let mut changed_callables = 0;
    let mut counts = CleanupCounts::default();
    let rewritten = capability.rewrite(|_, edit| {
        let callable_counts = cleanup_callable(edit)?;
        if callable_counts.has_changes() {
            changed_callables += 1;
        }
        counts.add(callable_counts);
        Ok(())
    })?;

    rewritten.finish(pass_data(0, changed_callables, counts))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DefinitionScan {
    has_candidate: bool,
    protected_unreachable_blocks: usize,
}

fn scan_definition(definition: MirDefinitionRef<'_>) -> Result<DefinitionScan, MirRewriteError> {
    let cfg = local_cfg_facts_for_definition(definition)?;
    let protected = protected_blocks(&cfg);
    let reachable = cfg.reachable().iter().copied().collect::<BTreeSet<_>>();
    let has_branch = definition
        .body()
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.id))
        .any(|block| {
            branch_rewrite(
                block.id,
                &block.instructions,
                block.terminator.as_ref(),
                protected.contains(&block.id),
            )
            .is_some()
        });
    Ok(DefinitionScan {
        has_candidate: has_branch || !cfg.unreachable().is_empty(),
        protected_unreachable_blocks: cfg.protected_but_entry_unreachable().len(),
    })
}

fn cleanup_callable(edit: &mut MirCallableEdit) -> Result<CleanupCounts, MirRewriteError> {
    let before = edit.local_cfg_facts()?;
    let protected = protected_blocks(&before);
    let reachable = before.reachable().iter().copied().collect::<BTreeSet<_>>();
    let rewrites = before
        .blocks()
        .iter()
        .filter(|facts| reachable.contains(&facts.block()))
        .filter_map(|facts| {
            let block = edit
                .block(facts.block())
                .expect("CFG facts name a validated live edit block");
            branch_rewrite(
                block.id,
                &block.instructions,
                block.terminator.as_ref(),
                protected.contains(&block.id),
            )
        })
        .collect::<Vec<_>>();

    let mut counts = CleanupCounts::default();
    for rewrite in rewrites {
        edit.rewrite_block_terminator(rewrite.block, |_| {
            Some(MirTerminator::Goto {
                target: rewrite.target,
                span: rewrite.span,
            })
        })?;
        match rewrite.kind {
            BranchRewriteKind::Constant => {
                counts.constant_branches = counts.constant_branches.saturating_add(1);
            }
            BranchRewriteKind::SameTarget => {
                counts.same_target_branches = counts.same_target_branches.saturating_add(1);
            }
        }
    }

    let after_branches = edit.local_cfg_facts()?;
    counts.protected_unreachable_blocks = after_branches.protected_but_entry_unreachable().len();
    let removals = after_branches
        .unreachable()
        .iter()
        .map(|block| {
            let values = after_branches
                .block(*block)
                .expect("unreachable block belongs to the same CFG snapshot")
                .defined_values()
                .to_vec();
            (*block, values)
        })
        .collect::<Vec<_>>();
    drop(after_branches);

    for (block, values) in removals {
        for value in values {
            edit.remove_value(value)?;
            counts.removed_values = counts.removed_values.saturating_add(1);
        }
        edit.remove_block(block)?;
        counts.removed_blocks = counts.removed_blocks.saturating_add(1);
    }

    Ok(counts)
}

fn protected_blocks(cfg: &crate::mir::rewrite::MirLocalCfgFacts) -> BTreeSet<BlockId> {
    cfg.protected_roots()
        .iter()
        .map(|root| root.block())
        .collect()
}

fn branch_rewrite(
    block: BlockId,
    instructions: &[MirInstruction],
    terminator: Option<&MirTerminator>,
    protected: bool,
) -> Option<BranchRewrite> {
    if protected {
        return None;
    }

    let (condition, true_target, false_target, span) = match terminator? {
        MirTerminator::Branch {
            condition,
            true_target,
            false_target,
            span,
        } => (*condition, *true_target, *false_target, *span),
        MirTerminator::Return { .. }
        | MirTerminator::ReturnShared { .. }
        | MirTerminator::ReturnOptionalShared { .. }
        | MirTerminator::Panic { .. }
        | MirTerminator::Goto { .. }
        | MirTerminator::ShiftCountCheck { .. }
        | MirTerminator::IntegerDivisorCheck { .. }
        | MirTerminator::PrimitiveCastRangeCheck { .. }
        | MirTerminator::CheckedCast { .. }
        | MirTerminator::SharedCast { .. }
        | MirTerminator::OptionalUnwrap { .. }
        | MirTerminator::OptionalSharedUnwrap { .. }
        | MirTerminator::BeginOptionalView { .. }
        | MirTerminator::BeginOptionalBoxView { .. }
        | MirTerminator::CheckOptionalMutation { .. }
        | MirTerminator::ArrayPositionCheck { .. }
        | MirTerminator::ArrayOperationCheck { .. }
        | MirTerminator::ArrayLoop { .. }
        | MirTerminator::Terminate { .. } => return None,
    };

    let mut facts = PrimitiveConstantFacts::default();
    facts.begin_block();
    for instruction in instructions {
        if let MirInstruction::Assign(assignment) = instruction {
            facts.observe_assignment(assignment);
        }
    }

    if let Some(PrimitiveConstant::Bool(value)) = facts.constant(condition) {
        return Some(BranchRewrite {
            block,
            target: if value { true_target } else { false_target },
            span,
            kind: BranchRewriteKind::Constant,
        });
    }
    (true_target == false_target).then_some(BranchRewrite {
        block,
        target: true_target,
        span,
        kind: BranchRewriteKind::SameTarget,
    })
}

fn pass_data(
    processed_callables: usize,
    changed_callables: usize,
    counts: CleanupCounts,
) -> MirPassData {
    let data = if changed_callables == 0 {
        MirPassData::processed(processed_callables)
    } else {
        MirPassData::changed(changed_callables)
    };
    data.with_measurement(MirPassMeasurement::count(
        CONSTANT_BRANCHES,
        count(counts.constant_branches),
    ))
    .with_measurement(MirPassMeasurement::count(
        SAME_TARGET_BRANCHES,
        count(counts.same_target_branches),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_BLOCKS,
        count(counts.removed_blocks),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_VALUES,
        count(counts.removed_values),
    ))
    .with_measurement(MirPassMeasurement::count(
        PROTECTED_UNREACHABLE_BLOCKS,
        count(counts.protected_unreachable_blocks),
    ))
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "conservative_cfg_cleanup/tests.rs"]
mod tests;
