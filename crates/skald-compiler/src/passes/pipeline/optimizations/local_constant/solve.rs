//! Monotonic iterative solution of one immutable local constant graph.

use std::{collections::VecDeque, fmt};

use crate::{
    identity::CallableId,
    mir::{BlockId, MirDefinitionRef, MirTerminationReason, MirType, StorageId, ValueId},
};

use super::{
    super::{
        checked_integer_evaluation::{
            evaluate_integer_division, evaluate_shift, CheckedIntegerEvaluation,
        },
        checked_integer_topology::CheckedIntegerProtocolOperation,
        primitive_evaluation::{evaluate_rvalue, PrimitiveConstant, PrimitiveEvaluation},
    },
    graph::{LocalConstantGraph, NodeIndex, Producer},
    logical::{select_logical_path, LogicalTransferSelection},
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in crate::passes::pipeline::optimizations) enum LocalConstantIdentity {
    Value(ValueId),
    Carrier(StorageId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline::optimizations) enum LocalConstantProvenanceCategory {
    Literal,
    Primitive,
    CarrierStore,
    CarrierLoad,
    CheckedInteger,
    LogicalShort,
    LogicalRight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline::optimizations) struct LocalConstantProvenance {
    category: LocalConstantProvenanceCategory,
    depth: usize,
    crossed_carrier: bool,
    crossed_checked: bool,
    crossed_logical: bool,
}

impl LocalConstantProvenance {
    pub(in crate::passes::pipeline::optimizations) const fn category(
        self,
    ) -> LocalConstantProvenanceCategory {
        self.category
    }

    pub(in crate::passes::pipeline::optimizations) const fn depth(self) -> usize {
        self.depth
    }

    pub(in crate::passes::pipeline::optimizations) const fn crossed_carrier(self) -> bool {
        self.crossed_carrier
    }

    pub(in crate::passes::pipeline::optimizations) const fn crossed_checked(self) -> bool {
        self.crossed_checked
    }

    pub(in crate::passes::pipeline::optimizations) const fn crossed_logical(self) -> bool {
        self.crossed_logical
    }

    const fn literal() -> Self {
        Self {
            category: LocalConstantProvenanceCategory::Literal,
            depth: 0,
            crossed_carrier: false,
            crossed_checked: false,
            crossed_logical: false,
        }
    }

    fn derived(
        category: LocalConstantProvenanceCategory,
        dependencies: impl IntoIterator<Item = Self>,
    ) -> Self {
        let mut depth = 0usize;
        let mut crossed_carrier = false;
        let mut crossed_checked = false;
        let mut crossed_logical = false;
        for dependency in dependencies {
            depth = depth.max(dependency.depth.saturating_add(1));
            crossed_carrier |= dependency.crossed_carrier;
            crossed_checked |= dependency.crossed_checked;
            crossed_logical |= dependency.crossed_logical;
        }
        crossed_carrier |= matches!(
            category,
            LocalConstantProvenanceCategory::CarrierStore
                | LocalConstantProvenanceCategory::CarrierLoad
        );
        crossed_checked |= category == LocalConstantProvenanceCategory::CheckedInteger;
        crossed_logical |= matches!(
            category,
            LocalConstantProvenanceCategory::LogicalShort
                | LocalConstantProvenanceCategory::LogicalRight
        );
        Self {
            category,
            depth,
            crossed_carrier,
            crossed_checked,
            crossed_logical,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SolvedConstant {
    constant: PrimitiveConstant,
    provenance: LocalConstantProvenance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline::optimizations) struct LocalConstantFact {
    identity: LocalConstantIdentity,
    constant: PrimitiveConstant,
    provenance: LocalConstantProvenance,
}

impl LocalConstantFact {
    pub(in crate::passes::pipeline::optimizations) const fn identity(
        self,
    ) -> LocalConstantIdentity {
        self.identity
    }

    pub(in crate::passes::pipeline::optimizations) const fn constant(self) -> PrimitiveConstant {
        self.constant
    }

    pub(in crate::passes::pipeline::optimizations) const fn provenance(
        self,
    ) -> LocalConstantProvenance {
        self.provenance
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline::optimizations) enum LogicalSelectionKind {
    Short,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline::optimizations) struct LogicalSelection {
    record_index: usize,
    kind: LogicalSelectionKind,
    constant: Option<PrimitiveConstant>,
}

impl LogicalSelection {
    pub(in crate::passes::pipeline::optimizations) const fn record_index(self) -> usize {
        self.record_index
    }

    pub(in crate::passes::pipeline::optimizations) const fn kind(self) -> LogicalSelectionKind {
        self.kind
    }

    pub(in crate::passes::pipeline::optimizations) const fn constant(
        self,
    ) -> Option<PrimitiveConstant> {
        self.constant
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline::optimizations) struct RetainedCheckedFailure {
    result: ValueId,
    check_block: BlockId,
    reason: MirTerminationReason,
}

impl RetainedCheckedFailure {
    pub(in crate::passes::pipeline::optimizations) const fn result(self) -> ValueId {
        self.result
    }

    pub(in crate::passes::pipeline::optimizations) const fn check_block(self) -> BlockId {
        self.check_block
    }

    pub(in crate::passes::pipeline::optimizations) const fn reason(self) -> MirTerminationReason {
        self.reason
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline::optimizations) enum LocalConstantAnalysisError {
    Rewrite(crate::mir::rewrite::MirRewriteError),
    InvalidValueIdentity {
        expected: ValueId,
        actual: ValueId,
    },
    InvalidStorageIdentity {
        expected: StorageId,
        actual: StorageId,
    },
    UnknownValue {
        expected: CallableId,
        value: ValueId,
    },
    UnknownStorage {
        expected: CallableId,
        storage: StorageId,
    },
    DuplicateProducer {
        identity: LocalConstantIdentity,
    },
    DeclaredTypeMismatch {
        identity: LocalConstantIdentity,
        declared: Option<MirType>,
        produced: MirType,
    },
    DerivedTypeMismatch {
        identity: LocalConstantIdentity,
        expected: MirType,
        actual: MirType,
    },
    ContradictoryConstant {
        identity: LocalConstantIdentity,
        first: PrimitiveConstant,
        second: PrimitiveConstant,
    },
    ContradictoryLogicalSelection {
        record_index: usize,
    },
    InvalidProducer {
        identity: LocalConstantIdentity,
    },
    InvalidLogicalRecord {
        record_index: usize,
    },
    DuplicateLogicalRecord {
        record_index: usize,
    },
}

impl fmt::Display for LocalConstantAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "local constant analysis failed: {self:?}")
    }
}

impl std::error::Error for LocalConstantAnalysisError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline::optimizations) struct LocalConstantSolution {
    callable: CallableId,
    value_constants: Vec<Option<SolvedConstant>>,
    carrier_constants: Vec<Option<SolvedConstant>>,
    facts: Vec<LocalConstantFact>,
    logical_selections: Vec<Option<LogicalSelection>>,
    stable_selections: Vec<LogicalSelection>,
    retained_checked_failures: Vec<RetainedCheckedFailure>,
}

impl LocalConstantSolution {
    pub(in crate::passes::pipeline::optimizations) fn fact(
        &self,
        value: ValueId,
    ) -> Result<Option<LocalConstantFact>, LocalConstantAnalysisError> {
        if value.callable() != self.callable || value.index() >= self.value_constants.len() {
            return Err(LocalConstantAnalysisError::UnknownValue {
                expected: self.callable,
                value,
            });
        }
        Ok(
            self.value_constants[value.index()].map(|fact| LocalConstantFact {
                identity: LocalConstantIdentity::Value(value),
                constant: fact.constant,
                provenance: fact.provenance,
            }),
        )
    }

    pub(in crate::passes::pipeline::optimizations) fn constant(
        &self,
        value: ValueId,
    ) -> Result<Option<PrimitiveConstant>, LocalConstantAnalysisError> {
        if value.callable() != self.callable || value.index() >= self.value_constants.len() {
            return Err(LocalConstantAnalysisError::UnknownValue {
                expected: self.callable,
                value,
            });
        }
        Ok(self.value_constants[value.index()].map(|fact| fact.constant))
    }

    pub(in crate::passes::pipeline::optimizations) fn carrier_constant(
        &self,
        storage: StorageId,
    ) -> Result<Option<PrimitiveConstant>, LocalConstantAnalysisError> {
        if storage.callable() != self.callable || storage.index() >= self.carrier_constants.len() {
            return Err(LocalConstantAnalysisError::UnknownStorage {
                expected: self.callable,
                storage,
            });
        }
        Ok(self.carrier_constants[storage.index()].map(|fact| fact.constant))
    }

    pub(in crate::passes::pipeline::optimizations) fn selection(
        &self,
        record_index: usize,
    ) -> Result<Option<LogicalSelection>, LocalConstantAnalysisError> {
        self.logical_selections
            .get(record_index)
            .copied()
            .ok_or(LocalConstantAnalysisError::InvalidLogicalRecord { record_index })
    }

    pub(in crate::passes::pipeline::optimizations) fn facts(&self) -> &[LocalConstantFact] {
        &self.facts
    }

    pub(in crate::passes::pipeline::optimizations) fn selections(&self) -> &[LogicalSelection] {
        &self.stable_selections
    }

    pub(in crate::passes::pipeline::optimizations) fn retained_checked_failures(
        &self,
    ) -> &[RetainedCheckedFailure] {
        &self.retained_checked_failures
    }

    pub(super) fn local_constant(&self, value: ValueId) -> Option<PrimitiveConstant> {
        debug_assert_eq!(value.callable(), self.callable);
        self.value_constants
            .get(value.index())
            .copied()
            .flatten()
            .map(|fact| fact.constant)
    }
}

pub(in crate::passes::pipeline::optimizations) fn solve_local_constants(
    definition: MirDefinitionRef<'_>,
) -> Result<LocalConstantSolution, LocalConstantAnalysisError> {
    solve_with_seed_order(definition, false)
}

#[cfg(test)]
pub(super) fn solve_local_constants_with_reversed_seeds(
    definition: MirDefinitionRef<'_>,
) -> Result<LocalConstantSolution, LocalConstantAnalysisError> {
    solve_with_seed_order(definition, true)
}

fn solve_with_seed_order(
    definition: MirDefinitionRef<'_>,
    reversed: bool,
) -> Result<LocalConstantSolution, LocalConstantAnalysisError> {
    let graph = LocalConstantGraph::build(definition)?;
    let mut state = SolverState {
        constants: vec![None; graph.node_count()],
        selection_states: vec![None; graph.logical_record_count()],
        queue: graph.producer_nodes(reversed).into(),
    };

    while let Some(node) = state.queue.pop_front() {
        state.evaluate_node(&graph, node)?;
    }
    state.finish(&graph)
}

struct SolverState {
    constants: Vec<Option<SolvedConstant>>,
    selection_states: Vec<Option<LogicalSelectionKind>>,
    queue: VecDeque<NodeIndex>,
}

impl SolverState {
    fn evaluate_node(
        &mut self,
        graph: &LocalConstantGraph,
        target: NodeIndex,
    ) -> Result<(), LocalConstantAnalysisError> {
        let Some(producer) = graph.producer(target).cloned() else {
            return Ok(());
        };
        let derived = match producer {
            Producer::Primitive { rvalue, category } => {
                let PrimitiveEvaluation::Constant(constant) =
                    evaluate_rvalue(&rvalue, |value| self.value_constant(graph, value))
                else {
                    return Ok(());
                };
                let dependencies = primitive_dependencies(&rvalue)
                    .into_iter()
                    .filter_map(|value| self.value_fact(graph, value));
                let provenance = if category == LocalConstantProvenanceCategory::Literal {
                    LocalConstantProvenance::literal()
                } else {
                    LocalConstantProvenance::derived(
                        category,
                        dependencies.map(|fact| fact.provenance),
                    )
                };
                Some(SolvedConstant {
                    constant,
                    provenance,
                })
            }
            Producer::Transfer { source, category } => {
                self.constants[source.0].map(|source| SolvedConstant {
                    constant: source.constant,
                    provenance: LocalConstantProvenance::derived(category, [source.provenance]),
                })
            }
            Producer::Checked {
                operation,
                operands,
                ..
            } => {
                let (Some(first), Some(second)) =
                    (self.constants[operands[0].0], self.constants[operands[1].0])
                else {
                    return Ok(());
                };
                match evaluate_checked(operation, first.constant, second.constant) {
                    CheckedIntegerEvaluation::Success(constant) => Some(SolvedConstant {
                        constant,
                        provenance: LocalConstantProvenance::derived(
                            LocalConstantProvenanceCategory::CheckedInteger,
                            [first.provenance, second.provenance],
                        ),
                    }),
                    CheckedIntegerEvaluation::Failure(_)
                    | CheckedIntegerEvaluation::Unsupported => None,
                }
            }
            Producer::Logical { transfer } => {
                let transfer = graph.logical_transfer(transfer)?;
                let Some(left) = self.constants[transfer.left.0] else {
                    return Ok(());
                };
                let Some(selection) = select_logical_path(transfer.operation, left.constant) else {
                    return Err(LocalConstantAnalysisError::DerivedTypeMismatch {
                        identity: graph.identity(transfer.left),
                        expected: MirType::Bool,
                        actual: left.constant.ty(),
                    });
                };
                match selection {
                    LogicalTransferSelection::Short(constant) => {
                        self.publish_selection(transfer.record_index, LogicalSelectionKind::Short)?;
                        Some(SolvedConstant {
                            constant,
                            provenance: LocalConstantProvenance::derived(
                                LocalConstantProvenanceCategory::LogicalShort,
                                [left.provenance],
                            ),
                        })
                    }
                    LogicalTransferSelection::Right => {
                        let right = self.constants[transfer.right.0];
                        self.publish_selection(transfer.record_index, LogicalSelectionKind::Right)?;
                        right.map(|right| SolvedConstant {
                            constant: right.constant,
                            provenance: LocalConstantProvenance::derived(
                                LocalConstantProvenanceCategory::LogicalRight,
                                [left.provenance, right.provenance],
                            ),
                        })
                    }
                }
            }
        };
        if let Some(derived) = derived {
            self.publish_constant(graph, target, derived)?;
        }
        Ok(())
    }

    fn publish_constant(
        &mut self,
        graph: &LocalConstantGraph,
        target: NodeIndex,
        derived: SolvedConstant,
    ) -> Result<(), LocalConstantAnalysisError> {
        let expected =
            graph
                .node_type(target)
                .ok_or(LocalConstantAnalysisError::InvalidProducer {
                    identity: graph.identity(target),
                })?;
        if derived.constant.ty() != expected {
            return Err(LocalConstantAnalysisError::DerivedTypeMismatch {
                identity: graph.identity(target),
                expected,
                actual: derived.constant.ty(),
            });
        }
        match self.constants[target.0] {
            Some(existing) if existing.constant != derived.constant => {
                return Err(LocalConstantAnalysisError::ContradictoryConstant {
                    identity: graph.identity(target),
                    first: existing.constant,
                    second: derived.constant,
                });
            }
            Some(_) => return Ok(()),
            None => self.constants[target.0] = Some(derived),
        }
        self.queue.extend(graph.dependents(target).iter().copied());
        Ok(())
    }

    fn publish_selection(
        &mut self,
        record_index: usize,
        selection: LogicalSelectionKind,
    ) -> Result<(), LocalConstantAnalysisError> {
        let slot = self
            .selection_states
            .get_mut(record_index)
            .ok_or(LocalConstantAnalysisError::InvalidLogicalRecord { record_index })?;
        match *slot {
            Some(existing) if existing != selection => {
                Err(LocalConstantAnalysisError::ContradictoryLogicalSelection { record_index })
            }
            Some(_) => Ok(()),
            None => {
                *slot = Some(selection);
                Ok(())
            }
        }
    }

    fn value_fact(&self, graph: &LocalConstantGraph, value: ValueId) -> Option<SolvedConstant> {
        let node = graph
            .node_for_identity(LocalConstantIdentity::Value(value))
            .ok()
            .flatten()?;
        self.constants[node.0]
    }

    fn value_constant(
        &self,
        graph: &LocalConstantGraph,
        value: ValueId,
    ) -> Option<PrimitiveConstant> {
        self.value_fact(graph, value).map(|fact| fact.constant)
    }

    fn finish(
        self,
        graph: &LocalConstantGraph,
    ) -> Result<LocalConstantSolution, LocalConstantAnalysisError> {
        let mut value_constants = vec![None; graph.value_count()];
        let mut carrier_constants = vec![None; graph.storage_count()];
        let mut facts = Vec::new();
        for (index, constant) in self.constants.iter().copied().enumerate() {
            let Some(constant) = constant else {
                continue;
            };
            let identity = graph.identity(NodeIndex(index));
            match identity {
                LocalConstantIdentity::Value(value) => {
                    value_constants[value.index()] = Some(constant);
                }
                LocalConstantIdentity::Carrier(storage) => {
                    carrier_constants[storage.index()] = Some(constant);
                }
            }
            facts.push(LocalConstantFact {
                identity,
                constant: constant.constant,
                provenance: constant.provenance,
            });
        }

        let mut logical_selections = vec![None; graph.logical_record_count()];
        let mut stable_selections = Vec::new();
        for (record_index, kind) in self.selection_states.iter().copied().enumerate() {
            let Some(kind) = kind else {
                continue;
            };
            let transfer = graph.logical_transfer(record_index)?;
            let selection = LogicalSelection {
                record_index,
                kind,
                constant: self.constants[transfer.result.0].map(|fact| fact.constant),
            };
            logical_selections[record_index] = Some(selection);
            stable_selections.push(selection);
        }
        let mut retained_checked_failures = Vec::new();
        for index in 0..graph.node_count() {
            let target = NodeIndex(index);
            let Some(Producer::Checked {
                operation,
                operands,
                check_block,
            }) = graph.producer(target)
            else {
                continue;
            };
            let (Some(first), Some(second)) =
                (self.constants[operands[0].0], self.constants[operands[1].0])
            else {
                continue;
            };
            if let CheckedIntegerEvaluation::Failure(reason) =
                evaluate_checked(*operation, first.constant, second.constant)
            {
                let LocalConstantIdentity::Value(result) = graph.identity(target) else {
                    return Err(LocalConstantAnalysisError::InvalidProducer {
                        identity: graph.identity(target),
                    });
                };
                retained_checked_failures.push(RetainedCheckedFailure {
                    result,
                    check_block: *check_block,
                    reason,
                });
            }
        }

        Ok(LocalConstantSolution {
            callable: graph.callable(),
            value_constants,
            carrier_constants,
            facts,
            logical_selections,
            stable_selections,
            retained_checked_failures,
        })
    }
}

fn primitive_dependencies(rvalue: &crate::mir::MirRvalueKind) -> Vec<ValueId> {
    match rvalue {
        crate::mir::MirRvalueKind::Unary { operand, .. }
        | crate::mir::MirRvalueKind::PrimitiveCast { operand, .. } => vec![*operand],
        crate::mir::MirRvalueKind::Binary { left, right, .. }
        | crate::mir::MirRvalueKind::PrimitiveComparison { left, right, .. } => {
            vec![*left, *right]
        }
        crate::mir::MirRvalueKind::ConstantI64(_)
        | crate::mir::MirRvalueKind::ConstantU64(_)
        | crate::mir::MirRvalueKind::ConstantU8(_)
        | crate::mir::MirRvalueKind::ConstantF64Bits(_)
        | crate::mir::MirRvalueKind::ConstantBool(_)
        | crate::mir::MirRvalueKind::CallableAddress(_)
        | crate::mir::MirRvalueKind::PathCondition(_)
        | crate::mir::MirRvalueKind::Load(_)
        | crate::mir::MirRvalueKind::IntegerDivision { .. }
        | crate::mir::MirRvalueKind::Shift { .. }
        | crate::mir::MirRvalueKind::CheckedF64ToInteger { .. }
        | crate::mir::MirRvalueKind::TypeTest { .. }
        | crate::mir::MirRvalueKind::OptionalPresence { .. }
        | crate::mir::MirRvalueKind::OptionalBoxPresence { .. }
        | crate::mir::MirRvalueKind::ArrayLength { .. } => Vec::new(),
    }
}

fn evaluate_checked(
    operation: CheckedIntegerProtocolOperation,
    first: PrimitiveConstant,
    second: PrimitiveConstant,
) -> CheckedIntegerEvaluation {
    match operation {
        CheckedIntegerProtocolOperation::Division(operation) => {
            evaluate_integer_division(operation, first, second)
        }
        CheckedIntegerProtocolOperation::Shift(operation) => {
            evaluate_shift(operation, first, second)
        }
    }
}
