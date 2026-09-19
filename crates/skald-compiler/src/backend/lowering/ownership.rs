//! Shared-owner operations expressed through ordinary LIR memory and calls.

use super::{context::Lowerer, LowerError};
use crate::{
    backend::{
        lir::{Call, CallArgument, CallTarget, Constant, MemoryRepresentation, Operation},
        plan::{
            ArtifactId, ComponentRole, DataKey, HelperFamily, LirCallableId, PlanError,
            RuntimeService, ScalarType,
        },
    },
    mir::{
        BlockId, MirPlace, MirSharedAdopt, MirSharedAllocate, MirSharedAllocationTarget,
        MirSharedCast, MirSharedCastSource, MirSharedCastTransfer, MirSharedCopy,
        MirSharedFieldCopy, MirSharedFieldInitialize, MirSharedFieldReplace, MirSharedInitialize,
        MirSharedMove, MirSharedPublish, MirSharedRelease, MirSharedStatic, MirSharedTarget,
        MirType, MirViewTarget, StorageId,
    },
    source::Span,
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn shared_allocate(
        &mut self,
        block: BlockId,
        allocation: &MirSharedAllocate,
    ) -> Result<(), LowerError> {
        let bytes = match allocation.target {
            MirSharedAllocationTarget::Class(class) => {
                self.plan()
                    .semantic()
                    .class(class)
                    .ok_or(PlanError::UnknownDeclaration)?
                    .shared_allocation
                    .byte_count
            }
            MirSharedAllocationTarget::OptionalBox { target, optional } => {
                self.plan()
                    .semantic()
                    .optional_box(target)
                    .filter(|fact| fact.exact_optional == Some(optional))
                    .and_then(|fact| fact.allocation)
                    .ok_or(PlanError::InvalidLayout)?
                    .byte_count
            }
        };
        let bytes = self.append(block, Operation::Constant(Constant::U64(bytes)))?[0];
        let call = self.runtime_call(
            RuntimeService::Allocate,
            vec![CallArgument {
                role: ComponentRole::RuntimeParameter(0),
                value: bytes,
            }],
            block,
            allocation.span,
        )?;
        let results = self.append(block, Operation::Call(call))?;
        let [handle] = results.as_slice() else {
            return Err(PlanError::InvalidSignature.into());
        };
        self.store(block, allocation.allocation, *handle)
    }

    pub(super) fn shared_initialize(
        &mut self,
        block: BlockId,
        initialize: &MirSharedInitialize,
    ) -> Result<(), LowerError> {
        let class = initialize.target.class();
        let target = LirCallableId::Source(initialize.target.into());
        let signature = self.callable_signature(target)?;
        let roles = self
            .plan()
            .signature(self.plan().signature_id(signature.index())?)?
            .inputs
            .iter()
            .map(|component| component.role)
            .collect::<Vec<_>>();
        let handle = self.load_storage(block, initialize.allocation)?;
        let payload = self.shared_payload_address(block, handle)?;
        let metadata = self.data_address(block, DataKey::ClassDispatch(class))?;
        let arguments = roles
            .into_iter()
            .map(|role| {
                let value = match role {
                    ComponentRole::ReceiverStatic | ComponentRole::ReceiverComplete => payload,
                    ComponentRole::ReceiverMetadata => metadata,
                    _ => self.argument_component(block, &initialize.arguments, role)?,
                };
                Ok(CallArgument { role, value })
            })
            .collect::<Result<Vec<_>, LowerError>>()?;
        let attribution = self.attribution(block, initialize.span, false)?;
        self.append(
            block,
            Operation::Call(Call {
                target: CallTarget::Direct(ArtifactId::Callable(target)),
                signature,
                arguments,
                attribution,
            }),
        )?;
        Ok(())
    }

    pub(super) fn shared_publish(
        &mut self,
        block: BlockId,
        publish: &MirSharedPublish,
    ) -> Result<(), LowerError> {
        let ty = self.storage_type(publish.allocation)?;
        let metadata = match ty {
            MirType::Class(class) => DataKey::ClassDispatch(class),
            MirType::Optional(optional) => DataKey::OptionalBoxDescriptor(
                self.admitted
                    .program()
                    .exact_optional_box_type(optional)
                    .ok_or(PlanError::UnknownDeclaration)?
                    .id,
            ),
            _ => return Err(PlanError::InvalidDomain.into()),
        };
        let handle = self.load_storage(block, publish.allocation)?;
        let metadata = self.data_address(block, metadata)?;
        let header = self.shared_header()?;
        self.store_at_offset(
            block,
            handle,
            header.dynamic_metadata_offset,
            metadata,
            self.address_representation(),
        )?;
        let one = self.append(block, Operation::Constant(Constant::U64(1)))?[0];
        self.store_at_offset(
            block,
            handle,
            header.owner_count_offset,
            one,
            self.count_representation(),
        )
    }

    pub(super) fn shared_static(
        &mut self,
        block: BlockId,
        static_owner: &MirSharedStatic,
    ) -> Result<(), LowerError> {
        let handle = self.data_address(block, DataKey::Literal(static_owner.data))?;
        self.store(block, static_owner.destination, handle)
    }

    pub(super) fn shared_adopt(
        &mut self,
        block: BlockId,
        adopt: &MirSharedAdopt,
    ) -> Result<(), LowerError> {
        let handle = self.load_storage(block, adopt.allocation)?;
        self.store(block, adopt.destination, handle)
    }

    pub(super) fn shared_copy(
        &mut self,
        block: BlockId,
        copy: &MirSharedCopy,
    ) -> Result<(), LowerError> {
        let handle = self.load_storage(block, copy.source)?;
        self.retain(block, handle, copy.span)?;
        self.store(block, copy.destination, handle)
    }

    pub(super) fn shared_field_copy(
        &mut self,
        block: BlockId,
        copy: &MirSharedFieldCopy,
    ) -> Result<(), LowerError> {
        let handle = self.load_place(block, &copy.source)?;
        self.retain(block, handle, copy.span)?;
        self.store(block, copy.destination, handle)
    }

    pub(super) fn shared_cast(
        &mut self,
        block: BlockId,
        cast: &MirSharedCast,
    ) -> Result<(), LowerError> {
        let handle = self.shared_cast_source(block, &cast.source)?;
        if cast.transfer == MirSharedCastTransfer::Copy {
            self.retain(block, handle, cast.span)?;
        }
        self.store(block, cast.destination, handle)
    }

    pub(super) fn checked_shared_cast(
        &mut self,
        block: BlockId,
        cast: &MirSharedCast,
        success_target: BlockId,
        failure_target: BlockId,
    ) -> Result<(), LowerError> {
        let metadata = self.shared_cast_metadata(block, &cast.source)?;
        let condition = self.membership(block, metadata, self.shared_view(cast.target)?)?;
        let handle = self.shared_cast_source(block, &cast.source)?;
        let destination = self.address(block, cast.destination)?;
        let success = self.builder.reserve_block()?;
        self.builder.define_block(success, &[])?;

        if cast.transfer == MirSharedCastTransfer::Copy {
            let attribution = self.synthetic_attribution(success, cast.span)?;
            self.call_owner_helper_at(success, HelperFamily::Retain, handle, attribution)?;
        }
        self.builder.append(
            success,
            Operation::Store {
                address: destination,
                value: handle,
                representation: self.address_representation(),
            },
        )?;
        self.builder.terminate(
            success,
            crate::backend::lir::Terminator::Jump(self.edge(success_target)),
        )?;
        self.builder.terminate(
            self.active_blocks[block.index()],
            crate::backend::lir::Terminator::Branch {
                condition,
                true_edge: crate::backend::lir::Edge {
                    target: success,
                    arguments: vec![],
                },
                false_edge: self.edge(failure_target),
            },
        )?;
        Ok(())
    }

    pub(super) fn shared_move(
        &mut self,
        block: BlockId,
        transfer: &MirSharedMove,
    ) -> Result<(), LowerError> {
        let handle = self.load_storage(block, transfer.source)?;
        self.store(block, transfer.destination, handle)
    }

    pub(super) fn shared_release(
        &mut self,
        block: BlockId,
        release: &MirSharedRelease,
    ) -> Result<(), LowerError> {
        let handle = self.load_storage(block, release.owner)?;
        self.release(block, handle, release.span)
    }

    pub(super) fn shared_field_initialize(
        &mut self,
        block: BlockId,
        initialize: &MirSharedFieldInitialize,
    ) -> Result<(), LowerError> {
        let handle = self.load_storage(block, initialize.source)?;
        self.store_place(block, &initialize.destination, handle)
    }

    pub(super) fn shared_field_replace(
        &mut self,
        block: BlockId,
        replace: &MirSharedFieldReplace,
    ) -> Result<(), LowerError> {
        let replacement = self.load_storage(block, replace.source)?;
        let previous = self.load_place(block, &replace.destination)?;
        self.release(block, previous, replace.span)?;
        self.store_place(block, &replace.destination, replacement)
    }

    pub(super) fn shared_field_construct(
        &mut self,
        block: BlockId,
        destination: &MirPlace,
        source: &MirPlace,
        span: Span,
    ) -> Result<(), LowerError> {
        let handle = self.load_place(block, source)?;
        self.retain(block, handle, span)?;
        self.store_place(block, destination, handle)
    }

    pub(super) fn shared_field_assign(
        &mut self,
        block: BlockId,
        destination: &MirPlace,
        source: &MirPlace,
        span: Span,
    ) -> Result<(), LowerError> {
        // Securing the replacement first makes exact self-assignment and
        // overlapping object graphs safe even when releasing the old edge
        // runs arbitrary finalizers.
        let replacement = self.load_place(block, source)?;
        self.retain(block, replacement, span)?;
        let previous = self.load_place(block, destination)?;
        self.release(block, previous, span)?;
        self.store_place(block, destination, replacement)
    }

    pub(super) fn shared_cast_metadata(
        &mut self,
        block: BlockId,
        source: &MirSharedCastSource,
    ) -> Result<crate::backend::lir::ValueHandle<'plan>, LowerError> {
        let handle = self.shared_cast_source(block, source)?;
        self.load_at_offset(
            block,
            handle,
            self.shared_header()?.dynamic_metadata_offset,
            self.address_representation(),
        )
    }

    fn shared_cast_source(
        &mut self,
        block: BlockId,
        source: &MirSharedCastSource,
    ) -> Result<crate::backend::lir::ValueHandle<'plan>, LowerError> {
        match source {
            MirSharedCastSource::Owner { storage, .. } => self.load_storage(block, *storage),
            MirSharedCastSource::Field { place, .. } => self.load_place(block, place),
        }
    }

    fn retain(
        &mut self,
        block: BlockId,
        handle: crate::backend::lir::ValueHandle<'plan>,
        span: Span,
    ) -> Result<(), LowerError> {
        self.call_owner_helper(block, HelperFamily::Retain, handle, span)
    }

    fn release(
        &mut self,
        block: BlockId,
        handle: crate::backend::lir::ValueHandle<'plan>,
        span: Span,
    ) -> Result<(), LowerError> {
        self.call_owner_helper(block, HelperFamily::Release, handle, span)
    }

    fn call_owner_helper(
        &mut self,
        block: BlockId,
        family: HelperFamily,
        handle: crate::backend::lir::ValueHandle<'plan>,
        span: Span,
    ) -> Result<(), LowerError> {
        let target = owner_helper(self.plan(), family)?;
        let signature = self.callable_signature(target)?;
        let attribution = self.attribution(block, span, false)?;
        self.append(
            block,
            Operation::Call(Call {
                target: CallTarget::Direct(ArtifactId::Callable(target)),
                signature,
                arguments: vec![CallArgument {
                    role: ComponentRole::Parameter(0),
                    value: handle,
                }],
                attribution,
            }),
        )?;
        Ok(())
    }

    pub(super) fn call_owner_helper_at(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        family: HelperFamily,
        handle: crate::backend::lir::ValueHandle<'plan>,
        attribution: crate::backend::lir::CallAttribution,
    ) -> Result<(), LowerError> {
        let target = owner_helper(self.plan(), family)?;
        let signature = self.callable_signature(target)?;
        self.builder.append(
            block,
            Operation::Call(Call {
                target: CallTarget::Direct(ArtifactId::Callable(target)),
                signature,
                arguments: vec![CallArgument {
                    role: ComponentRole::Parameter(0),
                    value: handle,
                }],
                attribution,
            }),
        )?;
        Ok(())
    }

    fn runtime_call(
        &mut self,
        service: RuntimeService,
        arguments: Vec<CallArgument<crate::backend::lir::ValueHandle<'plan>>>,
        block: BlockId,
        span: Span,
    ) -> Result<Call<crate::backend::lir::ValueHandle<'plan>>, LowerError> {
        let target = ArtifactId::Runtime(service);
        let signature = self
            .plan()
            .artifact(self.plan().artifact_id(target)?, target.category())?
            .signature
            .ok_or(PlanError::InvalidSignature)?;
        Ok(Call {
            target: CallTarget::Direct(target),
            signature,
            arguments,
            attribution: self.attribution(block, span, false)?,
        })
    }

    pub(super) fn load_place(
        &mut self,
        block: BlockId,
        place: &MirPlace,
    ) -> Result<crate::backend::lir::ValueHandle<'plan>, LowerError> {
        let address = self.place_address(block, place)?;
        Ok(self.append(
            block,
            Operation::Load {
                address,
                representation: self.address_representation(),
            },
        )?[0])
    }

    pub(super) fn store_place(
        &mut self,
        block: BlockId,
        place: &MirPlace,
        value: crate::backend::lir::ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        let address = self.place_address(block, place)?;
        self.append(
            block,
            Operation::Store {
                address,
                value,
                representation: self.address_representation(),
            },
        )?;
        Ok(())
    }

    fn data_address(
        &mut self,
        block: BlockId,
        key: DataKey,
    ) -> Result<crate::backend::lir::ValueHandle<'plan>, LowerError> {
        Ok(self.append(
            block,
            Operation::SymbolAddress {
                symbol: ArtifactId::Data(key),
                ty: ScalarType::DataAddress,
            },
        )?[0])
    }

    fn shared_payload_address(
        &mut self,
        block: BlockId,
        handle: crate::backend::lir::ValueHandle<'plan>,
    ) -> Result<crate::backend::lir::ValueHandle<'plan>, LowerError> {
        self.byte_offset(block, handle, self.shared_header()?.header_size)
    }

    fn load_at_offset(
        &mut self,
        block: BlockId,
        base: crate::backend::lir::ValueHandle<'plan>,
        offset: usize,
        representation: MemoryRepresentation,
    ) -> Result<crate::backend::lir::ValueHandle<'plan>, LowerError> {
        let address = self.byte_offset(block, base, offset)?;
        Ok(self.append(
            block,
            Operation::Load {
                address,
                representation,
            },
        )?[0])
    }

    fn store_at_offset(
        &mut self,
        block: BlockId,
        base: crate::backend::lir::ValueHandle<'plan>,
        offset: usize,
        value: crate::backend::lir::ValueHandle<'plan>,
        representation: MemoryRepresentation,
    ) -> Result<(), LowerError> {
        let address = self.byte_offset(block, base, offset)?;
        self.append(
            block,
            Operation::Store {
                address,
                value,
                representation,
            },
        )?;
        Ok(())
    }

    fn storage_type(&self, storage: StorageId) -> Result<MirType, LowerError> {
        self.definition
            .storage(storage)
            .map(|storage| storage.ty)
            .ok_or_else(|| PlanError::InvalidDomain.into())
    }

    fn shared_header(&self) -> Result<crate::backend::plan::SharedHeaderLayout, LowerError> {
        self.plan()
            .semantic()
            .shared_header
            .ok_or_else(|| PlanError::InvalidLayout.into())
    }

    fn count_representation(&self) -> MemoryRepresentation {
        MemoryRepresentation {
            scalar: ScalarType::U64,
            bytes: 8,
            alignment: 8,
        }
    }

    fn shared_view(&self, target: MirSharedTarget) -> Result<MirViewTarget, LowerError> {
        Ok(match target {
            MirSharedTarget::Class(class) => MirViewTarget::Class(class),
            MirSharedTarget::Interface(interface) => MirViewTarget::Interface(interface),
            MirSharedTarget::Obj => MirViewTarget::Obj,
            MirSharedTarget::OptionalBox(optional_box) => match self
                .plan()
                .semantic()
                .optional_box(optional_box)
                .and_then(|fact| fact.object_view)
                .ok_or(PlanError::InvalidDomain)?
            {
                crate::backend::plan::ObjectViewTarget::Class(class) => MirViewTarget::Class(class),
                crate::backend::plan::ObjectViewTarget::Interface(interface) => {
                    MirViewTarget::Interface(interface)
                }
                crate::backend::plan::ObjectViewTarget::Obj => MirViewTarget::Obj,
            },
            MirSharedTarget::Array(_) => return Err(PlanError::InvalidDomain.into()),
        })
    }

    fn append(
        &mut self,
        block: BlockId,
        operation: Operation<
            crate::backend::lir::ValueHandle<'plan>,
            crate::backend::lir::ObjectHandle<'plan>,
            crate::backend::lir::BlockHandle<'plan>,
        >,
    ) -> Result<Vec<crate::backend::lir::ValueHandle<'plan>>, LowerError> {
        Ok(self
            .builder
            .append(self.active_blocks[block.index()], operation)?)
    }
}

pub(super) fn owner_helper(
    plan: crate::backend::plan::PlanView<'_>,
    family: HelperFamily,
) -> Result<LirCallableId, LowerError> {
    let layout = plan
        .semantic()
        .shared_header
        .ok_or(PlanError::InvalidLayout)?
        .handle_layout;
    plan.resources()
        .generated
        .iter()
        .find_map(|fact| match fact.callable {
            LirCallableId::Helper(key) if key.family == family && key.layout == layout => {
                Some(fact.callable)
            }
            _ => None,
        })
        .ok_or_else(|| PlanError::UnknownDeclaration.into())
}
