//! Checked draft mutations. Cross-block use/guard validity belongs to verification.

use super::model::{
    Block, BlockHandle, CallableDraft, Definition, Edge, Instruction, InstructionLocation,
    LifetimeDisposition, Object, ObjectHandle, ObjectRole, Operation, Terminator, Value,
    ValueHandle,
};
use super::AddressProvenance;
use crate::backend::effects::{Effect, Effects};
use crate::backend::graph::OwnedArena;
use crate::backend::plan::{CallableBinding, LayoutDisposition, PlanError, ScalarType};
use crate::source::Span;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum BuildError {
    Plan(PlanError),
    SizeOverflow,
    DuplicateDefinition,
    UndefinedBlock,
    AlreadyTerminated,
    InvalidEntry,
    InvalidScalar,
    InvalidMemory,
    InvalidObject,
    InvalidLifetime,
    InvalidEvidence,
    ResultMismatch,
    EdgeMismatch,
    InvalidReturn,
    InvalidCall,
    NarrowedEffects,
    InvalidTrace,
}
impl From<PlanError> for BuildError {
    fn from(error: PlanError) -> Self {
        Self::Plan(error)
    }
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct DraftBuilder<'p> {
    pub(super) draft: CallableDraft<'p>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'p> DraftBuilder<'p> {
    pub(in crate::backend) fn new(owner: CallableBinding<'p>) -> Result<Self, BuildError> {
        let mut builder = Self {
            draft: CallableDraft {
                owner,
                entry: None,
                trace_plan: None,
                inputs: Vec::new(),
                blocks: OwnedArena::new(owner),
                values: OwnedArena::new(owner),
                objects: OwnedArena::new(owner),
            },
        };
        for (component, input) in owner.signature()?.inputs.iter().enumerate() {
            let value = builder.draft.values.push(Value {
                ty: input.ty,
                definition: Some(Definition::EntryInput { component }),
                origin: None,
                provenance: AddressProvenance::Unknown,
            })?;
            builder.draft.inputs.push(value.id());
        }
        Ok(builder)
    }
    pub(in crate::backend) fn inputs(&self) -> impl ExactSizeIterator<Item = ValueHandle<'p>> + '_ {
        // Handles are issued by the arena, never reconstructed from a numeric ID.
        self.draft.values.handles().take(self.draft.inputs.len())
    }
    pub(in crate::backend) fn reserve_block(&mut self) -> Result<BlockHandle<'p>, BuildError> {
        Ok(self.draft.blocks.push(Block::default())?)
    }
    pub(in crate::backend) fn reserve_value(
        &mut self,
        ty: ScalarType,
        origin: Option<Span>,
    ) -> Result<ValueHandle<'p>, BuildError> {
        self.check_type(ty)?;
        Ok(self.draft.values.push(Value {
            ty,
            definition: None,
            origin,
            provenance: AddressProvenance::Unknown,
        })?)
    }
    pub(in crate::backend) fn define_block(
        &mut self,
        block: BlockHandle<'p>,
        parameters: &[ValueHandle<'p>],
    ) -> Result<(), BuildError> {
        if self.draft.blocks.get(block)?.parameters.is_some() {
            return Err(BuildError::DuplicateDefinition);
        }
        self.check_results(parameters, None)?;
        let ids = parameters.iter().map(|parameter| parameter.id()).collect();
        for (ordinal, parameter) in parameters.iter().copied().enumerate() {
            self.draft.values.get_mut(parameter)?.definition = Some(Definition::BlockParameter {
                block: block.id(),
                ordinal,
            });
        }
        self.draft.blocks.get_mut(block)?.parameters = Some(ids);
        Ok(())
    }
    pub(in crate::backend) fn set_entry(
        &mut self,
        block: BlockHandle<'p>,
    ) -> Result<(), BuildError> {
        let record = self.draft.blocks.get(block)?;
        if self.draft.entry.is_some()
            || record.parameters.as_deref() != Some(&[])
            || !record.instructions.is_empty()
            || record.terminator.is_some()
            || self.draft.blocks.iter().any(|(id, record)| {
                record.terminator.as_ref().is_some_and(|terminator| {
                    terminator
                        .edges(id)
                        .any(|(_, edge)| edge.target == block.id())
                })
            })
        {
            return Err(BuildError::InvalidEntry);
        }
        self.draft.entry = Some(block.id());
        Ok(())
    }
    pub(in crate::backend) fn declare_object(
        &mut self,
        object: Object,
    ) -> Result<ObjectHandle<'p>, BuildError> {
        let layout = object.layout;
        if layout.alignment == 0 || !layout.alignment.is_power_of_two() {
            return Err(BuildError::InvalidObject);
        }
        layout
            .size
            .checked_add(layout.alignment - 1)
            .ok_or(BuildError::SizeOverflow)?;
        if layout.disposition != LayoutDisposition::Addressable && layout.size != 0 {
            return Err(BuildError::InvalidObject);
        }
        if matches!(object.lifetime, LifetimeDisposition::Sites(0)) {
            return Err(BuildError::InvalidLifetime);
        }
        if object.role == ObjectRole::TraceRecord
            && self.draft.owner.context().runtime_trace()
                == crate::backend::RuntimeTracePolicy::Omitted
        {
            return Err(BuildError::Plan(PlanError::OmittedTrace));
        }
        Ok(self.draft.objects.push(object)?)
    }
    pub(in crate::backend) fn append(
        &mut self,
        block: BlockHandle<'p>,
        operation: Operation<ValueHandle<'p>, ObjectHandle<'p>, BlockHandle<'p>>,
    ) -> Result<Vec<ValueHandle<'p>>, BuildError> {
        self.writable(block)?;
        let (operation, types) = self.normalize(operation)?;
        let mut results = Vec::with_capacity(types.len());
        for ty in types {
            results.push(self.reserve_value(ty, None)?);
        }
        self.install(block, operation, &results)?;
        Ok(results)
    }
    pub(in crate::backend) fn append_into(
        &mut self,
        block: BlockHandle<'p>,
        operation: Operation<ValueHandle<'p>, ObjectHandle<'p>, BlockHandle<'p>>,
        results: &[ValueHandle<'p>],
    ) -> Result<(), BuildError> {
        self.writable(block)?;
        let (operation, types) = self.normalize(operation)?;
        self.check_results(results, Some(&types))?;
        self.install(block, operation, results)
    }
    fn install(
        &mut self,
        block: BlockHandle<'p>,
        operation: Operation,
        results: &[ValueHandle<'p>],
    ) -> Result<(), BuildError> {
        let ordinal = self.draft.blocks.get(block)?.instructions.len();
        ordinal.checked_add(1).ok_or(BuildError::SizeOverflow)?;
        let effects = self.operation_effects(&operation)?;
        let provenance = self.result_provenance(&operation)?;
        let instruction = InstructionLocation {
            block: block.id(),
            ordinal,
        };
        for (ordinal, result) in results.iter().copied().enumerate() {
            self.draft.values.get_mut(result)?.provenance = provenance;
            self.draft.values.get_mut(result)?.definition = Some(Definition::InstructionResult {
                instruction,
                ordinal,
            });
        }
        self.draft
            .blocks
            .get_mut(block)?
            .instructions
            .push(Instruction {
                operation,
                results: results.iter().map(|value| value.id()).collect(),
                effects,
            });
        Ok(())
    }
    fn check_results(
        &self,
        results: &[ValueHandle<'p>],
        types: Option<&[ScalarType]>,
    ) -> Result<(), BuildError> {
        if types.is_some_and(|types| types.len() != results.len()) {
            return Err(BuildError::ResultMismatch);
        }
        let mut seen = BTreeSet::new();
        for (ordinal, result) in results.iter().copied().enumerate() {
            let record = self.draft.values.get(result)?;
            if record.definition.is_some() || !seen.insert(result.id()) {
                return Err(BuildError::DuplicateDefinition);
            }
            if types.is_some_and(|types| types[ordinal] != record.ty) {
                return Err(BuildError::ResultMismatch);
            }
        }
        Ok(())
    }
    pub(super) fn writable(&self, block: BlockHandle<'p>) -> Result<(), BuildError> {
        let block = self.draft.blocks.get(block)?;
        if block.parameters.is_none() {
            return Err(BuildError::UndefinedBlock);
        }
        if block.terminator.is_some() {
            return Err(BuildError::AlreadyTerminated);
        }
        Ok(())
    }
    fn edge(&self, edge: Edge<ValueHandle<'p>, BlockHandle<'p>>) -> Result<Edge, BuildError> {
        let target = self.draft.blocks.get(edge.target)?;
        if self.draft.entry == Some(edge.target.id()) {
            return Err(BuildError::InvalidEntry);
        }
        let arguments = edge
            .arguments
            .iter()
            .copied()
            .map(|value| self.value(value))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(parameters) = &target.parameters {
            if parameters.len() != arguments.len() {
                return Err(BuildError::EdgeMismatch);
            }
            for (parameter, (_, ty)) in parameters.iter().zip(&arguments) {
                if self.draft.values.get_id(*parameter)?.ty != *ty {
                    return Err(BuildError::EdgeMismatch);
                }
            }
        }
        Ok(Edge {
            target: edge.target.id(),
            arguments: arguments.into_iter().map(|(value, _)| value).collect(),
        })
    }
    pub(in crate::backend) fn terminate(
        &mut self,
        block: BlockHandle<'p>,
        terminator: Terminator<ValueHandle<'p>, BlockHandle<'p>>,
    ) -> Result<(), BuildError> {
        self.writable(block)?;
        let terminator = match terminator {
            Terminator::Jump(edge) => Terminator::Jump(self.edge(edge)?),
            Terminator::Branch {
                condition,
                true_edge,
                false_edge,
            } => {
                let (condition, ty) = self.value(condition)?;
                if ty != ScalarType::Bool {
                    return Err(BuildError::InvalidScalar);
                }
                Terminator::Branch {
                    condition,
                    true_edge: self.edge(true_edge)?,
                    false_edge: self.edge(false_edge)?,
                }
            }
            Terminator::ScalarCheck {
                relation,
                success,
                failure,
            } => Terminator::ScalarCheck {
                relation: self.check_relation(relation)?,
                success: self.edge(success)?,
                failure: self.edge(failure)?,
            },
            Terminator::Return(values) => {
                let signature = self.draft.owner.signature()?;
                if matches!(signature.returns, crate::backend::plan::ReturnShape::Never)
                    || signature.results.len() != values.len()
                {
                    return Err(BuildError::InvalidReturn);
                }
                let values = values
                    .into_iter()
                    .zip(&signature.results)
                    .map(|(value, result)| {
                        let (value, ty) = self.value(value)?;
                        if ty != result.ty {
                            return Err(BuildError::InvalidReturn);
                        }
                        Ok(value)
                    })
                    .collect::<Result<_, _>>()?;
                Terminator::Return(values)
            }
            Terminator::ReportFailure { call, reason } => {
                if !matches!(
                    &call.target,
                    super::CallTarget::Direct(crate::backend::plan::ArtifactId::Runtime(
                        crate::backend::plan::RuntimeService::Panic
                    ))
                ) {
                    return Err(BuildError::InvalidCall);
                }
                let (call, _) = self.normalize_call(call, true)?;
                self.check_failure_message(&call, reason)?;
                Terminator::ReportFailure { call, reason }
            }
            Terminator::NonReturningCall(call) => {
                let (call, _) = self.normalize_call(call, true)?;
                Terminator::NonReturningCall(call)
            }
            Terminator::HardTrap => Terminator::HardTrap,
        };
        let effects = match &terminator {
            Terminator::ReportFailure { call, .. } | Terminator::NonReturningCall(call) => {
                self.call_effects(call)?
            }
            Terminator::HardTrap => Effects::new([Effect::HardTrap]),
            _ => Effects::default(),
        };
        let record = self.draft.blocks.get_mut(block)?;
        record.terminator = Some(terminator);
        record.terminal_effects = Some(effects);
        Ok(())
    }
    pub(in crate::backend) fn finish(self) -> CallableDraft<'p> {
        self.draft
    }
}
