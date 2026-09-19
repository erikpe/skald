use super::{
    context::{scalar_type, Lowerer},
    LowerError,
};
use crate::{
    backend::{
        lir::{AddressStride, MemoryRepresentation, Operation, ValueHandle},
        plan::{
            ArtifactId, DataKey, ObjectViewTarget, PlanError, ScalarType, SemanticType,
            SharedTarget,
        },
    },
    mir::{
        BlockId, MirPlace, MirPlaceBase, MirPlaceProjection, MirSharedTarget, MirType, StorageId,
    },
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn place_address(
        &mut self,
        block: BlockId,
        place: &MirPlace,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let (mut address, mut ty) = self.place_base(block, place.base)?;
        for projection in &place.projections {
            let (offset, projected) = match *projection {
                MirPlaceProjection::Base(class) => {
                    let SemanticType::Class(current) = ty else {
                        return Err(PlanError::InvalidDomain.into());
                    };
                    let fact = self
                        .plan()
                        .semantic()
                        .class(current)
                        .ok_or(PlanError::UnknownDeclaration)?;
                    let base = fact.base.ok_or(PlanError::InvalidDomain)?;
                    if base.class != class {
                        return Err(PlanError::InvalidDomain.into());
                    }
                    (base.offset, SemanticType::Class(base.class))
                }
                MirPlaceProjection::Field(field) => {
                    let fact = self
                        .plan()
                        .semantic()
                        .field(field)
                        .ok_or(PlanError::UnknownDeclaration)?;
                    (fact.offset, fact.ty)
                }
                MirPlaceProjection::OptionalPayload(class) => {
                    let optional = match ty {
                        SemanticType::Optional(optional) => optional,
                        _ => return Err(PlanError::InvalidDomain.into()),
                    };
                    let fact = self
                        .plan()
                        .semantic()
                        .optional(optional)
                        .ok_or(PlanError::UnknownDeclaration)?;
                    (fact.payload_offset, SemanticType::Class(class))
                }
                MirPlaceProjection::AggregateOptionalPayload(optional)
                | MirPlaceProjection::CheckedOptionalPayload(optional) => {
                    let fact = self
                        .plan()
                        .semantic()
                        .optional(optional)
                        .ok_or(PlanError::UnknownDeclaration)?;
                    (fact.payload_offset, fact.payload)
                }
                MirPlaceProjection::ArrayElement {
                    array,
                    normalized_index,
                } => {
                    let fact = self
                        .plan()
                        .semantic()
                        .array(array)
                        .ok_or(PlanError::UnknownDeclaration)?;
                    address = self.byte_offset(block, address, fact.element_offset)?;
                    let index_address = self.address(block, normalized_index)?;
                    let index = self.builder.append(
                        self.active_blocks[block.index()],
                        Operation::Load {
                            address: index_address,
                            representation: self.representation(normalized_index)?,
                        },
                    )?[0];
                    address = self.builder.append(
                        self.active_blocks[block.index()],
                        Operation::ScaledIndex {
                            base: address,
                            index,
                            stride: AddressStride::new(fact.stride, 1)?,
                        },
                    )?[0];
                    ty = fact.element;
                    continue;
                }
            };
            address = self.byte_offset(block, address, offset)?;
            ty = projected;
        }
        Ok(address)
    }

    pub(super) fn place_representation(
        &self,
        place: &MirPlace,
    ) -> Result<MemoryRepresentation, LowerError> {
        let ty = self.place_type(place)?;
        let layout = self
            .plan()
            .semantic()
            .layout(ty)
            .ok_or(PlanError::UnknownDeclaration)?;
        let layout = self.plan().layout(self.plan().layout_id(layout.index())?)?;
        let mir_ty = semantic_mir_type(ty).ok_or(PlanError::InvalidSignature)?;
        Ok(MemoryRepresentation {
            scalar: scalar_type(self.admitted, mir_ty)?,
            bytes: layout.size,
            alignment: layout.alignment,
        })
    }

    pub(super) fn place_type(&self, place: &MirPlace) -> Result<SemanticType, LowerError> {
        let mut ty = match place.base {
            MirPlaceBase::StaticField(field) | MirPlaceBase::StaticLifecycleDestination(field) => {
                semantic_type(
                    self.admitted
                        .program()
                        .static_field(field)
                        .ok_or(PlanError::UnknownDeclaration)?
                        .ty,
                )
            }
            MirPlaceBase::Storage(storage)
            | MirPlaceBase::AliasParameter(storage)
            | MirPlaceBase::CheckedView(storage)
            | MirPlaceBase::ArrayAlias(storage) => semantic_type(
                self.definition
                    .storage(storage)
                    .ok_or(PlanError::InvalidDomain)?
                    .ty,
            ),
            MirPlaceBase::SharedPointee(owner) => {
                shared_payload(shared_storage_target(self.definition, owner)?)?
            }
            MirPlaceBase::SharedAllocationPayload(storage) => semantic_type(
                self.definition
                    .storage(storage)
                    .ok_or(PlanError::InvalidDomain)?
                    .ty,
            ),
            MirPlaceBase::OptionalBoxPayload { target, .. } => {
                let fact = self
                    .plan()
                    .semantic()
                    .optional_box(target)
                    .ok_or(PlanError::UnknownDeclaration)?;
                object_view_type(fact.object_view.ok_or(PlanError::InvalidDomain)?)
            }
        };
        for projection in &place.projections {
            ty = match *projection {
                MirPlaceProjection::Base(class) => SemanticType::Class(class),
                MirPlaceProjection::Field(field) => {
                    self.plan()
                        .semantic()
                        .field(field)
                        .ok_or(PlanError::UnknownDeclaration)?
                        .ty
                }
                MirPlaceProjection::OptionalPayload(class) => SemanticType::Class(class),
                MirPlaceProjection::AggregateOptionalPayload(optional)
                | MirPlaceProjection::CheckedOptionalPayload(optional) => {
                    self.plan()
                        .semantic()
                        .optional(optional)
                        .ok_or(PlanError::UnknownDeclaration)?
                        .payload
                }
                MirPlaceProjection::ArrayElement { array, .. } => {
                    self.plan()
                        .semantic()
                        .array(array)
                        .ok_or(PlanError::UnknownDeclaration)?
                        .element
                }
            };
        }
        Ok(ty)
    }

    fn place_base(
        &mut self,
        block: BlockId,
        base: MirPlaceBase,
    ) -> Result<(ValueHandle<'plan>, SemanticType), LowerError> {
        match base {
            MirPlaceBase::StaticField(field) | MirPlaceBase::StaticLifecycleDestination(field) => {
                let ty = self
                    .admitted
                    .program()
                    .static_field(field)
                    .ok_or(PlanError::UnknownDeclaration)?
                    .ty;
                let address = self.builder.append(
                    self.active_blocks[block.index()],
                    Operation::SymbolAddress {
                        symbol: ArtifactId::Data(DataKey::Static(field)),
                        ty: ScalarType::DataAddress,
                    },
                )?[0];
                Ok((address, semantic_type(ty)))
            }
            MirPlaceBase::Storage(storage)
            | MirPlaceBase::AliasParameter(storage)
            | MirPlaceBase::ArrayAlias(storage) => {
                let ty = self
                    .definition
                    .storage(storage)
                    .ok_or(PlanError::InvalidDomain)?
                    .ty;
                Ok((self.address(block, storage)?, semantic_type(ty)))
            }
            MirPlaceBase::CheckedView(storage) => {
                let ty = self
                    .definition
                    .storage(storage)
                    .ok_or(PlanError::InvalidDomain)?
                    .ty;
                Ok((self.load_storage(block, storage)?, semantic_type(ty)))
            }
            MirPlaceBase::SharedPointee(owner) => {
                let target = shared_storage_target(self.definition, owner)?;
                let handle = self.load_storage(block, owner)?;
                let offset = self.shared_payload_offset(target)?;
                Ok((
                    self.byte_offset(block, handle, offset)?,
                    shared_payload(target)?,
                ))
            }
            MirPlaceBase::SharedAllocationPayload(storage) => {
                let ty = semantic_type(
                    self.definition
                        .storage(storage)
                        .ok_or(PlanError::InvalidDomain)?
                        .ty,
                );
                let allocation = self.load_storage(block, storage)?;
                let offset = match ty {
                    SemanticType::Class(class) => {
                        self.plan()
                            .semantic()
                            .class(class)
                            .ok_or(PlanError::UnknownDeclaration)?
                            .shared_allocation
                            .payload_offset
                    }
                    SemanticType::Optional(optional) => {
                        self.plan()
                            .semantic()
                            .optional_boxes
                            .iter()
                            .find(|fact| fact.exact_optional == Some(optional))
                            .and_then(|fact| fact.allocation)
                            .ok_or(PlanError::UnknownDeclaration)?
                            .payload_offset
                    }
                    _ => return Err(PlanError::InvalidDomain.into()),
                };
                Ok((self.byte_offset(block, allocation, offset)?, ty))
            }
            MirPlaceBase::OptionalBoxPayload { owner, target } => {
                let mut address = self.load_storage(block, owner)?;
                let fact = self
                    .plan()
                    .semantic()
                    .optional_box(target)
                    .ok_or(PlanError::UnknownDeclaration)?;
                let header = self
                    .plan()
                    .semantic()
                    .shared_header
                    .ok_or(PlanError::InvalidLayout)?;
                address = self.byte_offset(block, address, header.header_size)?;
                address = self.byte_offset(
                    block,
                    address,
                    fact.payload_offset.ok_or(PlanError::InvalidDomain)?,
                )?;
                let ty = object_view_type(fact.object_view.ok_or(PlanError::InvalidDomain)?);
                Ok((address, ty))
            }
        }
    }

    fn shared_payload_offset(&self, target: MirSharedTarget) -> Result<usize, LowerError> {
        Ok(match target {
            MirSharedTarget::Class(class) => {
                self.plan()
                    .semantic()
                    .class(class)
                    .ok_or(PlanError::UnknownDeclaration)?
                    .shared_allocation
                    .payload_offset
            }
            MirSharedTarget::OptionalBox(target) => {
                self.plan()
                    .semantic()
                    .optional_box(target)
                    .and_then(|fact| fact.allocation)
                    .ok_or(PlanError::UnknownDeclaration)?
                    .payload_offset
            }
            MirSharedTarget::Obj | MirSharedTarget::Interface(_) | MirSharedTarget::Array(_) => {
                self.plan()
                    .semantic()
                    .shared_header
                    .ok_or(PlanError::UnknownDeclaration)?
                    .header_size
            }
        })
    }
}

pub(super) fn semantic_type(ty: MirType) -> SemanticType {
    match ty {
        MirType::I64 => SemanticType::I64,
        MirType::U64 => SemanticType::U64,
        MirType::U8 => SemanticType::U8,
        MirType::F64 => SemanticType::F64,
        MirType::Bool => SemanticType::Bool,
        MirType::Function(id) => SemanticType::Function(id),
        MirType::Array(id) => SemanticType::Array(id),
        MirType::Class(id) => SemanticType::Class(id),
        MirType::Interface(id) => SemanticType::Interface(id),
        MirType::Obj => SemanticType::Obj,
        MirType::Shared(target) => SemanticType::Shared(shared_target(target)),
        MirType::Optional(id) => SemanticType::Optional(id),
        MirType::Unit => SemanticType::Unit,
    }
}

fn shared_target(target: MirSharedTarget) -> SharedTarget {
    match target {
        MirSharedTarget::Obj => SharedTarget::Obj,
        MirSharedTarget::Class(id) => SharedTarget::Class(id),
        MirSharedTarget::Interface(id) => SharedTarget::Interface(id),
        MirSharedTarget::Array(id) => SharedTarget::Array(id),
        MirSharedTarget::OptionalBox(id) => SharedTarget::OptionalBox(id),
    }
}

fn object_view_type(target: ObjectViewTarget) -> SemanticType {
    match target {
        ObjectViewTarget::Class(id) => SemanticType::Class(id),
        ObjectViewTarget::Interface(id) => SemanticType::Interface(id),
        ObjectViewTarget::Obj => SemanticType::Obj,
    }
}

fn shared_payload(target: MirSharedTarget) -> Result<SemanticType, LowerError> {
    Ok(match target {
        MirSharedTarget::Obj => SemanticType::Obj,
        MirSharedTarget::Class(id) => SemanticType::Class(id),
        MirSharedTarget::Interface(id) => SemanticType::Interface(id),
        MirSharedTarget::Array(id) => SemanticType::Array(id),
        MirSharedTarget::OptionalBox(_) => return Err(PlanError::InvalidDomain.into()),
    })
}

pub(super) fn shared_storage_target(
    definition: crate::mir::MirDefinitionRef<'_>,
    storage: StorageId,
) -> Result<MirSharedTarget, LowerError> {
    match definition
        .storage(storage)
        .ok_or(PlanError::InvalidDomain)?
        .ty
    {
        MirType::Shared(target) => Ok(target),
        _ => Err(PlanError::InvalidDomain.into()),
    }
}

fn semantic_mir_type(ty: SemanticType) -> Option<MirType> {
    Some(match ty {
        SemanticType::I64 => MirType::I64,
        SemanticType::U64 => MirType::U64,
        SemanticType::U8 => MirType::U8,
        SemanticType::F64 => MirType::F64,
        SemanticType::Bool => MirType::Bool,
        SemanticType::Function(id) => MirType::Function(id),
        SemanticType::Shared(target) => MirType::Shared(match target {
            SharedTarget::Obj => MirSharedTarget::Obj,
            SharedTarget::Class(id) => MirSharedTarget::Class(id),
            SharedTarget::Interface(id) => MirSharedTarget::Interface(id),
            SharedTarget::Array(id) => MirSharedTarget::Array(id),
            SharedTarget::OptionalBox(id) => MirSharedTarget::OptionalBox(id),
        }),
        SemanticType::Class(_)
        | SemanticType::Interface(_)
        | SemanticType::Obj
        | SemanticType::Array(_)
        | SemanticType::Optional(_)
        | SemanticType::Unit => return None,
    })
}
