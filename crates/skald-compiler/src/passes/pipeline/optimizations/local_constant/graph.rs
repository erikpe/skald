//! Construction of the immutable callable-local constant dependency graph.

use crate::{
    identity::CallableId,
    mir::{
        rewrite::{value_use_census_for_definition, MirRewriteError},
        BlockId, MirDefinitionRef, MirInstruction, MirLogicalOperation, MirRvalueKind, MirType,
        StorageId, ValueId,
    },
};

use super::{
    super::{
        checked_integer_topology::{
            observe_checked_integer_topologies, CheckedIntegerProtocolOperation,
            CheckedIntegerTopologyObservation,
        },
        logical_topology::{observe_logical_topologies, LogicalTopologyObservation},
    },
    carrier::{certify_checked_integer_carriers, CheckedCarrierCertificationObservation},
    LocalConstantAnalysisError, LocalConstantIdentity, LocalConstantProvenanceCategory,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct NodeIndex(pub(super) usize);

#[derive(Clone, Debug)]
pub(super) enum Producer {
    Primitive {
        rvalue: MirRvalueKind,
        category: LocalConstantProvenanceCategory,
    },
    Transfer {
        source: NodeIndex,
        category: LocalConstantProvenanceCategory,
    },
    Checked {
        operation: CheckedIntegerProtocolOperation,
        operands: [NodeIndex; 2],
        check_block: BlockId,
    },
    Logical {
        transfer: usize,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LogicalTransfer {
    pub(super) record_index: usize,
    pub(super) operation: MirLogicalOperation,
    pub(super) left: NodeIndex,
    pub(super) right: NodeIndex,
    pub(super) result: NodeIndex,
}

#[derive(Debug)]
pub(super) struct LocalConstantGraph {
    callable: CallableId,
    value_count: usize,
    storage_count: usize,
    node_types: Vec<Option<MirType>>,
    identities: Vec<Option<LocalConstantIdentity>>,
    producers: Vec<Option<Producer>>,
    reverse_dependencies: Vec<Vec<NodeIndex>>,
    logical_transfers: Vec<Option<LogicalTransfer>>,
}

impl LocalConstantGraph {
    pub(super) fn build(
        definition: MirDefinitionRef<'_>,
    ) -> Result<Self, LocalConstantAnalysisError> {
        // This shared census is also the exhaustive identity validation
        // boundary for every value-bearing MIR role.
        value_use_census_for_definition(definition)?;

        let value_count = definition.values().len();
        let storage_count = definition.storage_entries().len();
        let node_count = value_count.saturating_add(storage_count);
        let callable = definition.callable();
        let mut graph = Self {
            callable,
            value_count,
            storage_count,
            node_types: vec![None; node_count],
            identities: vec![None; node_count],
            producers: vec![None; node_count],
            reverse_dependencies: vec![Vec::new(); node_count],
            logical_transfers: vec![None; definition.logical_expressions().len()],
        };

        for (index, value) in definition.values().iter().enumerate() {
            let expected = ValueId::new(callable, index);
            if value.id != expected {
                return Err(LocalConstantAnalysisError::InvalidValueIdentity {
                    expected,
                    actual: value.id,
                });
            }
            let node = graph.value_node(value.id)?;
            graph.node_types[node.0] = Some(value.ty);
            graph.identities[node.0] = Some(LocalConstantIdentity::Value(value.id));
        }
        for (index, storage) in definition.storage_entries().iter().enumerate() {
            let expected = StorageId::new(callable, index);
            if storage.id != expected {
                return Err(LocalConstantAnalysisError::InvalidStorageIdentity {
                    expected,
                    actual: storage.id,
                });
            }
        }

        graph.add_ordinary_value_producers(definition)?;
        graph.add_certified_carrier_producers(definition)?;
        graph.add_checked_producers(definition)?;
        graph.add_logical_producers(definition)?;
        graph.build_reverse_dependencies()?;
        Ok(graph)
    }

    fn add_ordinary_value_producers(
        &mut self,
        definition: MirDefinitionRef<'_>,
    ) -> Result<(), LocalConstantAnalysisError> {
        for block in &definition.body().blocks {
            for instruction in &block.instructions {
                let MirInstruction::Assign(assignment) = instruction else {
                    continue;
                };
                let target = self.value_node(assignment.result)?;
                if self.node_type(target) != Some(assignment.rvalue.ty) {
                    return Err(LocalConstantAnalysisError::DeclaredTypeMismatch {
                        identity: LocalConstantIdentity::Value(assignment.result),
                        declared: self.node_type(target),
                        produced: assignment.rvalue.ty,
                    });
                }
                let category = match assignment.rvalue.kind {
                    MirRvalueKind::ConstantI64(_)
                    | MirRvalueKind::ConstantU64(_)
                    | MirRvalueKind::ConstantU8(_)
                    | MirRvalueKind::ConstantBool(_) => {
                        Some(LocalConstantProvenanceCategory::Literal)
                    }
                    MirRvalueKind::Unary { .. }
                    | MirRvalueKind::Binary { .. }
                    | MirRvalueKind::PrimitiveComparison { .. }
                    | MirRvalueKind::PrimitiveCast { .. } => {
                        Some(LocalConstantProvenanceCategory::Primitive)
                    }
                    MirRvalueKind::ConstantF64Bits(_)
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
                };
                if let Some(category) = category {
                    self.set_producer(
                        target,
                        Producer::Primitive {
                            rvalue: assignment.rvalue.kind.clone(),
                            category,
                        },
                    )?;
                }
            }
        }
        Ok(())
    }

    fn add_certified_carrier_producers(
        &mut self,
        definition: MirDefinitionRef<'_>,
    ) -> Result<(), LocalConstantAnalysisError> {
        for observation in certify_checked_integer_carriers(definition)? {
            let CheckedCarrierCertificationObservation::Certified(certificate) = observation else {
                continue;
            };
            let carrier = self.carrier_node(certificate.storage())?;
            if self.node_types[carrier.0]
                .replace(certificate.ty())
                .is_some()
            {
                return Err(LocalConstantAnalysisError::DuplicateProducer {
                    identity: LocalConstantIdentity::Carrier(certificate.storage()),
                });
            }
            self.identities[carrier.0] =
                Some(LocalConstantIdentity::Carrier(certificate.storage()));
            self.set_producer(
                carrier,
                Producer::Transfer {
                    source: self.value_node(certificate.store().source())?,
                    category: LocalConstantProvenanceCategory::CarrierStore,
                },
            )?;
            for load in certificate.loads() {
                self.set_producer(
                    self.value_node(load.result())?,
                    Producer::Transfer {
                        source: carrier,
                        category: LocalConstantProvenanceCategory::CarrierLoad,
                    },
                )?;
            }
        }
        Ok(())
    }

    fn add_checked_producers(
        &mut self,
        definition: MirDefinitionRef<'_>,
    ) -> Result<(), LocalConstantAnalysisError> {
        for observation in observe_checked_integer_topologies(definition)? {
            let CheckedIntegerTopologyObservation::Protocol(topology) = observation else {
                continue;
            };
            self.set_producer(
                self.value_node(topology.result_assignment.value)?,
                Producer::Checked {
                    operation: topology.check.operation(),
                    operands: [
                        self.value_node(topology.operand_loads[0].value)?,
                        self.value_node(topology.operand_loads[1].value)?,
                    ],
                    check_block: topology.check_block,
                },
            )?;
        }
        Ok(())
    }

    fn add_logical_producers(
        &mut self,
        definition: MirDefinitionRef<'_>,
    ) -> Result<(), LocalConstantAnalysisError> {
        for observation in observe_logical_topologies(definition)? {
            let LogicalTopologyObservation::Protocol(topology) = observation else {
                continue;
            };
            let transfer = LogicalTransfer {
                record_index: topology.record_index,
                operation: topology.operation,
                left: self.value_node(topology.left_result)?,
                right: self.value_node(topology.right_result)?,
                result: self.value_node(topology.selected_result)?,
            };
            let slot = self
                .logical_transfers
                .get_mut(topology.record_index)
                .ok_or(LocalConstantAnalysisError::InvalidLogicalRecord {
                    record_index: topology.record_index,
                })?;
            if slot.replace(transfer).is_some() {
                return Err(LocalConstantAnalysisError::DuplicateLogicalRecord {
                    record_index: topology.record_index,
                });
            }
            self.set_producer(
                transfer.result,
                Producer::Logical {
                    transfer: topology.record_index,
                },
            )?;
        }
        Ok(())
    }

    fn build_reverse_dependencies(&mut self) -> Result<(), LocalConstantAnalysisError> {
        for target in 0..self.producers.len() {
            let target = NodeIndex(target);
            for dependency in self.dependencies(target)? {
                self.reverse_dependencies[dependency.0].push(target);
            }
        }
        Ok(())
    }

    fn set_producer(
        &mut self,
        target: NodeIndex,
        producer: Producer,
    ) -> Result<(), LocalConstantAnalysisError> {
        if self.producers[target.0].replace(producer).is_some() {
            return Err(LocalConstantAnalysisError::DuplicateProducer {
                identity: self.identity(target),
            });
        }
        Ok(())
    }

    fn dependencies(
        &self,
        target: NodeIndex,
    ) -> Result<Vec<NodeIndex>, LocalConstantAnalysisError> {
        let Some(producer) = &self.producers[target.0] else {
            return Ok(Vec::new());
        };
        Ok(match producer {
            Producer::Primitive { rvalue, .. } => match rvalue {
                MirRvalueKind::Unary { operand, .. }
                | MirRvalueKind::PrimitiveCast { operand, .. } => vec![self.value_node(*operand)?],
                MirRvalueKind::Binary { left, right, .. }
                | MirRvalueKind::PrimitiveComparison { left, right, .. } => {
                    vec![self.value_node(*left)?, self.value_node(*right)?]
                }
                MirRvalueKind::ConstantI64(_)
                | MirRvalueKind::ConstantU64(_)
                | MirRvalueKind::ConstantU8(_)
                | MirRvalueKind::ConstantBool(_) => Vec::new(),
                MirRvalueKind::ConstantF64Bits(_)
                | MirRvalueKind::CallableAddress(_)
                | MirRvalueKind::PathCondition(_)
                | MirRvalueKind::Load(_)
                | MirRvalueKind::IntegerDivision { .. }
                | MirRvalueKind::Shift { .. }
                | MirRvalueKind::CheckedF64ToInteger { .. }
                | MirRvalueKind::TypeTest { .. }
                | MirRvalueKind::OptionalPresence { .. }
                | MirRvalueKind::OptionalBoxPresence { .. }
                | MirRvalueKind::ArrayLength { .. } => {
                    return Err(LocalConstantAnalysisError::InvalidProducer {
                        identity: self.identity(target),
                    });
                }
            },
            Producer::Transfer { source, .. } => vec![*source],
            Producer::Checked { operands, .. } => operands.to_vec(),
            Producer::Logical { transfer } => {
                let transfer = self.logical_transfer(*transfer)?;
                vec![transfer.left, transfer.right]
            }
        })
    }

    pub(super) const fn callable(&self) -> CallableId {
        self.callable
    }

    pub(super) fn node_count(&self) -> usize {
        self.producers.len()
    }

    pub(super) const fn value_count(&self) -> usize {
        self.value_count
    }

    pub(super) const fn storage_count(&self) -> usize {
        self.storage_count
    }

    pub(super) fn producer(&self, node: NodeIndex) -> Option<&Producer> {
        self.producers[node.0].as_ref()
    }

    pub(super) fn producer_nodes(&self, reversed: bool) -> Vec<NodeIndex> {
        let mut nodes = self
            .producers
            .iter()
            .enumerate()
            .filter_map(|(index, producer)| producer.as_ref().map(|_| NodeIndex(index)))
            .collect::<Vec<_>>();
        if reversed {
            nodes.reverse();
        }
        nodes
    }

    pub(super) fn dependents(&self, node: NodeIndex) -> &[NodeIndex] {
        &self.reverse_dependencies[node.0]
    }

    pub(super) fn node_type(&self, node: NodeIndex) -> Option<MirType> {
        self.node_types.get(node.0).copied().flatten()
    }

    pub(super) fn identity(&self, node: NodeIndex) -> LocalConstantIdentity {
        self.identities[node.0].expect("every producer belongs to a fact-bearing graph node")
    }

    pub(super) fn node_for_identity(
        &self,
        identity: LocalConstantIdentity,
    ) -> Result<Option<NodeIndex>, LocalConstantAnalysisError> {
        match identity {
            LocalConstantIdentity::Value(value) => self.value_node(value).map(Some),
            LocalConstantIdentity::Carrier(storage) => {
                let node = self.carrier_node(storage)?;
                Ok(self.identities[node.0].map(|_| node))
            }
        }
    }

    pub(super) fn logical_transfer(
        &self,
        record_index: usize,
    ) -> Result<LogicalTransfer, LocalConstantAnalysisError> {
        self.logical_transfers
            .get(record_index)
            .copied()
            .flatten()
            .ok_or(LocalConstantAnalysisError::InvalidLogicalRecord { record_index })
    }

    pub(super) fn logical_record_count(&self) -> usize {
        self.logical_transfers.len()
    }

    fn value_node(&self, value: ValueId) -> Result<NodeIndex, LocalConstantAnalysisError> {
        if value.callable() != self.callable || value.index() >= self.value_count {
            return Err(LocalConstantAnalysisError::UnknownValue {
                expected: self.callable,
                value,
            });
        }
        Ok(NodeIndex(value.index()))
    }

    fn carrier_node(&self, storage: StorageId) -> Result<NodeIndex, LocalConstantAnalysisError> {
        if storage.callable() != self.callable || storage.index() >= self.storage_count {
            return Err(LocalConstantAnalysisError::UnknownStorage {
                expected: self.callable,
                storage,
            });
        }
        Ok(NodeIndex(self.value_count.saturating_add(storage.index())))
    }
}

impl From<MirRewriteError> for LocalConstantAnalysisError {
    fn from(error: MirRewriteError) -> Self {
        Self::Rewrite(error)
    }
}
