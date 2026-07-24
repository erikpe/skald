//! MIR place validation and structural relationships.

use crate::identity::CallableId;

use super::{
    super::model::{
        MirAliasAccess, MirBasicBlock, MirDefinitionRef, MirPlace, MirPlaceBase,
        MirPlaceProjection, MirReceiverAccess, MirStorage, MirStorageKind, MirType,
    },
    context::Verifier,
};

#[derive(Clone, Copy)]
pub(super) struct VerifiedPlace {
    pub(super) ty: MirType,
    pub(super) access: MirAliasAccess,
}

impl Verifier<'_> {
    pub(super) fn verify_place(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        place: &MirPlace,
    ) -> Option<VerifiedPlace> {
        let storage_id = place.base.storage();
        let Some(storage) = function.storage(storage_id) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("place base {storage_id} is not declared in this function"),
            );
            return None;
        };
        if storage.kind == MirStorageKind::SharedAllocation {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "unpublished shared allocation storage {storage_id} cannot be used as a place"
                ),
            );
            return None;
        }
        let access = match (place.base, storage.kind) {
            (
                MirPlaceBase::Storage(_),
                MirStorageKind::AliasParameter(_) | MirStorageKind::CheckedView(_),
            ) => {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("alias parameter storage {storage_id} requires an indirect base"),
                );
                return None;
            }
            (MirPlaceBase::AliasParameter(_), MirStorageKind::AliasParameter(access)) => access,
            (MirPlaceBase::AliasParameter(_), _) => {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("indirect alias base {storage_id} is not alias parameter storage"),
                );
                return None;
            }
            (MirPlaceBase::CheckedView(_), MirStorageKind::CheckedView(access)) => access,
            (MirPlaceBase::CheckedView(_), _) => {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("checked-view base {storage_id} is not checked-view storage"),
                );
                return None;
            }
            (MirPlaceBase::SharedPointee(_), kind)
                if matches!(kind, MirStorageKind::Local | MirStorageKind::Parameter)
                    && matches!(storage.ty, MirType::Shared(_)) =>
            {
                MirAliasAccess::Mutable
            }
            (MirPlaceBase::SharedPointee(_), _) => {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!(
                        "shared-pointee base {storage_id} requires a stable local or parameter owner"
                    ),
                );
                return None;
            }
            (MirPlaceBase::Storage(_), _) => self.storage_access(function, storage),
        };
        let mut ty = match (place.base, storage.ty) {
            (MirPlaceBase::SharedPointee(_), MirType::Shared(target)) => target.ty(),
            _ => storage.ty,
        };
        for projection in &place.projections {
            match *projection {
                MirPlaceProjection::Base(base) => {
                    let MirType::Class(owner) = ty else {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("base projection {base} has a non-class base"),
                        );
                        return None;
                    };
                    if self.program.direct_base(owner) != Some(base) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!(
                                "base projection {base} is not the declared direct base of {owner}"
                            ),
                        );
                        return None;
                    }
                    ty = MirType::Class(base);
                }
                MirPlaceProjection::Field(field_id) => {
                    let MirType::Class(owner) = ty else {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("field projection {field_id} has a non-class base"),
                        );
                        return None;
                    };
                    if field_id.class() != owner {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("field projection {field_id} belongs to the wrong class"),
                        );
                        return None;
                    }
                    let Some(field) = self.program.field(field_id) else {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("field projection {field_id} is not declared"),
                        );
                        return None;
                    };
                    ty = field.ty;
                }
            }
        }
        Some(VerifiedPlace { ty, access })
    }

    pub(super) fn storage_access(
        &self,
        function: MirDefinitionRef<'_>,
        storage: &MirStorage,
    ) -> MirAliasAccess {
        if storage.kind != MirStorageKind::Receiver {
            return MirAliasAccess::Mutable;
        }
        match function.callable() {
            CallableId::Method(method) => match self
                .program
                .method(method)
                .map(|method| method.receiver_access)
            {
                Some(MirReceiverAccess::ReadOnly) => MirAliasAccess::ReadOnly,
                Some(MirReceiverAccess::Mutable) => MirAliasAccess::Mutable,
                None => MirAliasAccess::ReadOnly,
            },
            CallableId::Initializer(_)
            | CallableId::CopyConstructor(_)
            | CallableId::CopyAssignment(_)
            | CallableId::Destructor(_) => MirAliasAccess::Mutable,
            CallableId::Function(_) => MirAliasAccess::ReadOnly,
        }
    }
}

pub(super) fn is_ancestor(ancestor: &MirPlace, place: &MirPlace) -> bool {
    ancestor.base == place.base && place.projections.starts_with(&ancestor.projections)
}

pub(super) fn places_overlap(left: &MirPlace, right: &MirPlace) -> bool {
    is_ancestor(left, right) || is_ancestor(right, left)
}

#[cfg(test)]
mod tests;
