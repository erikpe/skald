use super::{LowerError, PendingFeature};
use crate::backend::pilot::AdmittedPilot;
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
    pub(super) admitted: &'plan AdmittedPilot<'input>,
    pub(super) definition: MirDefinitionRef<'plan>,
    pub(super) builder: DraftBuilder<'plan>,
    pub(super) blocks: Vec<BlockHandle<'plan>>,
    pub(super) values: Vec<ValueHandle<'plan>>,
    pub(super) objects: Vec<ObjectHandle<'plan>>,
    // Computed addresses are block-local, even though the underlying object
    // represents semantic storage across blocks and dynamic lifetime epochs.
    pub(super) addresses: BTreeMap<(BlockId, StorageId), ValueHandle<'plan>>,
}

impl<'plan, 'input> Lowerer<'plan, 'input> {
    pub(super) fn new(
        admitted: &'plan AdmittedPilot<'input>,
        owner: CallableBinding<'plan>,
    ) -> Result<Self, LowerError> {
        admitted
            .plan()
            .view()
            .require_same_context(owner.context())?;
        let LirCallableId::Source(callable) = owner.key() else {
            return Err(LowerError::Pending {
                callable: owner.key(),
                feature: PendingFeature::Entry,
            });
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
            addresses: BTreeMap::new(),
        };
        lowerer.declare_storage()?;
        let entry = definition.body().entry;
        let inputs = lowerer.builder.inputs().collect::<Vec<_>>();
        for (input, component) in inputs.into_iter().zip(&owner.signature()?.inputs) {
            let ComponentRole::Parameter(index) = component.role else {
                return Err(PlanError::InvalidSignature.into());
            };
            let storage = definition.parameters()[index];
            lowerer.store(entry, storage, input)?;
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
    admitted: &'plan AdmittedPilot<'_>,
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
    admitted: &AdmittedPilot<'_>,
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
        _ => return Err(PlanError::InvalidSignature.into()),
    })
}
