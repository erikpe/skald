//! Conservative elimination of unused, non-failing scalar definitions.

use std::collections::BTreeSet;

use crate::mir::{
    rewrite::{
        value_use_census_for_definition, MirCallableEdit, MirLocalIdentitySite, MirValueUseCensus,
    },
    MirAssignment, MirInstruction, MirRvalueKind, ValueId,
};

use super::super::{
    execution::{
        MirPassData, MirPassFailure, MirPassMeasurement, MirProofPassCapability,
        MirProofPassOutcome,
    },
    policy::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
    MirPassIdentity, MirPassStage,
};

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(0);
const NAME: &str = "dead-pure-definition-elimination";
const DESCRIPTION: &str = "Removes unused non-failing scalar MIR definitions.";
const REMOVED_ASSIGNMENTS: &str = "removed assignment instructions";
const REMOVED_VALUE_DECLARATIONS: &str = "removed value declarations";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, MirPassStage::ProofRich, NAME, DESCRIPTION),
    MirPassImplementation::proof_rich(IDENTITY, transform),
);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct EliminationCount {
    assignments: usize,
    values: usize,
}

impl EliminationCount {
    const fn has_changes(self) -> bool {
        self.assignments != 0
    }

    fn add(&mut self, other: Self) {
        self.assignments = self.assignments.saturating_add(other.assignments);
        self.values = self.values.saturating_add(other.values);
    }
}

fn transform(capability: MirProofPassCapability) -> Result<MirProofPassOutcome, MirPassFailure> {
    let mut processed_callables = 0;
    let mut has_candidate = false;
    for definition in capability.verified().executable_definitions() {
        processed_callables += 1;
        let census =
            value_use_census_for_definition(definition).map_err(MirPassFailure::Rewrite)?;
        has_candidate |= definition
            .body()
            .blocks
            .iter()
            .enumerate()
            .any(|(block, basic_block)| {
                basic_block.instructions.iter().enumerate().any(
                    |(instruction, instruction_value)| {
                        removable_assignment(
                            &census,
                            MirLocalIdentitySite::Instruction { block, instruction },
                            instruction_value,
                        )
                        .is_some()
                    },
                )
            });
    }

    if !has_candidate {
        return capability.unchanged_with(pass_data(processed_callables, 0, Default::default()));
    }

    let mut removed = EliminationCount::default();
    let mut changed_callables = 0;
    let rewritten = capability.rewrite(|_, edit| {
        let callable_removed = eliminate_callable(edit)?;
        if callable_removed.has_changes() {
            changed_callables += 1;
            removed.add(callable_removed);
        }
        Ok(())
    })?;

    rewritten.finish(pass_data(0, changed_callables, removed))
}

fn pass_data(
    processed_callables: usize,
    changed_callables: usize,
    removed: EliminationCount,
) -> MirPassData {
    let data = if changed_callables == 0 {
        MirPassData::processed(processed_callables)
    } else {
        MirPassData::changed(changed_callables)
    };
    data.with_measurement(MirPassMeasurement::count(
        REMOVED_ASSIGNMENTS,
        count(removed.assignments),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_VALUE_DECLARATIONS,
        count(removed.values),
    ))
}

fn eliminate_callable(
    edit: &mut MirCallableEdit,
) -> Result<EliminationCount, crate::mir::rewrite::MirRewriteError> {
    let mut removed = EliminationCount::default();

    loop {
        let census = edit.value_use_census()?;
        let mut values = Vec::new();
        let mut by_block = Vec::new();

        for &block in edit.block_order() {
            let selected = edit
                .block(block)?
                .instructions
                .iter()
                .enumerate()
                .filter_map(|(instruction, instruction_value)| {
                    removable_assignment(
                        &census,
                        MirLocalIdentitySite::Instruction {
                            block: block.index(),
                            instruction,
                        },
                        instruction_value,
                    )
                })
                .collect::<Vec<_>>();
            if !selected.is_empty() {
                values.extend(selected.iter().copied());
                by_block.push((block, selected.into_iter().collect::<BTreeSet<_>>()));
            }
        }

        if values.is_empty() {
            break;
        }

        for (block, selected) in by_block {
            edit.rewrite_block_instructions(block, |instructions| {
                instructions
                    .iter()
                    .filter(|instruction| {
                        !matches!(instruction, MirInstruction::Assign(assignment) if selected.contains(&assignment.result))
                    })
                    .cloned()
                    .collect()
            })?;
        }
        for value in values.iter().copied() {
            edit.remove_value(value)?;
        }

        removed.assignments = removed.assignments.saturating_add(values.len());
        removed.values = removed.values.saturating_add(values.len());
    }

    Ok(removed)
}

fn removable_assignment(
    census: &MirValueUseCensus,
    site: MirLocalIdentitySite,
    instruction: &MirInstruction,
) -> Option<ValueId> {
    let assignment = match instruction {
        MirInstruction::Assign(assignment) => assignment,
        MirInstruction::StorageLive(_)
        | MirInstruction::StorageDead(_)
        | MirInstruction::Call(_)
        | MirInstruction::Cleanup(_)
        | MirInstruction::Initialize(_)
        | MirInstruction::Store(_)
        | MirInstruction::CopyConstruct(_)
        | MirInstruction::CopyAssign(_)
        | MirInstruction::EndFullExpression(_)
        | MirInstruction::BindCheckedView(_)
        | MirInstruction::EndCheckedView(_)
        | MirInstruction::SharedAllocate(_)
        | MirInstruction::SharedInitialize(_)
        | MirInstruction::SharedPublish(_)
        | MirInstruction::SharedStatic(_)
        | MirInstruction::SharedAdopt(_)
        | MirInstruction::SharedCopy(_)
        | MirInstruction::SharedFieldCopy(_)
        | MirInstruction::SharedCast(_)
        | MirInstruction::SharedMove(_)
        | MirInstruction::SharedRelease(_)
        | MirInstruction::SharedFieldInitialize(_)
        | MirInstruction::SharedFieldReplace(_)
        | MirInstruction::StringInitialize(_)
        | MirInstruction::OptionalInitialize(_)
        | MirInstruction::OptionalAssign(_)
        | MirInstruction::AggregateOptionalInitialize(_)
        | MirInstruction::AggregateOptionalAssign(_)
        | MirInstruction::AggregateOptionalPublish(_)
        | MirInstruction::AggregateOptionalCleanup(_)
        | MirInstruction::ClassOptionalInitialize(_)
        | MirInstruction::ClassOptionalAssign(_)
        | MirInstruction::ClassOptionalPublish(_)
        | MirInstruction::ClassOptionalCleanup(_)
        | MirInstruction::EndOptionalView(_)
        | MirInstruction::EndOptionalBoxView(_)
        | MirInstruction::OptionalSharedInitialize(_)
        | MirInstruction::OptionalSharedAssign(_)
        | MirInstruction::OptionalSharedCleanup(_)
        | MirInstruction::Array(_)
        | MirInstruction::Io(_) => return None,
    };

    removable_rvalue(&assignment.rvalue.kind)
        .then_some(assignment)
        .and_then(|assignment| unused_definition_at(census, site, assignment))
}

fn unused_definition_at(
    census: &MirValueUseCensus,
    site: MirLocalIdentitySite,
    assignment: &MirAssignment,
) -> Option<ValueId> {
    census
        .get(assignment.result)
        .filter(|entry| entry.definition() == Some(site) && entry.uses() == 0)
        .map(|entry| entry.value())
}

const fn removable_rvalue(kind: &MirRvalueKind) -> bool {
    match kind {
        MirRvalueKind::ConstantI64(_)
        | MirRvalueKind::ConstantU64(_)
        | MirRvalueKind::ConstantU8(_)
        | MirRvalueKind::ConstantF64Bits(_)
        | MirRvalueKind::ConstantBool(_)
        | MirRvalueKind::Unary { .. }
        | MirRvalueKind::Binary { .. }
        | MirRvalueKind::PrimitiveComparison { .. } => true,
        MirRvalueKind::PrimitiveCast { operation, .. } => !operation.may_terminate(),
        MirRvalueKind::CallableAddress(_)
        | MirRvalueKind::PathCondition(_)
        | MirRvalueKind::Load(_)
        | MirRvalueKind::IntegerDivision { .. }
        | MirRvalueKind::Shift { .. }
        | MirRvalueKind::CheckedF64ToInteger { .. }
        | MirRvalueKind::TypeTest { .. }
        | MirRvalueKind::OptionalPresence { .. }
        | MirRvalueKind::OptionalBoxPresence { .. }
        | MirRvalueKind::ArrayLength { .. } => false,
    }
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
