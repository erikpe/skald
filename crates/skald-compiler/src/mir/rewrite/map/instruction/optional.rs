//! Optional values, aggregate optionals, guarded views, and shared optionals.

macro_rules! define_optional_instruction_traversal {
    (($($mir_mutability:tt)*)) => {
        fn map_optional_source<M: MirLocalIdentityMapper>(
            source: &$($mir_mutability)* MirOptionalSource,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            match source {
                MirOptionalSource::Absent => Ok(()),
                MirOptionalSource::Present(value) => map_value_use(
                    mapper,
                    site,
                    MirValueUseRole::OwnershipOrLifecycle,
                    value,
                ),
                MirOptionalSource::Copy(place) => map_place(
                    place,
                    mapper,
                    site,
                    MirPlaceUseContext::OwnershipOrLifecycle,
                ),
            }
        }

        fn map_optional_initialize<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirOptionalInitialize,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirOptionalInitialize {
                destination,
                source,
                span: _,
            } = instruction;
            map_place(
                destination,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )?;
            map_optional_source(source, mapper, site)
        }

        fn map_optional_assign<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirOptionalAssign,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirOptionalAssign {
                destination,
                source,
                authorization: _,
                final_authorization: _,
                span: _,
            } = instruction;
            map_place(
                destination,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )?;
            map_optional_source(source, mapper, site)
        }

        fn map_aggregate_optional_source<M: MirLocalIdentityMapper>(
            source: &$($mir_mutability)* MirAggregateOptionalSource,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            match source {
                MirAggregateOptionalSource::Absent | MirAggregateOptionalSource::Unpublished => Ok(()),
                MirAggregateOptionalSource::Copy(place) => map_place(
                    place,
                    mapper,
                    site,
                    MirPlaceUseContext::OwnershipOrLifecycle,
                ),
            }
        }

        fn map_aggregate_optional_initialize<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirAggregateOptionalInitialize,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirAggregateOptionalInitialize {
                optional: _,
                destination,
                source,
                span: _,
            } = instruction;
            map_place(
                destination,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )?;
            map_aggregate_optional_source(source, mapper, site)
        }

        fn map_aggregate_optional_assign<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirAggregateOptionalAssign,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirAggregateOptionalAssign {
                optional: _,
                destination,
                source,
                authorization: _,
                final_authorization: _,
                span: _,
            } = instruction;
            map_place(
                destination,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )?;
            map_aggregate_optional_source(source, mapper, site)
        }

        fn map_class_optional_source<M: MirLocalIdentityMapper>(
            source: &$($mir_mutability)* MirClassOptionalSource,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            match source {
                MirClassOptionalSource::Absent => Ok(()),
                MirClassOptionalSource::Present(place) | MirClassOptionalSource::Copy(place) => {
                    map_place(
                        place,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )
                }
            }
        }

        fn map_class_optional_initialize<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirClassOptionalInitialize,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirClassOptionalInitialize {
                optional: _,
                destination,
                source,
                class: _,
                copy_constructor: _,
                span: _,
            } = instruction;
            map_place(
                destination,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )?;
            map_class_optional_source(source, mapper, site)
        }

        fn map_class_optional_assign<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirClassOptionalAssign,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirClassOptionalAssign {
                optional: _,
                destination,
                source,
                class: _,
                copy_constructor: _,
                copy_assignment: _,
                authorization: _,
                final_authorization: _,
                span: _,
            } = instruction;
            map_place(
                destination,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )?;
            map_class_optional_source(source, mapper, site)
        }

        fn map_optional_view_begin<M: MirLocalIdentityMapper>(
            begin: &$($mir_mutability)* MirOptionalViewBegin,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirOptionalViewBegin {
                optional: _,
                guard,
                source,
                payload: _,
                span: _,
            } = begin;
            map_optional_guard(mapper, site, guard)?;
            map_place(
                source,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )
        }

        fn map_optional_view_end<M: MirLocalIdentityMapper>(
            end: &$($mir_mutability)* MirOptionalViewEnd,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirOptionalViewEnd {
                optional: _,
                guard,
                source,
                payload: _,
                span: _,
            } = end;
            map_optional_guard(mapper, site, guard)?;
            map_place(
                source,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )
        }

        fn map_optional_box_view_begin<M: MirLocalIdentityMapper>(
            begin: &$($mir_mutability)* MirOptionalBoxViewBegin,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirOptionalBoxViewBegin {
                box_target: _,
                layer: _,
                guard,
                owner,
                span: _,
            } = begin;
            map_optional_guard(mapper, site, guard)?;
            map_storage_use(mapper, site, MirStorageUseRole::Alias, owner)
        }

        fn map_optional_box_view_end<M: MirLocalIdentityMapper>(
            end: &$($mir_mutability)* MirOptionalBoxViewEnd,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirOptionalBoxViewEnd {
                box_target: _,
                layer: _,
                guard,
                owner,
                span: _,
            } = end;
            map_optional_guard(mapper, site, guard)?;
            map_storage_use(mapper, site, MirStorageUseRole::Alias, owner)
        }

        fn map_optional_shared_source<M: MirLocalIdentityMapper>(
            source: &$($mir_mutability)* MirOptionalSharedSource,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            match source {
                MirOptionalSharedSource::Absent => Ok(()),
                MirOptionalSharedSource::Present(owner) | MirOptionalSharedSource::Move(owner) => {
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, owner)
                }
                MirOptionalSharedSource::Copy(place) => map_place(
                    place,
                    mapper,
                    site,
                    MirPlaceUseContext::OwnershipOrLifecycle,
                ),
            }
        }

        fn map_optional_shared_initialize<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirOptionalSharedInitialize,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirOptionalSharedInitialize {
                optional: _,
                destination,
                source,
                target: _,
                span: _,
            } = instruction;
            map_place(
                destination,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )?;
            map_optional_shared_source(source, mapper, site)
        }

        fn map_optional_shared_assign<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirOptionalSharedAssign,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirOptionalSharedAssign {
                optional: _,
                destination,
                source,
                target: _,
                authorization: _,
                final_authorization: _,
                span: _,
            } = instruction;
            map_place(
                destination,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )?;
            map_optional_shared_source(source, mapper, site)
        }

    };
}

pub(in crate::mir::rewrite::map) use define_optional_instruction_traversal;
