//! HIR binding and object-place translation.

use super::*;
use crate::identity::BindingId;
use crate::object_path::ObjectProjection;

impl BodyLowerer<'_> {
    pub(super) fn lower_field_place(&mut self, place: &crate::hir::HirFieldPlace) -> MirPlace {
        let receiver = match (
            &place.checked_cast,
            &place.shared_view,
            &place.optional_view,
        ) {
            (Some(cast), None, None) => self.lower_checked_object_view(cast).source,
            (None, Some(view), None) | (None, None, Some(view)) => {
                self.lower_object_view(view).source
            }
            (None, None, None) => self.lower_object_place(&place.receiver),
            _ => unreachable!("a field carrier has one provenance"),
        };
        receiver.project_field(place.field)
    }

    pub(super) fn lower_object_place(&self, place: &crate::hir::HirObjectPlace) -> MirPlace {
        let storage = self.storage_for_binding(place.root());
        let root = if matches!(self.storage[storage.index()].ty, MirType::Shared(_)) {
            MirPlace::shared_pointee(storage)
        } else {
            match self.storage[storage.index()].kind {
                MirStorageKind::AliasParameter(_) => MirPlace::alias_parameter(storage),
                MirStorageKind::CheckedView(_) => MirPlace::checked_view(storage),
                MirStorageKind::Return
                | MirStorageKind::Receiver
                | MirStorageKind::Parameter
                | MirStorageKind::Local => MirPlace::base(storage),
                MirStorageKind::Argument
                | MirStorageKind::Temporary
                | MirStorageKind::SharedAnchor
                | MirStorageKind::ScalarSpill
                | MirStorageKind::OptionalUnwrap
                | MirStorageKind::SharedAllocation => {
                    unreachable!("HIR object paths cannot use compiler-owned storage")
                }
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
        }
    }
}
