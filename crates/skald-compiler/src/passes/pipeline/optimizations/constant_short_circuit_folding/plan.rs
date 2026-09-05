//! Immutable whole-program plan for logical path selection.

use std::{collections::BTreeMap, fmt};

use crate::{
    identity::CallableId,
    mir::{
        rewrite::{MirCallableEdit, MirCallableEditSnapshot, MirRewriteError},
        BlockId, MirInstruction, MirLogicalOperation, MirPlace, MirProgram, MirRvalueKind,
        MirTerminator, MirType,
    },
};

use super::super::{
    local_constant::{
        solve_local_constants, LocalConstantAnalysisError, LogicalSelection, LogicalSelectionKind,
    },
    logical_topology::{
        observe_logical_topologies, LogicalProtocolTopology, LogicalTopologyObservation,
        LogicalTopologyRejectionReason,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum LogicalSelectionPlanError {
    Rewrite(MirRewriteError),
    Analysis(LocalConstantAnalysisError),
    RejectedTopology {
        callable: CallableId,
        record_index: usize,
        reason: LogicalTopologyRejectionReason,
    },
    MissingTopology {
        callable: CallableId,
        record_index: usize,
    },
    InconsistentSelection {
        callable: CallableId,
        record_index: usize,
    },
    ConflictingCandidates {
        callable: CallableId,
    },
}

impl fmt::Display for LogicalSelectionPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rewrite(error) => error.fmt(formatter),
            Self::Analysis(error) => error.fmt(formatter),
            Self::RejectedTopology {
                callable,
                record_index,
                reason,
            } => write!(
                formatter,
                "logical selection plan for {callable} rejected record {record_index}: {reason:?}"
            ),
            Self::MissingTopology {
                callable,
                record_index,
            } => write!(
                formatter,
                "logical selection plan for {callable} has no topology for record {record_index}"
            ),
            Self::InconsistentSelection {
                callable,
                record_index,
            } => write!(
                formatter,
                "logical selection plan for {callable} has an inconsistent selection for record {record_index}"
            ),
            Self::ConflictingCandidates { callable } => write!(
                formatter,
                "logical selection plan for {callable} contains conflicting edits"
            ),
        }
    }
}

impl From<MirRewriteError> for LogicalSelectionPlanError {
    fn from(error: MirRewriteError) -> Self {
        Self::Rewrite(error)
    }
}

impl From<LocalConstantAnalysisError> for LogicalSelectionPlanError {
    fn from(error: LocalConstantAnalysisError) -> Self {
        Self::Analysis(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminatorRewrite {
    block: BlockId,
    expected: MirTerminator,
    replacement: MirTerminator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InstructionRewrite {
    block: BlockId,
    instruction: usize,
    expected: MirInstruction,
    replacement: MirInstruction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LogicalSelectionCandidate {
    record_index: usize,
    operation: MirLogicalOperation,
    selection_kind: LogicalSelectionKind,
    split: TerminatorRewrite,
    selection: TerminatorRewrite,
    selected_result: Option<InstructionRewrite>,
}

impl LogicalSelectionCandidate {
    fn prepare(
        definition: crate::mir::MirDefinitionRef<'_>,
        topology: &LogicalProtocolTopology,
        selection: LogicalSelection,
    ) -> Result<Self, LogicalSelectionPlanError> {
        let callable = definition.callable();
        if selection.record_index() != topology.record_index {
            return Err(LogicalSelectionPlanError::InconsistentSelection {
                callable,
                record_index: selection.record_index(),
            });
        }
        let split =
            definition
                .block(topology.split)
                .ok_or(LogicalSelectionPlanError::MissingTopology {
                    callable,
                    record_index: topology.record_index,
                })?;
        let selection_block = definition.block(topology.selection).ok_or(
            LogicalSelectionPlanError::MissingTopology {
                callable,
                record_index: topology.record_index,
            },
        )?;
        let split_expected =
            split
                .terminator
                .clone()
                .ok_or(LogicalSelectionPlanError::MissingTopology {
                    callable,
                    record_index: topology.record_index,
                })?;
        let selection_expected = selection_block.terminator.clone().ok_or(
            LogicalSelectionPlanError::MissingTopology {
                callable,
                record_index: topology.record_index,
            },
        )?;
        let split_span = terminator_span(&split_expected).ok_or(
            LogicalSelectionPlanError::InconsistentSelection {
                callable,
                record_index: topology.record_index,
            },
        )?;
        let selection_span = terminator_span(&selection_expected).ok_or(
            LogicalSelectionPlanError::InconsistentSelection {
                callable,
                record_index: topology.record_index,
            },
        )?;
        let (split_target, selection_target) = match selection.kind() {
            LogicalSelectionKind::Short => (topology.inactive_predecessor, topology.short),
            LogicalSelectionKind::Right => (topology.active_predecessor, topology.right_entry),
        };
        let selected_result = match selection.constant() {
            None => None,
            Some(super::super::primitive_evaluation::PrimitiveConstant::Bool(value)) => {
                Some(selected_result_rewrite(definition, topology, value)?)
            }
            Some(_) => {
                return Err(LogicalSelectionPlanError::InconsistentSelection {
                    callable,
                    record_index: topology.record_index,
                });
            }
        };

        Ok(Self {
            record_index: topology.record_index,
            operation: topology.operation,
            selection_kind: selection.kind(),
            split: TerminatorRewrite {
                block: topology.split,
                expected: split_expected,
                replacement: MirTerminator::Goto {
                    target: split_target,
                    span: split_span,
                },
            },
            selection: TerminatorRewrite {
                block: topology.selection,
                expected: selection_expected,
                replacement: MirTerminator::Goto {
                    target: selection_target,
                    span: selection_span,
                },
            },
            selected_result,
        })
    }

    fn validate(&self, edit: &MirCallableEdit) -> Result<(), MirRewriteError> {
        validate_terminator(edit, &self.split)?;
        validate_terminator(edit, &self.selection)?;
        if let Some(rewrite) = &self.selected_result {
            let actual = edit
                .block(rewrite.block)?
                .instructions
                .get(rewrite.instruction)
                .ok_or(MirRewriteError::StaleCallableSnapshot {
                    callable: edit.callable(),
                    subject: "logical selected-result instruction position",
                })?;
            if actual != &rewrite.expected {
                return Err(MirRewriteError::StaleCallableSnapshot {
                    callable: edit.callable(),
                    subject: "logical selected-result instruction",
                });
            }
        }
        Ok(())
    }

    fn apply(&self, edit: &mut MirCallableEdit) -> Result<(), MirRewriteError> {
        edit.replace_terminator(
            self.split.block,
            &self.split.expected,
            self.split.replacement.clone(),
        )?;
        edit.replace_terminator(
            self.selection.block,
            &self.selection.expected,
            self.selection.replacement.clone(),
        )?;
        if let Some(rewrite) = &self.selected_result {
            edit.replace_instruction(
                rewrite.block,
                rewrite.instruction,
                &rewrite.expected,
                rewrite.replacement.clone(),
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::passes::pipeline) struct LogicalSelectionCounts {
    pub(in crate::passes::pipeline) and_short: usize,
    pub(in crate::passes::pipeline) and_right: usize,
    pub(in crate::passes::pipeline) or_short: usize,
    pub(in crate::passes::pipeline) or_right: usize,
    pub(in crate::passes::pipeline) replaced_selected_results: usize,
}

impl LogicalSelectionCounts {
    fn record(&mut self, candidate: &LogicalSelectionCandidate) {
        match (candidate.operation, candidate.selection_kind) {
            (MirLogicalOperation::And, LogicalSelectionKind::Short) => {
                self.and_short = self.and_short.saturating_add(1);
            }
            (MirLogicalOperation::And, LogicalSelectionKind::Right) => {
                self.and_right = self.and_right.saturating_add(1);
            }
            (MirLogicalOperation::Or, LogicalSelectionKind::Short) => {
                self.or_short = self.or_short.saturating_add(1);
            }
            (MirLogicalOperation::Or, LogicalSelectionKind::Right) => {
                self.or_right = self.or_right.saturating_add(1);
            }
        }
        if candidate.selected_result.is_some() {
            self.replaced_selected_results = self.replaced_selected_results.saturating_add(1);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallableLogicalSelectionPlan {
    snapshot: MirCallableEditSnapshot,
    candidates: Vec<LogicalSelectionCandidate>,
}

/// Every logical selection derived from one immutable proof-rich program.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::passes::pipeline) struct LogicalSelectionPlan {
    callables: BTreeMap<CallableId, CallableLogicalSelectionPlan>,
    callable_order: Vec<CallableId>,
    selection_count: usize,
}

impl LogicalSelectionPlan {
    pub(super) fn prepare(program: &MirProgram) -> Result<Self, LogicalSelectionPlanError> {
        let mut plan = Self::default();
        for definition in program.executable_definitions() {
            let callable = definition.callable();
            let solution = solve_local_constants(definition)?;
            let mut topologies = vec![None; definition.logical_expressions().len()];
            for observation in observe_logical_topologies(definition)? {
                match observation {
                    LogicalTopologyObservation::Protocol(topology) => {
                        let record_index = topology.record_index;
                        let slot = topologies.get_mut(record_index).ok_or(
                            LogicalSelectionPlanError::MissingTopology {
                                callable,
                                record_index,
                            },
                        )?;
                        if slot.replace(topology).is_some() {
                            return Err(LogicalSelectionPlanError::ConflictingCandidates {
                                callable,
                            });
                        }
                    }
                    LogicalTopologyObservation::Rejected {
                        record_index,
                        reason,
                    } => {
                        return Err(LogicalSelectionPlanError::RejectedTopology {
                            callable,
                            record_index,
                            reason,
                        });
                    }
                }
            }

            let mut candidates = Vec::new();
            for selection in solution.selections() {
                let topology = topologies
                    .get(selection.record_index())
                    .and_then(Option::as_deref)
                    .ok_or(LogicalSelectionPlanError::MissingTopology {
                        callable,
                        record_index: selection.record_index(),
                    })?;
                candidates.push(LogicalSelectionCandidate::prepare(
                    definition, topology, *selection,
                )?);
            }
            if !candidates_are_non_conflicting(&candidates) {
                return Err(LogicalSelectionPlanError::ConflictingCandidates { callable });
            }
            let candidate_count = candidates.len();
            let previous = plan.callables.insert(
                callable,
                CallableLogicalSelectionPlan {
                    snapshot: MirCallableEditSnapshot::capture(definition),
                    candidates,
                },
            );
            if previous.is_some() {
                return Err(LogicalSelectionPlanError::ConflictingCandidates { callable });
            }
            plan.selection_count = plan.selection_count.saturating_add(candidate_count);
            plan.callable_order.push(callable);
        }
        Ok(plan)
    }

    pub(in crate::passes::pipeline) const fn is_empty(&self) -> bool {
        self.selection_count == 0
    }

    pub(in crate::passes::pipeline) fn processed_callables(&self) -> usize {
        self.callable_order.len()
    }

    pub(in crate::passes::pipeline) fn changed_callable_count(&self) -> usize {
        self.callables
            .values()
            .filter(|plan| !plan.candidates.is_empty())
            .count()
    }

    pub(in crate::passes::pipeline) fn counts(&self) -> LogicalSelectionCounts {
        let mut counts = LogicalSelectionCounts::default();
        for candidate in self
            .callables
            .values()
            .flat_map(|callable| &callable.candidates)
        {
            counts.record(candidate);
        }
        counts
    }

    pub(in crate::passes::pipeline) fn validate_program(
        &self,
        program: &MirProgram,
    ) -> Result<(), MirRewriteError> {
        let definitions = program.executable_definitions().collect::<Vec<_>>();
        if !definitions
            .iter()
            .map(|definition| definition.callable())
            .eq(self.callable_order.iter().copied())
        {
            let callable = definitions
                .first()
                .map(|definition| definition.callable())
                .or_else(|| self.callable_order.first().copied())
                .unwrap_or(program.entry_function.into());
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable,
                subject: "logical selection program order",
            });
        }
        for definition in definitions {
            let callable = definition.callable();
            let callable_plan =
                self.callables
                    .get(&callable)
                    .ok_or(MirRewriteError::StaleCallableSnapshot {
                        callable,
                        subject: "logical selection callable inventory",
                    })?;
            if !candidates_are_non_conflicting(&callable_plan.candidates) {
                return Err(MirRewriteError::StaleCallableSnapshot {
                    callable,
                    subject: "logical selection plan conflicts",
                });
            }
            callable_plan
                .snapshot
                .validate_definition(definition, "logical selection plan")?;
        }
        Ok(())
    }

    pub(in crate::passes::pipeline) fn apply_callable(
        &self,
        callable: CallableId,
        edit: &mut MirCallableEdit,
    ) -> Result<(), MirRewriteError> {
        let callable_plan =
            self.callables
                .get(&callable)
                .ok_or(MirRewriteError::StaleCallableSnapshot {
                    callable,
                    subject: "logical selection callable inventory",
                })?;
        callable_plan
            .snapshot
            .validate(edit, "logical selection plan")?;
        if !candidates_are_non_conflicting(&callable_plan.candidates) {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable,
                subject: "logical selection plan conflicts",
            });
        }
        for candidate in &callable_plan.candidates {
            candidate.validate(edit)?;
        }
        for candidate in &callable_plan.candidates {
            candidate.apply(edit)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) const fn selection_count(&self) -> usize {
        self.selection_count
    }

    #[cfg(test)]
    pub(super) fn duplicate_first_candidate_for_test(&mut self) {
        if let Some(plan) = self
            .callables
            .values_mut()
            .find(|plan| !plan.candidates.is_empty())
        {
            plan.candidates.push(plan.candidates[0].clone());
            self.selection_count = self.selection_count.saturating_add(1);
        }
    }
}

fn selected_result_rewrite(
    definition: crate::mir::MirDefinitionRef<'_>,
    topology: &LogicalProtocolTopology,
    value: bool,
) -> Result<InstructionRewrite, LogicalSelectionPlanError> {
    let callable = definition.callable();
    let block =
        definition
            .block(topology.join)
            .ok_or(LogicalSelectionPlanError::MissingTopology {
                callable,
                record_index: topology.record_index,
            })?;
    // The logical proof record authorizes these exact protocol sites even when
    // they also serve as lifecycle attachment roots. The plan only retargets
    // their existing edges and result load; it neither deletes nor moves an
    // attachment, lifetime operation, storage declaration, or block.
    let (instruction, expected) = block
        .instructions
        .iter()
        .enumerate()
        .find(|(_, instruction)| {
            matches!(
                instruction,
                MirInstruction::Assign(assignment)
                    if assignment.result == topology.selected_result
                        && assignment.rvalue.ty == MirType::Bool
                        && matches!(
                            assignment.rvalue.kind,
                            MirRvalueKind::Load(ref place)
                                if *place == MirPlace::base(topology.result)
                        )
            )
        })
        .ok_or(LogicalSelectionPlanError::MissingTopology {
            callable,
            record_index: topology.record_index,
        })?;
    let mut replacement = expected.clone();
    let MirInstruction::Assign(assignment) = &mut replacement else {
        unreachable!("the selected-result instruction is an assignment")
    };
    assignment.rvalue.kind = MirRvalueKind::ConstantBool(value);
    Ok(InstructionRewrite {
        block: topology.join,
        instruction,
        expected: expected.clone(),
        replacement,
    })
}

fn validate_terminator(
    edit: &MirCallableEdit,
    rewrite: &TerminatorRewrite,
) -> Result<(), MirRewriteError> {
    if edit.block(rewrite.block)?.terminator.as_ref() != Some(&rewrite.expected) {
        return Err(MirRewriteError::StaleCallableSnapshot {
            callable: edit.callable(),
            subject: "logical selection terminator",
        });
    }
    Ok(())
}

fn candidates_are_non_conflicting(candidates: &[LogicalSelectionCandidate]) -> bool {
    let mut previous_record = None;
    let mut terminators = std::collections::BTreeSet::new();
    let mut instructions = std::collections::BTreeSet::new();
    for candidate in candidates {
        if previous_record.is_some_and(|previous| previous >= candidate.record_index) {
            return false;
        }
        previous_record = Some(candidate.record_index);
        if !terminators.insert(candidate.split.block)
            || !terminators.insert(candidate.selection.block)
        {
            return false;
        }
        if let Some(rewrite) = &candidate.selected_result {
            if !instructions.insert((rewrite.block, rewrite.instruction)) {
                return false;
            }
        }
    }
    true
}

fn terminator_span(terminator: &MirTerminator) -> Option<crate::source::Span> {
    match terminator {
        MirTerminator::Branch { span, .. } => Some(*span),
        _ => None,
    }
}
