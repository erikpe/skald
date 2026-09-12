//! Places, projections, object views, receivers, and object origins.

macro_rules! define_place_traversal {
    (($($mir_mutability:tt)*)) => {
        #[derive(Clone, Copy)]
        enum MirPlaceUseContext {
            OrdinaryRead,
            OrdinaryWrite(MirStorageWriteAuthorization),
            Alias,
            Call,
            OwnershipOrLifecycle,
            InputOutput,
            OtherExecutable,
        }

        impl MirPlaceUseContext {
            const fn storage_role(self, place: MirStoragePlaceUse) -> MirStorageUseRole {
                match self {
                    Self::OrdinaryRead => MirStorageUseRole::OrdinaryRead(place),
                    Self::OrdinaryWrite(authorization) => MirStorageUseRole::OrdinaryWrite {
                        place,
                        authorization,
                    },
                    Self::Alias => MirStorageUseRole::Alias,
                    Self::Call => MirStorageUseRole::Call,
                    Self::OwnershipOrLifecycle => MirStorageUseRole::OwnershipOrLifecycle,
                    Self::InputOutput => MirStorageUseRole::InputOutput,
                    Self::OtherExecutable => MirStorageUseRole::OtherExecutable,
                }
            }
        }

        fn map_place<M: MirLocalIdentityMapper>(
            place: &$($mir_mutability)* MirPlace,
            mapper: &mut M,
            site: MirLocalIdentitySite,
            context: MirPlaceUseContext,
        ) -> Result<(), M::Error> {
            let MirPlace { base, projections } = place;
            let place_use = if !projections.is_empty() {
                MirStoragePlaceUse::Projected
            } else if matches!(base, MirPlaceBase::Storage(_)) {
                MirStoragePlaceUse::ExactBase
            } else {
                MirStoragePlaceUse::Alias
            };
            let role = context.storage_role(place_use);
            match base {
                MirPlaceBase::StaticField(_) | MirPlaceBase::StaticLifecycleDestination(_) => {}
                MirPlaceBase::Storage(storage)
                | MirPlaceBase::AliasParameter(storage)
                | MirPlaceBase::CheckedView(storage)
                | MirPlaceBase::ArrayAlias(storage)
                | MirPlaceBase::SharedPointee(storage)
                | MirPlaceBase::SharedAllocationPayload(storage) => {
                    map_storage_use(mapper, site, role, storage)?;
                }
                MirPlaceBase::OptionalBoxPayload { owner, target: _ } => {
                    map_storage_use(mapper, site, role, owner)?;
                }
            }
            for projection in projections {
                match projection {
                    MirPlaceProjection::Base(_)
                    | MirPlaceProjection::Field(_)
                    | MirPlaceProjection::OptionalPayload(_)
                    | MirPlaceProjection::AggregateOptionalPayload(_)
                    | MirPlaceProjection::CheckedOptionalPayload(_) => {}
                    MirPlaceProjection::ArrayElement {
                        array: _,
                        normalized_index,
                    } => map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OtherExecutable,
                        normalized_index,
                    )?,
                }
            }
            Ok(())
        }

        fn map_object_view<M: MirLocalIdentityMapper>(
            view: &$($mir_mutability)* MirObjectView,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirObjectView {
                source,
                origin,
                target: _,
                access: _,
                provenance: _,
                span: _,
            } = view;
            map_place(source, mapper, site, MirPlaceUseContext::Alias)?;
            map_object_origin(origin, mapper, site)
        }

        fn map_method_receiver<M: MirLocalIdentityMapper>(
            receiver: &$($mir_mutability)* MirMethodReceiver,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirMethodReceiver {
                place,
                origin,
                access: _,
                provenance: _,
            } = receiver;
            map_place(place, mapper, site, MirPlaceUseContext::Alias)?;
            map_object_origin(origin, mapper, site)
        }

        fn map_object_origin<M: MirLocalIdentityMapper>(
            origin: &$($mir_mutability)* MirObjectOrigin,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            match origin {
                MirObjectOrigin::Exact {
                    complete,
                    dynamic_class: _,
                } => map_place(complete, mapper, site, MirPlaceUseContext::Alias),
                MirObjectOrigin::Forwarded {
                    carrier,
                    static_target: _,
                    access: _,
                    dispatch_limit: _,
                    span: _,
                } => map_storage_use(mapper, site, MirStorageUseRole::Alias, carrier),
                MirObjectOrigin::Shared {
                    owner,
                    static_target: _,
                    access: _,
                    exact_dynamic_class: _,
                    span: _,
                } => map_storage_use(mapper, site, MirStorageUseRole::Alias, owner),
            }
        }

    };
}

pub(super) use define_place_traversal;
