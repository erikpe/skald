//! HIR binding and object-place translation.

use super::*;
use crate::identity::BindingId;
use crate::object_path::ObjectProjection;

impl BodyLowerer<'_> {
    pub(super) fn lower_field_place(&self, place: &crate::hir::HirFieldPlace) -> MirPlace {
        self.lower_object_place(&place.receiver)
            .project_field(place.field)
    }

    pub(super) fn lower_object_place(&self, place: &crate::hir::HirObjectPlace) -> MirPlace {
        let storage = self.storage_for_binding(place.root());
        let root = match self.storage[storage.index()].kind {
            MirStorageKind::AliasParameter(_) => MirPlace::alias_parameter(storage),
            MirStorageKind::NarrowedAlias(_) => MirPlace::narrowed_alias(storage),
            MirStorageKind::Return
            | MirStorageKind::Receiver
            | MirStorageKind::Parameter
            | MirStorageKind::Local => MirPlace::base(storage),
            MirStorageKind::Argument | MirStorageKind::Temporary => {
                unreachable!("HIR object paths cannot use compiler-owned storage")
            }
        };
        place
            .projections()
            .iter()
            .fold(root, |projected, projection| match *projection {
                ObjectProjection::Field(field) => projected.project_field(field),
                ObjectProjection::Base(base) => projected.project_base(base),
            })
    }

    pub(super) fn storage_for_binding(&self, binding: BindingId) -> StorageId {
        assert_eq!(
            binding.callable(),
            self.input.callable,
            "typed binding must belong to the current callable"
        );
        match binding {
            BindingId::Receiver(_) => self
                .receiver_storage
                .expect("receiver binding requires member receiver storage"),
            BindingId::Parameter(id) => self.parameter_storage[id.index()],
            BindingId::Local(id) => self.local_storage[id.index()],
            BindingId::NarrowedAlias(id) => self.narrowed_alias_storage[id.index()],
        }
    }
}
