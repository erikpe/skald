//! Checked structural writes; semantic descriptor verification precedes publication.
use super::{
    storage::{Block, Object, ObjectRole, Terminal, Value},
    AbiBindings, Flow, OperandRole, Payload, Representation, SelectedDraft, SelectionContext,
};
use crate::{
    backend::{
        graph::{
            DefinitionSite, LocalHandle, LoweredBlockId, LoweredObjectId, LoweredValueId,
            OwnedArena, SelectedBlockId, SelectedObjectId, SelectedValueId,
        },
        lir::{CompletionReceipt, ProgramError, VerifiedCallable},
        plan::{LayoutFact, LirCallableId, PlanError},
    },
    source::Span,
};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum SelectedBuildError {
    Context(PlanError),
    DuplicateDefinition,
    Terminated,
    InvalidFlow,
    TypeMismatch,
}
impl From<PlanError> for SelectedBuildError {
    fn from(error: PlanError) -> Self {
        Self::Context(error)
    }
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct SelectedBuilder<'p, P> {
    draft: SelectedDraft<'p, P>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p, P: Payload> SelectedBuilder<'p, P> {
    pub(in crate::backend) fn new(
        context: &'p SelectionContext<'p>,
        key: LirCallableId,
        input: Option<CompletionReceipt<'p>>,
    ) -> Result<Self, ProgramError> {
        let owner = context.binding(key)?;
        match &input {
            Some(receipt) => {
                context.extension.parent().require_input(receipt)?;
                if receipt.owner().key() != key {
                    return Err(PlanError::WrongOwner.into());
                }
            }
            None if matches!(key, LirCallableId::TargetThunk(_)) => {}
            None => return Err(PlanError::AbsentBody.into()),
        }
        Ok(Self {
            draft: SelectedDraft {
                context,
                owner,
                input,
                entry: None,
                inputs: vec![],
                abi: None,
                values: OwnedArena::new(owner),
                blocks: OwnedArena::new(owner),
                objects: OwnedArena::new(owner),
                origins: BTreeMap::new(),
                block_origins: BTreeMap::new(),
                object_origins: BTreeMap::new(),
            },
        })
    }
    pub(in crate::backend) fn value(
        &mut self,
        ty: Representation,
        origin: Option<Span>,
    ) -> Result<LocalHandle<'p, SelectedValueId>, SelectedBuildError> {
        self.draft.context.representation(ty)?;
        Ok(self.draft.values.push(Value {
            ty,
            definition: None,
            origin,
        })?)
    }
    pub(in crate::backend) fn map_origin(
        &mut self,
        body: &VerifiedCallable<'p>,
        from: LoweredValueId,
        to: LocalHandle<'p, SelectedValueId>,
    ) -> Result<(), SelectedBuildError> {
        if !self
            .draft
            .input
            .as_ref()
            .is_some_and(|receipt| receipt.matches(body))
        {
            return Err(PlanError::WrongContext.into());
        }
        let origin = body
            .draft()
            .values()
            .find(|(id, _)| *id == from)
            .ok_or(PlanError::OutOfBounds)?
            .1
            .origin;
        self.draft.values.get(to)?;
        if self.draft.origins.contains_key(&from) {
            return Err(SelectedBuildError::DuplicateDefinition);
        }
        self.draft.values.get_mut(to)?.origin = origin;
        self.draft.origins.insert(from, to.id());
        Ok(())
    }
    pub(in crate::backend) fn block(
        &mut self,
        parameters: &[LocalHandle<'p, SelectedValueId>],
        origin: Option<Span>,
    ) -> Result<LocalHandle<'p, SelectedBlockId>, SelectedBuildError> {
        if parameters
            .iter()
            .map(|v| v.id())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != parameters.len()
        {
            return Err(SelectedBuildError::DuplicateDefinition);
        }
        for v in parameters {
            if self.draft.values.get(*v)?.definition.is_some() {
                return Err(SelectedBuildError::DuplicateDefinition);
            }
        }
        let block = self.draft.blocks.push(Block {
            parameters: parameters.iter().map(|v| v.id()).collect(),
            instructions: vec![],
            terminal: None,
            origin,
        })?;
        for (ordinal, v) in parameters.iter().enumerate() {
            self.draft.values.get_mut(*v)?.definition = Some(DefinitionSite::Parameter {
                block: block.id().index(),
                ordinal,
            });
        }
        Ok(block)
    }
    pub(in crate::backend) fn entry(
        &mut self,
        block: LocalHandle<'p, SelectedBlockId>,
        inputs: &[LocalHandle<'p, SelectedValueId>],
        bindings: AbiBindings,
    ) -> Result<(), SelectedBuildError> {
        self.draft.blocks.get(block)?;
        if self.draft.entry.is_some() {
            return Err(SelectedBuildError::DuplicateDefinition);
        }
        if inputs
            .iter()
            .map(|v| v.id())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != inputs.len()
        {
            return Err(SelectedBuildError::DuplicateDefinition);
        }
        let signature = self.draft.owner.signature()?;
        if inputs.len() != bindings.inputs().len()
            || bindings
                .inputs()
                .iter()
                .map(|b| b.component)
                .collect::<Vec<_>>()
                != signature.inputs
            || bindings
                .results()
                .iter()
                .map(|b| b.component)
                .collect::<Vec<_>>()
                != signature.results
        {
            return Err(SelectedBuildError::TypeMismatch);
        }
        for binding in bindings.inputs().iter().chain(bindings.results()) {
            self.draft.context.require_abi_binding(binding)?;
        }
        for (value, binding) in inputs.iter().zip(bindings.inputs()) {
            if self.draft.values.get(*value)?.ty != binding.representation {
                return Err(SelectedBuildError::TypeMismatch);
            }
        }
        for v in inputs {
            if self.draft.values.get(*v)?.definition.is_some() {
                return Err(SelectedBuildError::DuplicateDefinition);
            }
        }
        for (ordinal, v) in inputs.iter().enumerate() {
            self.draft.values.get_mut(*v)?.definition = Some(DefinitionSite::EntryInput(ordinal));
        }
        self.draft.abi = Some(bindings);
        self.draft.entry = Some(block.id());
        self.draft.inputs = inputs.iter().map(|v| v.id()).collect();
        Ok(())
    }
    pub(in crate::backend) fn append(
        &mut self,
        block: LocalHandle<'p, SelectedBlockId>,
        payload: P,
    ) -> Result<(), SelectedBuildError> {
        let b = self.draft.blocks.get(block)?;
        if b.terminal.is_some() {
            return Err(SelectedBuildError::Terminated);
        }
        if payload.describe().flow != Flow::Instruction || payload.describe().successors != 0 {
            return Err(SelectedBuildError::InvalidFlow);
        }
        let instruction = b.instructions.len();
        let desc = payload.describe();
        let results: Vec<_> = desc
            .operands
            .iter()
            .filter(|op| op.role == OperandRole::Definition)
            .map(|op| op.value)
            .collect();
        if results
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != results.len()
        {
            return Err(SelectedBuildError::DuplicateDefinition);
        }
        for op in &*desc.operands {
            let value = self.draft.values.get_id(op.value)?;
            if value.ty != op.representation {
                return Err(SelectedBuildError::TypeMismatch);
            }
            if op.role == OperandRole::Definition && value.definition.is_some() {
                return Err(SelectedBuildError::DuplicateDefinition);
            }
        }
        for (ordinal, id) in results.iter().enumerate() {
            let handle = self.draft.values.handle_id(*id)?;
            self.draft.values.get_mut(handle)?.definition = Some(DefinitionSite::Result {
                block: block.id().index(),
                instruction,
                ordinal,
            });
        }
        self.draft.blocks.get_mut(block)?.instructions.push(payload);
        Ok(())
    }
    pub(in crate::backend) fn terminate(
        &mut self,
        block: LocalHandle<'p, SelectedBlockId>,
        payload: P,
        edges: &[(
            LocalHandle<'p, SelectedBlockId>,
            Vec<LocalHandle<'p, SelectedValueId>>,
        )],
    ) -> Result<(), SelectedBuildError> {
        if self.draft.blocks.get(block)?.terminal.is_some() {
            return Err(SelectedBuildError::Terminated);
        }
        if payload.describe().flow == Flow::Instruction
            || (payload.describe().flow == Flow::Branch && edges.is_empty())
            || payload.describe().successors != edges.len()
        {
            return Err(SelectedBuildError::InvalidFlow);
        }
        for op in &*payload.describe().operands {
            let value = self.draft.values.get_id(op.value)?;
            if value.ty != op.representation || op.role == OperandRole::Definition {
                return Err(SelectedBuildError::TypeMismatch);
            }
        }
        let mut normalized = vec![];
        for (target, args) in edges {
            self.draft.blocks.get(*target)?;
            for v in args {
                self.draft.values.get(*v)?;
            }
            normalized.push((target.id(), args.iter().map(|v| v.id()).collect()));
        }
        self.draft.blocks.get_mut(block)?.terminal = Some(Terminal {
            payload,
            edges: normalized,
        });
        Ok(())
    }
    pub(in crate::backend) fn object(
        &mut self,
        layout: LayoutFact,
        role: ObjectRole,
        origin: Option<Span>,
    ) -> Result<LocalHandle<'p, SelectedObjectId>, SelectedBuildError> {
        if layout.alignment == 0 || !layout.alignment.is_power_of_two() {
            return Err(PlanError::InvalidLayout.into());
        }
        Ok(self.draft.objects.push(Object {
            layout,
            role,
            origin,
        })?)
    }
    pub(in crate::backend) fn map_block_origin(
        &mut self,
        body: &VerifiedCallable<'p>,
        from: LoweredBlockId,
        to: LocalHandle<'p, SelectedBlockId>,
        origin: Option<Span>,
    ) -> Result<(), SelectedBuildError> {
        if !self
            .draft
            .input
            .as_ref()
            .is_some_and(|receipt| receipt.matches(body))
        {
            return Err(PlanError::WrongContext.into());
        }
        if !body.draft().blocks().any(|(id, _)| id == from) {
            return Err(PlanError::OutOfBounds.into());
        }
        self.draft.blocks.get(to)?;
        if self.draft.block_origins.contains_key(&from) {
            return Err(SelectedBuildError::DuplicateDefinition);
        }
        self.draft.blocks.get_mut(to)?.origin = origin;
        self.draft.block_origins.insert(from, to.id());
        Ok(())
    }
    pub(in crate::backend) fn map_object_origin(
        &mut self,
        body: &VerifiedCallable<'p>,
        from: LoweredObjectId,
        to: LocalHandle<'p, SelectedObjectId>,
    ) -> Result<(), SelectedBuildError> {
        if !self
            .draft
            .input
            .as_ref()
            .is_some_and(|receipt| receipt.matches(body))
        {
            return Err(PlanError::WrongContext.into());
        }
        let origin = body
            .draft()
            .objects()
            .find(|(id, _)| *id == from)
            .ok_or(PlanError::OutOfBounds)?
            .1
            .origin;
        self.draft.objects.get(to)?;
        if self.draft.object_origins.contains_key(&from) {
            return Err(SelectedBuildError::DuplicateDefinition);
        }
        self.draft.objects.get_mut(to)?.origin = origin;
        self.draft.object_origins.insert(from, to.id());
        Ok(())
    }
    pub(in crate::backend) fn block_origin(
        &self,
        block: LocalHandle<'p, SelectedBlockId>,
    ) -> Result<Option<Span>, SelectedBuildError> {
        Ok(self.draft.blocks.get(block)?.origin)
    }
    pub(in crate::backend) fn object_description(
        &self,
        object: LocalHandle<'p, SelectedObjectId>,
    ) -> Result<(LayoutFact, &ObjectRole, Option<Span>), SelectedBuildError> {
        let object = self.draft.objects.get(object)?;
        Ok((object.layout, &object.role, object.origin))
    }
    pub(in crate::backend) fn finish(self) -> SelectedDraft<'p, P> {
        self.draft
    }
}
