//! Logical calls are assembled from semantic component roles. Target ABI
//! classification remains wholly owned by selection.

use super::{context::Lowerer, LowerError};
use crate::{
    backend::{
        lir::{Call, CallArgument, CallTarget, Operation, ValueHandle},
        plan::{ArtifactId, ComponentRole, LirCallableId, PlanError},
    },
    mir::{
        BlockId, MirArgument, MirCall, MirCallReceiver, MirCallTarget, MirFunctionLinkage,
        MirMethodCallTarget, MirObjectView,
    },
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn call(&mut self, block: BlockId, source: &MirCall) -> Result<(), LowerError> {
        let (target, signature) = self.call_target(source)?;
        let roles = self
            .plan()
            .signature(self.plan().signature_id(signature.index())?)?
            .inputs
            .iter()
            .map(|component| component.role)
            .collect::<Vec<_>>();
        let arguments = roles
            .into_iter()
            .map(|role| {
                Ok(CallArgument {
                    role,
                    value: self.call_component(block, source, role)?,
                })
            })
            .collect::<Result<Vec<_>, LowerError>>()?;
        let attribution = self.attribution(block, source.span, false)?;
        let call = Call {
            target,
            signature,
            arguments,
            attribution,
        };
        if let Some(storage) = source.shared_result {
            let produced = self
                .builder
                .append(self.blocks[block.index()], Operation::Call(call))?;
            let [value] = produced.as_slice() else {
                return Err(PlanError::InvalidSignature.into());
            };
            self.store(block, storage, *value)?;
        } else {
            let results = source
                .result
                .iter()
                .map(|value| self.values[value.index()])
                .collect::<Vec<_>>();
            self.builder.append_into(
                self.blocks[block.index()],
                Operation::Call(call),
                &results,
            )?;
        }
        Ok(())
    }

    fn call_target(
        &self,
        source: &MirCall,
    ) -> Result<
        (
            CallTarget<ValueHandle<'plan>>,
            crate::backend::plan::SignatureId,
        ),
        LowerError,
    > {
        let artifact = match source.target {
            MirCallTarget::Direct(id) => {
                let declaration = self
                    .admitted
                    .program()
                    .declarations
                    .get(id)
                    .ok_or(PlanError::UnknownDeclaration)?;
                match declaration.linkage {
                    MirFunctionLinkage::External { link } => ArtifactId::External(link),
                    _ => ArtifactId::Callable(LirCallableId::Source(id.into())),
                }
            }
            MirCallTarget::Static(id) | MirCallTarget::Method(MirMethodCallTarget::Direct(id)) => {
                ArtifactId::Callable(LirCallableId::Source(id.into()))
            }
            MirCallTarget::Indirect(target) => {
                return Ok((
                    CallTarget::Indirect(self.values[target.callee.index()]),
                    self.admitted
                        .function_type(target.function_type)
                        .ok_or(PlanError::UnknownDeclaration)?,
                ))
            }
            MirCallTarget::Method(MirMethodCallTarget::Virtual { .. })
            | MirCallTarget::Interface(_) => return Err(PlanError::InvalidDomain.into()),
        };
        let signature = self
            .plan()
            .artifact(self.plan().artifact_id(artifact)?, artifact.category())?
            .signature
            .ok_or(PlanError::InvalidSignature)?;
        Ok((CallTarget::Direct(artifact), signature))
    }

    fn call_component(
        &mut self,
        block: BlockId,
        source: &MirCall,
        role: ComponentRole,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        match role {
            ComponentRole::ResultDestination(_) => self.place_address(
                block,
                source
                    .destination
                    .as_ref()
                    .ok_or(PlanError::InvalidSignature)?,
            ),
            ComponentRole::ReceiverStatic => self.place_address(block, receiver(source)?.source()),
            ComponentRole::ReceiverComplete => Ok(self
                .object_origin(block, receiver(source)?.origin())?
                .complete),
            ComponentRole::ReceiverMetadata => Ok(self
                .object_origin(block, receiver(source)?.origin())?
                .metadata),
            ComponentRole::Parameter(index) => match argument(source, index)? {
                MirArgument::Value(value) => Ok(self.values[value.index()]),
                MirArgument::SharedOwner(owner) => self.load_call_storage(block, *owner),
                _ => Err(PlanError::InvalidSignature.into()),
            },
            ComponentRole::AggregateAddress { parameter, .. } => {
                match argument(source, parameter)? {
                    MirArgument::Place(place) | MirArgument::OwnedPlace(place) => {
                        self.place_address(block, place)
                    }
                    _ => Err(PlanError::InvalidSignature.into()),
                }
            }
            ComponentRole::AliasAddress(index) => match argument(source, index)? {
                MirArgument::Place(place) => self.place_address(block, place),
                MirArgument::View(view) => self.place_address(block, &view.source),
                _ => Err(PlanError::InvalidSignature.into()),
            },
            ComponentRole::AliasComplete(index) => {
                Ok(self.alias_origin(block, argument(source, index)?)?.complete)
            }
            ComponentRole::AliasMetadata(index) => {
                Ok(self.alias_origin(block, argument(source, index)?)?.metadata)
            }
            ComponentRole::RuntimeParameter(_) | ComponentRole::Result => {
                Err(PlanError::InvalidSignature.into())
            }
        }
    }

    fn alias_origin(
        &mut self,
        block: BlockId,
        argument: &MirArgument,
    ) -> Result<super::context::ObjectOriginValues<'plan>, LowerError> {
        match argument {
            MirArgument::Place(place) => self.inferred_origin(block, place),
            MirArgument::View(view) => self.object_origin(block, &view.origin),
            _ => Err(PlanError::InvalidSignature.into()),
        }
    }

    fn load_call_storage(
        &mut self,
        block: BlockId,
        storage: crate::mir::StorageId,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let address = self.address(block, storage)?;
        let representation = self.representation(storage)?;
        Ok(self.builder.append(
            self.blocks[block.index()],
            Operation::Load {
                address,
                representation,
            },
        )?[0])
    }
}

fn argument(source: &MirCall, index: usize) -> Result<&MirArgument, LowerError> {
    source
        .arguments
        .get(index)
        .ok_or_else(|| PlanError::InvalidSignature.into())
}

enum Receiver<'mir> {
    Method(&'mir crate::mir::MirMethodReceiver),
    Interface(&'mir MirObjectView),
}

impl Receiver<'_> {
    fn source(&self) -> &crate::mir::MirPlace {
        match self {
            Self::Method(receiver) => &receiver.place,
            Self::Interface(view) => &view.source,
        }
    }

    fn origin(&self) -> &crate::mir::MirObjectOrigin {
        match self {
            Self::Method(receiver) => &receiver.origin,
            Self::Interface(view) => &view.origin,
        }
    }
}

fn receiver(source: &MirCall) -> Result<Receiver<'_>, LowerError> {
    match source
        .receiver
        .as_ref()
        .ok_or(PlanError::InvalidSignature)?
    {
        MirCallReceiver::Method(receiver) => Ok(Receiver::Method(receiver)),
        MirCallReceiver::Interface(view) => Ok(Receiver::Interface(view)),
    }
}
