//! Independent stored-draft structural adapter; no builder success is trusted.
use super::super::{
    AddressProvenance, CallableDraft, Definition, ScalarCheck, ScalarDomainEvidence, Terminator,
};
use crate::backend::graph::{
    BlockDescription, DefinitionSite, EdgeDescription, GraphDescription, GraphFailure,
    GraphIdentity, GraphLocation, GraphReason, GraphStage, GraphView, InstructionDescription,
    LoweredBlockId, LoweredObjectId, LoweredValueId, TypedUse, ValueDescription,
};
use crate::backend::plan::{PlanError, ReturnShape, ScalarType};

#[cfg_attr(not(test), allow(dead_code))]
impl GraphView for CallableDraft<'_> {
    type Type = ScalarType;
    fn identity(&self) -> GraphIdentity {
        GraphIdentity {
            stage: GraphStage::Lowered,
            target: self.owner.context().profile(),
            callable: self.owner.key(),
        }
    }
    fn describe(&self) -> Result<GraphDescription<ScalarType>, GraphFailure> {
        let identity = self.identity();
        let failure = |location, reason| GraphFailure {
            identity: Box::new(identity),
            location,
            reason,
            origin: None,
        };
        for result in [
            self.blocks.require_owner(self.owner),
            self.values.require_owner(self.owner),
            self.objects.require_owner(self.owner),
        ] {
            result.map_err(|error| self.lookup_error(GraphLocation::Entry, error))?;
        }
        let signature = self
            .owner
            .signature()
            .map_err(|_| failure(GraphLocation::Entry, GraphReason::TypeMismatch))?;
        if signature.inputs.len() != self.inputs.len() {
            return Err(failure(GraphLocation::Entry, GraphReason::ArityMismatch));
        }
        let entry = self
            .entry
            .map(|id| self.block_index(id, GraphLocation::Entry))
            .transpose()?;
        let inputs = self
            .inputs
            .iter()
            .zip(&signature.inputs)
            .map(|(id, component)| Ok((self.value_index(*id, GraphLocation::Entry)?, component.ty)))
            .collect::<Result<_, GraphFailure>>()?;
        let values = self
            .values
            .iter()
            .map(|(id, value)| {
                let location = GraphLocation::Value(id.index());
                self.value_index(id, location)?;
                if let AddressProvenance::Object { object, .. } = value.provenance {
                    self.object_index(object, location)?;
                }
                let definition = value
                    .definition
                    .map(|definition| {
                        Ok(match definition {
                            Definition::EntryInput { component } => {
                                DefinitionSite::EntryInput(component)
                            }
                            Definition::BlockParameter { block, ordinal } => {
                                DefinitionSite::Parameter {
                                    block: self.block_index(block, location)?,
                                    ordinal,
                                }
                            }
                            Definition::InstructionResult {
                                instruction,
                                ordinal,
                            } => DefinitionSite::Result {
                                block: self.block_index(instruction.block, location)?,
                                instruction: instruction.ordinal,
                                ordinal,
                            },
                        })
                    })
                    .transpose()?;
                Ok(ValueDescription {
                    ty: value.ty,
                    definition,
                    origin: value.origin,
                })
            })
            .collect::<Result<_, GraphFailure>>()?;
        let blocks = self
            .blocks
            .iter()
            .map(|(id, block)| {
                let index = self.block_index(id, GraphLocation::Block(id.index()))?;
                let parameters = block
                    .parameters
                    .as_ref()
                    .map(|parameters| {
                        parameters
                            .iter()
                            .enumerate()
                            .map(|(ordinal, value)| {
                                self.value_index(
                                    *value,
                                    GraphLocation::Parameter {
                                        block: index,
                                        ordinal,
                                    },
                                )
                            })
                            .collect::<Result<_, _>>()
                    })
                    .transpose()?;
                let instructions = block
                    .instructions
                    .iter()
                    .enumerate()
                    .map(|(ordinal, instruction)| {
                        let location = GraphLocation::Instruction {
                            block: index,
                            ordinal,
                            operand: 0,
                        };
                        let (uses, types) =
                            self.operation_description(&instruction.operation, location)?;
                        if types.len() != instruction.results.len() {
                            return Err(failure(location, GraphReason::ArityMismatch));
                        }
                        let results = instruction
                            .results
                            .iter()
                            .zip(types)
                            .enumerate()
                            .map(|(operand, (value, ty))| {
                                Ok((
                                    self.value_index(
                                        *value,
                                        GraphLocation::Instruction {
                                            block: index,
                                            ordinal,
                                            operand,
                                        },
                                    )?,
                                    ty,
                                ))
                            })
                            .collect::<Result<_, GraphFailure>>()?;
                        // Effect object IDs are references too; effect sufficiency is a later check.
                        self.effect_references(&instruction.effects, location)?;
                        Ok(InstructionDescription { uses, results })
                    })
                    .collect::<Result<_, GraphFailure>>()?;
                let terminal = block
                    .terminator
                    .as_ref()
                    .map(|terminal| {
                        let location = GraphLocation::Terminator {
                            block: index,
                            operand: 0,
                        };
                        let uses = match terminal {
                            Terminator::Branch { condition, .. } => {
                                vec![self.used(*condition, Some(ScalarType::Bool), location)?]
                            }
                            Terminator::ScalarCheck { relation, .. } => match relation {
                                ScalarCheck::NonZeroDivisor { ty, divisor } => {
                                    vec![self.used(*divisor, Some(*ty), location)?]
                                }
                                ScalarCheck::ShiftCountBelowWidth { count, .. } => {
                                    vec![self.used(*count, Some(ScalarType::U64), location)?]
                                }
                                ScalarCheck::FiniteTruncatedF64InIntegerRange {
                                    source, ..
                                } => vec![self.used(*source, Some(ScalarType::F64), location)?],
                            },
                            Terminator::Return(values) => {
                                if signature.returns == ReturnShape::Never
                                    || values.len() != signature.results.len()
                                {
                                    return Err(failure(location, GraphReason::ArityMismatch));
                                }
                                values
                                    .iter()
                                    .zip(&signature.results)
                                    .enumerate()
                                    .map(|(operand, (value, result))| {
                                        self.used(
                                            *value,
                                            Some(result.ty),
                                            GraphLocation::Terminator {
                                                block: index,
                                                operand,
                                            },
                                        )
                                    })
                                    .collect::<Result<_, _>>()?
                            }
                            Terminator::ReportFailure { call, .. }
                            | Terminator::NonReturningCall(call) => {
                                let (uses, results) =
                                    self.call_description(call, true, location)?;
                                if !results.is_empty() {
                                    return Err(failure(location, GraphReason::ArityMismatch));
                                }
                                uses
                            }
                            Terminator::Jump(_) | Terminator::HardTrap => vec![],
                        };
                        let edges = terminal
                            .edges(id)
                            .map(|(occurrence, edge)| {
                                let location = GraphLocation::Edge {
                                    block: index,
                                    slot: occurrence.slot,
                                    operand: 0,
                                };
                                let target = self.block_index(edge.target, location)?;
                                let arguments = edge
                                    .arguments
                                    .iter()
                                    .enumerate()
                                    .map(|(operand, value)| {
                                        self.used(
                                            *value,
                                            None,
                                            GraphLocation::Edge {
                                                block: index,
                                                slot: occurrence.slot,
                                                operand,
                                            },
                                        )
                                    })
                                    .collect::<Result<_, _>>()?;
                                Ok(EdgeDescription { target, arguments })
                            })
                            .collect::<Result<_, GraphFailure>>()?;
                        if let Some(effects) = &block.terminal_effects {
                            self.effect_references(effects, location)?;
                        }
                        Ok((uses, edges))
                    })
                    .transpose()?;
                Ok(BlockDescription {
                    parameters,
                    instructions,
                    terminal,
                })
            })
            .collect::<Result<_, GraphFailure>>()?;
        if let Some(plan) = &self.trace_plan {
            if let Some(record) = plan.record {
                self.object_index(record, GraphLocation::Entry)?;
            }
        }
        Ok(GraphDescription {
            entry,
            inputs,
            values,
            blocks,
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl CallableDraft<'_> {
    pub(super) fn lookup_error(&self, location: GraphLocation, error: PlanError) -> GraphFailure {
        GraphFailure {
            identity: Box::new(self.identity()),
            location,
            reason: match error {
                PlanError::WrongContext => GraphReason::WrongContext,
                PlanError::WrongTarget => GraphReason::WrongTarget,
                PlanError::WrongOwner => GraphReason::WrongOwner,
                _ => GraphReason::OutOfBounds,
            },
            origin: None,
        }
    }
    pub(super) fn graph_error(&self, location: GraphLocation, reason: GraphReason) -> GraphFailure {
        GraphFailure {
            identity: Box::new(self.identity()),
            location,
            reason,
            origin: None,
        }
    }
    pub(super) fn value_index(
        &self,
        id: LoweredValueId,
        location: GraphLocation,
    ) -> Result<usize, GraphFailure> {
        self.values
            .get_id(id)
            .map_err(|error| self.lookup_error(location, error))?;
        Ok(id.index())
    }
    pub(super) fn block_index(
        &self,
        id: LoweredBlockId,
        location: GraphLocation,
    ) -> Result<usize, GraphFailure> {
        self.blocks
            .get_id(id)
            .map_err(|error| self.lookup_error(location, error))?;
        Ok(id.index())
    }
    pub(super) fn object_index(
        &self,
        id: LoweredObjectId,
        location: GraphLocation,
    ) -> Result<usize, GraphFailure> {
        self.objects
            .get_id(id)
            .map_err(|error| self.lookup_error(location, error))?;
        Ok(id.index())
    }
    pub(super) fn used(
        &self,
        id: LoweredValueId,
        expected: Option<ScalarType>,
        location: GraphLocation,
    ) -> Result<TypedUse<ScalarType>, GraphFailure> {
        Ok(TypedUse {
            value: self.value_index(id, location)?,
            expected,
        })
    }
    pub(super) fn ty(
        &self,
        id: LoweredValueId,
        location: GraphLocation,
    ) -> Result<ScalarType, GraphFailure> {
        Ok(self
            .values
            .get_id(id)
            .map_err(|error| self.lookup_error(location, error))?
            .ty)
    }
    fn effect_references(
        &self,
        effects: &crate::backend::effects::Effects<LoweredObjectId>,
        location: GraphLocation,
    ) -> Result<(), GraphFailure> {
        use crate::backend::effects::{Effect, MemoryRegion};
        for effect in effects.iter() {
            if let Effect::Read(MemoryRegion::Object(object))
            | Effect::Write(MemoryRegion::Object(object)) = effect
            {
                self.object_index(*object, location)?;
            }
        }
        Ok(())
    }
    pub(super) fn evidence_references(
        &self,
        evidence: &ScalarDomainEvidence,
        location: GraphLocation,
    ) -> Result<(), GraphFailure> {
        match evidence {
            ScalarDomainEvidence::SuccessCheck(block) => {
                self.block_index(*block, location)?;
            }
            ScalarDomainEvidence::ExactConstant(value) => {
                self.value_index(*value, location)?;
            }
        }
        Ok(())
    }
}
