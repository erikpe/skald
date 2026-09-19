use super::{
    context::{Lowerer, ObjectOriginValues},
    place::shared_storage_target,
    LowerError,
};
use crate::{
    backend::{
        lir::{Operation, ValueHandle},
        plan::{ArtifactId, DataKey, PlanError, ScalarType, SemanticType},
    },
    mir::{BlockId, MirObjectOrigin, MirPlace, MirPlaceProjection, MirSharedTarget, MirType},
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn object_origin(
        &mut self,
        block: BlockId,
        origin: &MirObjectOrigin,
    ) -> Result<ObjectOriginValues<'plan>, LowerError> {
        match origin {
            MirObjectOrigin::Exact {
                complete,
                dynamic_class,
            } => Ok(ObjectOriginValues {
                complete: self.place_address(block, complete)?,
                metadata: self.metadata_address(block, *dynamic_class)?,
            }),
            MirObjectOrigin::Forwarded { carrier, .. } => self.entry_origin(*carrier),
            MirObjectOrigin::Shared { owner, .. } => {
                let handle = self.load_storage(block, *owner)?;
                let header = *self
                    .plan()
                    .semantic()
                    .shared_header
                    .as_ref()
                    .ok_or(PlanError::UnknownDeclaration)?;
                let complete_place = match shared_storage_target(self.definition, *owner)? {
                    MirSharedTarget::OptionalBox(target) => {
                        MirPlace::optional_box_payload(*owner, target)
                    }
                    _ => MirPlace::shared_pointee(*owner),
                };
                let metadata_address =
                    self.byte_offset(block, handle, header.dynamic_metadata_offset)?;
                let metadata = self.builder.append(
                    self.active_blocks[block.index()],
                    Operation::Load {
                        address: metadata_address,
                        representation: self.address_representation(),
                    },
                )?[0];
                Ok(ObjectOriginValues {
                    complete: self.place_address(block, &complete_place)?,
                    metadata,
                })
            }
        }
    }

    pub(super) fn inferred_origin(
        &mut self,
        block: BlockId,
        place: &MirPlace,
    ) -> Result<ObjectOriginValues<'plan>, LowerError> {
        let mut exact = None;
        for (index, projection) in place.projections.iter().enumerate() {
            let class = match *projection {
                MirPlaceProjection::Base(_) => None,
                MirPlaceProjection::Field(field) => {
                    match self.plan().semantic().field(field).map(|f| f.ty) {
                        Some(SemanticType::Class(class)) => Some(class),
                        _ => None,
                    }
                }
                MirPlaceProjection::OptionalPayload(class) => Some(class),
                MirPlaceProjection::AggregateOptionalPayload(optional)
                | MirPlaceProjection::CheckedOptionalPayload(optional) => {
                    match self.plan().semantic().optional(optional).map(|f| f.payload) {
                        Some(SemanticType::Class(class)) => Some(class),
                        _ => None,
                    }
                }
                MirPlaceProjection::ArrayElement { array, .. } => {
                    match self.plan().semantic().array(array).map(|f| f.element) {
                        Some(SemanticType::Class(class)) => Some(class),
                        _ => None,
                    }
                }
            };
            if let Some(class) = class {
                let mut complete = place.clone();
                complete.projections.truncate(index + 1);
                exact = Some((complete, class));
            }
        }
        if let Some((complete, class)) = exact {
            return self.object_origin(
                block,
                &MirObjectOrigin::Exact {
                    complete,
                    dynamic_class: class,
                },
            );
        }
        if let Some(storage) = place.base.local_storage() {
            if self.object_origins.contains_key(&storage) {
                return self.entry_origin(storage);
            }
            if let MirType::Class(class) = self
                .definition
                .storage(storage)
                .ok_or(PlanError::InvalidDomain)?
                .ty
            {
                let complete = MirPlace::base(storage);
                return self.object_origin(
                    block,
                    &MirObjectOrigin::Exact {
                        complete,
                        dynamic_class: class,
                    },
                );
            }
        }
        if let Some(field) = place.base.static_field() {
            if let MirType::Class(class) = self
                .admitted
                .program()
                .static_field(field)
                .ok_or(PlanError::UnknownDeclaration)?
                .ty
            {
                let complete = MirPlace {
                    base: place.base,
                    projections: vec![],
                };
                return self.object_origin(
                    block,
                    &MirObjectOrigin::Exact {
                        complete,
                        dynamic_class: class,
                    },
                );
            }
        }
        Err(PlanError::InvalidDomain.into())
    }

    fn metadata_address(
        &mut self,
        block: BlockId,
        class: crate::identity::ClassId,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        Ok(self.builder.append(
            self.active_blocks[block.index()],
            Operation::SymbolAddress {
                symbol: ArtifactId::Data(DataKey::ClassDispatch(class)),
                ty: ScalarType::DataAddress,
            },
        )?[0])
    }
}
