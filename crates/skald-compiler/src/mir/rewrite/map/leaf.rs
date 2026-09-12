//! Typed identity leaves and their semantic role dispatch.

macro_rules! define_identity_leaf_traversal {
    (($($mir_mutability:tt)*), $leaf:ident) => {
        fn map_optional_storage<M: MirLocalIdentityMapper>(
            mapper: &mut M,
            site: MirLocalIdentitySite,
            role: MirStorageUseRole,
            identity: &$($mir_mutability)* Option<StorageId>,
        ) -> Result<(), M::Error> {
            if let Some(identity) = identity {
                map_storage_use(mapper, site, role, identity)?;
            }
            Ok(())
        }

        fn map_storage_use<M: MirLocalIdentityMapper>(
            mapper: &mut M,
            site: MirLocalIdentitySite,
            role: MirStorageUseRole,
            identity: &$($mir_mutability)* StorageId,
        ) -> Result<(), M::Error> {
            $leaf!(storage_use, mapper, site, role, identity)
        }

        fn map_value<M: MirLocalIdentityMapper>(
            mapper: &mut M,
            site: MirLocalIdentitySite,
            identity: &$($mir_mutability)* ValueId,
        ) -> Result<(), M::Error> {
            $leaf!(value, mapper, site, identity)
        }

        fn map_value_use<M: MirLocalIdentityMapper>(
            mapper: &mut M,
            site: MirLocalIdentitySite,
            role: MirValueUseRole,
            identity: &$($mir_mutability)* ValueId,
        ) -> Result<(), M::Error> {
            $leaf!(value_use, mapper, site, role, identity)
        }

        fn map_value_definition<M: MirLocalIdentityMapper>(
            mapper: &mut M,
            site: MirLocalIdentitySite,
            identity: &$($mir_mutability)* ValueId,
        ) -> Result<(), M::Error> {
            $leaf!(value_definition, mapper, site, identity)
        }

        fn map_block<M: MirLocalIdentityMapper>(
            mapper: &mut M,
            site: MirLocalIdentitySite,
            identity: &$($mir_mutability)* BlockId,
        ) -> Result<(), M::Error> {
            $leaf!(block, mapper, site, identity)
        }

        fn map_path_condition<M: MirLocalIdentityMapper>(
            mapper: &mut M,
            site: MirLocalIdentitySite,
            identity: &$($mir_mutability)* PathConditionId,
        ) -> Result<(), M::Error> {
            $leaf!(path_condition, mapper, site, identity)
        }

        fn map_optional_guard<M: MirLocalIdentityMapper>(
            mapper: &mut M,
            site: MirLocalIdentitySite,
            identity: &$($mir_mutability)* OptionalGuardId,
        ) -> Result<(), M::Error> {
            $leaf!(optional_guard, mapper, site, identity)
        }
    };
}

pub(super) use define_identity_leaf_traversal;
