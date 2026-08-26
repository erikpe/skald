//! HIR binding and object-place translation.

use super::*;
use crate::identity::BindingId;
use crate::object_path::ObjectProjection;

impl BodyLowerer<'_> {
    pub(super) fn lower_binding_place(&self, binding: BindingId) -> MirPlace {
        let storage = self.storage_for_binding(binding);
        match self.storage[storage.index()].kind {
            MirStorageKind::AliasParameter(_) => MirPlace::alias_parameter(storage),
            MirStorageKind::CheckedView(_) => MirPlace::checked_view(storage),
            _ => MirPlace::base(storage),
        }
    }

    pub(super) fn lower_field_place(&mut self, place: &crate::hir::HirFieldPlace) -> MirPlace {
        let receiver = match &place.receiver {
            crate::hir::HirObjectReceiver::Place { place, .. } => self.lower_object_place(place),
            crate::hir::HirObjectReceiver::Checked { view, .. } => {
                self.lower_checked_object_view(view).source
            }
            crate::hir::HirObjectReceiver::View { view, .. } => self.lower_object_view(view).source,
            crate::hir::HirObjectReceiver::ArrayElement {
                element,
                place: projected,
                ..
            } => project_object_path(
                self.lower_array_element_place(element),
                projected.projections(),
            ),
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
                | MirStorageKind::PrimitiveAlias
                | MirStorageKind::PathCondition
                | MirStorageKind::OptionalUnwrap
                | MirStorageKind::SharedAllocation
                | MirStorageKind::ArrayBacking
                | MirStorageKind::ArrayProduced
                | MirStorageKind::ArraySlice
                | MirStorageKind::ArrayPosition
                | MirStorageKind::ArrayAnchor(_)
                | MirStorageKind::ArrayAlias(_) => {
                    unreachable!("HIR object paths cannot use compiler-owned storage")
                }
            }
        };
        project_object_path(root, place.projections())
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

fn project_object_path(root: MirPlace, projections: &[ObjectProjection]) -> MirPlace {
    projections
        .iter()
        .fold(root, |projected, projection| match *projection {
            ObjectProjection::Field(field) => projected.project_field(field),
            ObjectProjection::Base(base) => projected.project_base(base),
        })
}
