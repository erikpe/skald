use super::{
    context::{scalar_type, Lowerer, ObjectOriginValues},
    LowerError,
};
use crate::{
    backend::{
        lir::{
            Constant, LifetimeDisposition, LifetimeMarker, MemoryRepresentation, Object,
            ObjectRole, Operation, ValueHandle,
        },
        plan::{PlanError, ScalarType},
    },
    mir::{BlockId, MirInstruction, MirPlace, MirPlaceBase, MirType, StorageId},
};

impl<'plan> Lowerer<'plan, '_> {
    pub(super) fn declare_storage(&mut self) -> Result<(), LowerError> {
        let explicit_lifetimes = self
            .definition
            .body()
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .filter_map(|i| match i {
                MirInstruction::StorageLive(live) => Some(live.storage),
                MirInstruction::StorageDead(dead) => Some(dead.storage),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        for storage in self.definition.storage_entries() {
            let address_carrier = matches!(
                storage.kind,
                crate::mir::MirStorageKind::SharedAllocation
                    | crate::mir::MirStorageKind::CheckedView(_)
            ) || matches!(storage.ty, MirType::Shared(_));
            let id = if address_carrier {
                self.plan()
                    .semantic()
                    .shared_header
                    .ok_or(PlanError::UnknownDeclaration)?
                    .handle_layout
            } else {
                self.admitted
                    .layout(storage.ty)
                    .ok_or(PlanError::UnknownDeclaration)?
            };
            let layout = *self.plan().layout(self.plan().layout_id(id.index())?)?;
            let explicit_lifetime =
                storage.ty != MirType::Unit && explicit_lifetimes.contains(&storage.id);
            // One source storage identity has one lexical lifetime site. Loop
            // iterations and alternate exits repeat that site's dynamic epochs.
            let lifetime = if explicit_lifetime {
                LifetimeDisposition::Sites(1)
            } else {
                LifetimeDisposition::WholeCallable
            };
            let caller_addressed = matches!(
                storage.kind,
                crate::mir::MirStorageKind::Receiver
                    | crate::mir::MirStorageKind::AliasParameter(_)
            ) || (matches!(
                storage.kind,
                crate::mir::MirStorageKind::Return | crate::mir::MirStorageKind::Parameter
            ) && scalar_type(self.admitted, storage.ty).is_err());
            self.objects.push(if caller_addressed {
                None
            } else {
                Some(self.builder.declare_object(Object {
                    layout,
                    role: ObjectRole::SemanticStorage,
                    lifetime,
                    origin: Some(storage.span),
                })?)
            });
        }
        Ok(())
    }
    pub(super) fn representation(
        &self,
        storage: StorageId,
    ) -> Result<MemoryRepresentation, LowerError> {
        let storage = self
            .definition
            .storage(storage)
            .expect("verified local storage");
        if matches!(
            storage.kind,
            crate::mir::MirStorageKind::SharedAllocation
                | crate::mir::MirStorageKind::CheckedView(_)
        ) || matches!(storage.ty, MirType::Shared(_))
        {
            return Ok(self.address_representation());
        }
        let ty = storage.ty;
        let id = self
            .admitted
            .layout(ty)
            .ok_or(PlanError::UnknownDeclaration)?;
        let layout = self.plan().layout(self.plan().layout_id(id.index())?)?;
        Ok(MemoryRepresentation {
            scalar: scalar_type(self.admitted, ty)?,
            bytes: layout.size,
            alignment: layout.alignment,
        })
    }
    pub(super) fn address(
        &mut self,
        block: BlockId,
        storage: StorageId,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        if let Some(address) = self.entry_addresses.get(&storage) {
            Ok(*address)
        } else {
            // One MIR instruction may expand into an internal CFG. Define a
            // fresh symbolic address at the active LIR block so it dominates
            // every use in that path; caching by MIR block is insufficient.
            let object = self.objects[storage.index()].ok_or(PlanError::InvalidDomain)?;
            Ok(self.builder.append(
                self.active_blocks[block.index()],
                Operation::ObjectAddress(object),
            )?[0])
        }
    }
    pub(super) fn store(
        &mut self,
        block: BlockId,
        storage: StorageId,
        value: ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        let address = self.address(block, storage)?;
        let representation = self.representation(storage)?;
        self.builder.append(
            self.active_blocks[block.index()],
            Operation::Store {
                address,
                value,
                representation,
            },
        )?;
        Ok(())
    }
    pub(super) fn lifetime(
        &mut self,
        block: BlockId,
        storage: StorageId,
        marker: LifetimeMarker,
    ) -> Result<(), LowerError> {
        if self
            .definition
            .storage(storage)
            .expect("verified local storage")
            .ty
            != MirType::Unit
        {
            let object = self.objects[storage.index()].ok_or(PlanError::InvalidDomain)?;
            self.builder.append(
                self.active_blocks[block.index()],
                Operation::Lifetime {
                    marker,
                    object,
                    site: 0,
                },
            )?;
        }
        Ok(())
    }

    pub(super) fn bind_entry_address(
        &mut self,
        storage: StorageId,
        value: ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        if self.entry_addresses.insert(storage, value).is_some() {
            return Err(PlanError::InvalidSignature.into());
        }
        Ok(())
    }

    pub(super) fn bind_origin_complete(
        &mut self,
        storage: StorageId,
        value: ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        let origin = self.object_origins.entry(storage).or_default();
        if origin.complete.replace(value).is_some() {
            return Err(PlanError::InvalidSignature.into());
        }
        Ok(())
    }

    pub(super) fn bind_origin_metadata(
        &mut self,
        storage: StorageId,
        value: ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        let origin = self.object_origins.entry(storage).or_default();
        if origin.metadata.replace(value).is_some() {
            return Err(PlanError::InvalidSignature.into());
        }
        Ok(())
    }

    pub(super) fn bind_origin(
        &mut self,
        storage: StorageId,
        origin: ObjectOriginValues<'plan>,
    ) -> Result<(), LowerError> {
        if self.object_origins.contains_key(&storage) {
            return Err(PlanError::InvalidSignature.into());
        }
        self.object_origins.insert(
            storage,
            super::context::EntryObjectOrigin {
                complete: Some(origin.complete),
                metadata: Some(origin.metadata),
            },
        );
        Ok(())
    }

    pub(super) fn entry_origin(
        &self,
        storage: StorageId,
    ) -> Result<ObjectOriginValues<'plan>, LowerError> {
        let origin = self
            .object_origins
            .get(&storage)
            .ok_or(PlanError::InvalidSignature)?;
        Ok(ObjectOriginValues {
            complete: origin.complete.ok_or(PlanError::InvalidSignature)?,
            metadata: origin.metadata.ok_or(PlanError::InvalidSignature)?,
        })
    }

    pub(super) fn load_storage(
        &mut self,
        block: BlockId,
        storage: StorageId,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let address = self.address(block, storage)?;
        Ok(self.builder.append(
            self.active_blocks[block.index()],
            Operation::Load {
                address,
                representation: self.representation(storage)?,
            },
        )?[0])
    }

    pub(super) fn byte_offset(
        &mut self,
        block: BlockId,
        base: ValueHandle<'plan>,
        bytes: usize,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        if bytes == 0 {
            return Ok(base);
        }
        let offset = self.builder.append(
            self.active_blocks[block.index()],
            Operation::Constant(Constant::U64(
                bytes.try_into().map_err(|_| PlanError::SizeOverflow)?,
            )),
        )?[0];
        Ok(self.builder.append(
            self.active_blocks[block.index()],
            Operation::ByteOffset { base, offset },
        )?[0])
    }

    pub(super) fn address_representation(&self) -> MemoryRepresentation {
        let layout = self.plan().profile().data_layout;
        MemoryRepresentation {
            scalar: ScalarType::DataAddress,
            bytes: layout.pointer_bytes,
            alignment: layout.pointer_alignment,
        }
    }
}

pub(super) fn local(place: &MirPlace) -> Result<StorageId, LowerError> {
    if let MirPlaceBase::Storage(storage) = place.base {
        if place.projections.is_empty() {
            return Ok(storage);
        }
    }
    Err(PlanError::InvalidDomain.into())
}
