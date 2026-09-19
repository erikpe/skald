use super::LowerError;
use crate::backend::planning::AdmittedProgram;
use crate::{
    backend::{
        lir::{
            verify_callable, BlockHandle, DraftBuilder, ObjectHandle, ValueHandle, VerifiedCallable,
        },
        plan::{CallableBinding, ComponentRole, LirCallableId, PlanError, PlanView, ScalarType},
    },
    identity::CallableId,
    mir::{BlockId, MirDefinitionRef, MirType, StorageId},
};
use std::collections::BTreeMap;

pub(super) struct Lowerer<'plan, 'input> {
    pub(super) admitted: &'plan AdmittedProgram<'input>,
    pub(super) definition: MirDefinitionRef<'plan>,
    pub(super) builder: DraftBuilder<'plan>,
    pub(super) blocks: Vec<BlockHandle<'plan>>,
    pub(super) values: Vec<ValueHandle<'plan>>,
    pub(super) objects: Vec<Option<ObjectHandle<'plan>>>,
    /// Entry-carried addresses replace local objects for aggregate results,
    /// aggregate value parameters, aliases, and receivers.
    pub(super) entry_addresses: BTreeMap<StorageId, ValueHandle<'plan>>,
    pub(super) object_origins: BTreeMap<StorageId, EntryObjectOrigin<'plan>>,
    pub(super) trace_record: Option<ObjectHandle<'plan>>,
    pub(super) guards: BTreeMap<BlockId, super::numeric::Guard>,
    // Computed addresses are block-local, even though the underlying object
    // represents semantic storage across blocks and dynamic lifetime epochs.
    pub(super) addresses: BTreeMap<(BlockId, StorageId), ValueHandle<'plan>>,
}

#[derive(Clone, Copy)]
pub(super) struct ObjectOriginValues<'plan> {
    pub(super) complete: ValueHandle<'plan>,
    pub(super) metadata: ValueHandle<'plan>,
}

#[derive(Default)]
pub(super) struct EntryObjectOrigin<'plan> {
    pub(super) complete: Option<ValueHandle<'plan>>,
    pub(super) metadata: Option<ValueHandle<'plan>>,
}

impl<'plan, 'input> Lowerer<'plan, 'input> {
    pub(super) fn new(
        admitted: &'plan AdmittedProgram<'input>,
        owner: CallableBinding<'plan>,
    ) -> Result<Self, LowerError> {
        admitted
            .plan()
            .view()
            .require_same_context(owner.context())?;
        let LirCallableId::Source(callable) = owner.key() else {
            return Err(PlanError::InvalidDomain.into());
        };
        let definition = definition(admitted, callable)?;
        let mut builder = DraftBuilder::new(owner)?;
        let blocks = definition
            .body()
            .blocks
            .iter()
            .map(|_| builder.reserve_block())
            .collect::<Result<Vec<_>, _>>()?;
        let values = definition
            .values()
            .iter()
            .map(|value| {
                builder
                    .reserve_value(scalar_type(admitted, value.ty)?, Some(value.span))
                    .map_err(LowerError::from)
            })
            .collect::<Result<Vec<_>, LowerError>>()?;
        for block in &blocks {
            builder.define_block(*block, &[])?;
        }
        builder.set_entry(blocks[definition.body().entry.index()])?;
        let mut lowerer = Self {
            admitted,
            definition,
            builder,
            blocks,
            values,
            objects: vec![],
            entry_addresses: BTreeMap::new(),
            object_origins: BTreeMap::new(),
            trace_record: None,
            guards: super::numeric::guards(definition)?,
            addresses: BTreeMap::new(),
        };
        lowerer.declare_storage()?;
        lowerer.initialize_trace()?;
        let entry = definition.body().entry;
        let inputs = lowerer.builder.inputs().collect::<Vec<_>>();
        for (input, component) in inputs.into_iter().zip(&owner.signature()?.inputs) {
            match component.role {
                ComponentRole::ResultDestination(_) => {
                    let storage = definition
                        .return_storage()
                        .ok_or(PlanError::InvalidSignature)?;
                    lowerer.bind_entry_address(storage, input)?;
                }
                ComponentRole::ReceiverStatic => {
                    let storage = definition.receiver().ok_or(PlanError::InvalidSignature)?;
                    lowerer.bind_entry_address(storage, input)?;
                }
                ComponentRole::ReceiverComplete => {
                    let storage = definition.receiver().ok_or(PlanError::InvalidSignature)?;
                    lowerer.bind_origin_complete(storage, input)?;
                }
                ComponentRole::ReceiverMetadata => {
                    let storage = definition.receiver().ok_or(PlanError::InvalidSignature)?;
                    lowerer.bind_origin_metadata(storage, input)?;
                }
                ComponentRole::AggregateAddress { parameter, .. }
                | ComponentRole::AliasAddress(parameter) => {
                    let storage = *definition
                        .parameters()
                        .get(parameter)
                        .ok_or(PlanError::InvalidSignature)?;
                    lowerer.bind_entry_address(storage, input)?;
                }
                ComponentRole::AliasComplete(parameter) => {
                    let storage = *definition
                        .parameters()
                        .get(parameter)
                        .ok_or(PlanError::InvalidSignature)?;
                    lowerer.bind_origin_complete(storage, input)?;
                }
                ComponentRole::AliasMetadata(parameter) => {
                    let storage = *definition
                        .parameters()
                        .get(parameter)
                        .ok_or(PlanError::InvalidSignature)?;
                    lowerer.bind_origin_metadata(storage, input)?;
                }
                ComponentRole::Parameter(index) => {
                    let storage = *definition
                        .parameters()
                        .get(index)
                        .ok_or(PlanError::InvalidSignature)?;
                    lowerer.store(entry, storage, input)?;
                }
                ComponentRole::RuntimeParameter(_) | ComponentRole::Result => {
                    return Err(PlanError::InvalidSignature.into())
                }
            }
        }
        Ok(lowerer)
    }
    pub(super) fn plan(&self) -> PlanView<'plan> {
        self.admitted.plan().view()
    }
    pub(super) fn finish(mut self) -> Result<VerifiedCallable<'plan>, LowerError> {
        for block in &self.definition.body().blocks {
            for instruction in &block.instructions {
                self.instruction(block.id, instruction)?;
            }
            self.terminator(
                block.id,
                block
                    .terminator
                    .as_ref()
                    .expect("verified final MIR terminator"),
            )?;
        }
        verify_callable(self.builder.finish()).map_err(LowerError::Verification)
    }
}

pub(super) fn definition<'plan>(
    admitted: &'plan AdmittedProgram<'_>,
    callable: CallableId,
) -> Result<MirDefinitionRef<'plan>, LowerError> {
    let program = admitted.program();
    match callable {
        CallableId::Function(id) => program.definitions.get(id).map(MirDefinitionRef::Function),
        _ => program
            .member_definition(callable)
            .map(MirDefinitionRef::Member),
    }
    .ok_or(LowerError::MissingBody(callable))
}

pub(super) fn scalar_type(
    admitted: &AdmittedProgram<'_>,
    ty: MirType,
) -> Result<ScalarType, LowerError> {
    Ok(match ty {
        MirType::I64 => ScalarType::I64,
        MirType::U64 => ScalarType::U64,
        MirType::U8 => ScalarType::U8,
        MirType::Bool => ScalarType::Bool,
        MirType::F64 => ScalarType::F64,
        MirType::Function(id) => ScalarType::CodeAddress(
            admitted
                .function_type(id)
                .ok_or(PlanError::UnknownDeclaration)?,
        ),
        MirType::Shared(_) => ScalarType::DataAddress,
        _ => return Err(PlanError::InvalidSignature.into()),
    })
}
