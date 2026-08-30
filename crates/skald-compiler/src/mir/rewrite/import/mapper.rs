//! Exhaustive source-to-destination identity mapping during import.

use crate::identity::CallableId;
use crate::mir::{BlockId, OptionalGuardId, PathConditionId, StorageId, ValueId};

use super::super::{
    error::{MirReferenceFailure, MirRewriteError},
    identity::MirLocalId,
    MirLocalIdentityMapper, MirLocalIdentitySite,
};
use super::model::{MirImportMap, MirImportMaps};

pub(super) struct RehomeMapper<'maps> {
    source: CallableId,
    maps: &'maps MirImportMaps,
}

impl<'maps> RehomeMapper<'maps> {
    pub(super) const fn new(source: CallableId, maps: &'maps MirImportMaps) -> Self {
        Self { source, maps }
    }

    pub(super) fn storage(
        &self,
        site: MirLocalIdentitySite,
        identity: StorageId,
    ) -> Result<StorageId, MirRewriteError> {
        import_reference(self.source, &self.maps.storage, site, identity)
    }

    pub(super) fn block(
        &self,
        site: MirLocalIdentitySite,
        identity: BlockId,
    ) -> Result<BlockId, MirRewriteError> {
        import_reference(self.source, &self.maps.blocks, site, identity)
    }

    pub(super) fn path_condition(
        &self,
        site: MirLocalIdentitySite,
        identity: PathConditionId,
    ) -> Result<PathConditionId, MirRewriteError> {
        import_reference(self.source, &self.maps.path_conditions, site, identity)
    }
}

impl MirLocalIdentityMapper for RehomeMapper<'_> {
    type Error = MirRewriteError;

    fn map_storage(
        &mut self,
        site: MirLocalIdentitySite,
        identity: StorageId,
    ) -> Result<StorageId, Self::Error> {
        self.storage(site, identity)
    }

    fn map_value(
        &mut self,
        site: MirLocalIdentitySite,
        identity: ValueId,
    ) -> Result<ValueId, Self::Error> {
        import_reference(self.source, &self.maps.values, site, identity)
    }

    fn map_block(
        &mut self,
        site: MirLocalIdentitySite,
        identity: BlockId,
    ) -> Result<BlockId, Self::Error> {
        self.block(site, identity)
    }

    fn map_path_condition(
        &mut self,
        site: MirLocalIdentitySite,
        identity: PathConditionId,
    ) -> Result<PathConditionId, Self::Error> {
        self.path_condition(site, identity)
    }

    fn map_optional_guard(
        &mut self,
        site: MirLocalIdentitySite,
        identity: OptionalGuardId,
    ) -> Result<OptionalGuardId, Self::Error> {
        import_reference(self.source, &self.maps.optional_guards, site, identity)
    }
}

fn import_reference<I: MirLocalId>(
    expected_source: CallableId,
    map: &MirImportMap<I>,
    site: MirLocalIdentitySite,
    identity: I,
) -> Result<I, MirRewriteError> {
    if identity.callable() != expected_source {
        return Err(MirRewriteError::InvalidReference {
            expected: expected_source,
            identity: identity.local_identity(),
            site,
            failure: MirReferenceFailure::Foreign,
        });
    }
    map.entries
        .get(&identity)
        .copied()
        .ok_or(MirRewriteError::MissingImportSubstitution {
            identity: identity.local_identity(),
            site,
        })
}
