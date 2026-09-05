//! Exhaustive semantic use sites for callable-local storage.

use crate::mir::StorageId;

use super::{MirLocalIdentitySite, MirRewriteError};

/// Shape of a place access rooted in callable-local storage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirStoragePlaceUse {
    ExactBase,
    Projected,
    Alias,
}

/// Authorization carried by an ordinary scalar store.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirStorageWriteAuthorization {
    None,
    Cell,
    Final,
    CellAndFinal,
}

/// Closed semantic classification of a callable-local storage occurrence.
///
/// The categories are deliberately conservative. Only exact ordinary reads,
/// exact unauthorized ordinary writes, lifecycle markers, and checked-
/// protocol ownership may participate in scalar carrier certification.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirStorageUseRole {
    Declaration,
    Attachment,
    LifetimeLive,
    LifetimeDead,
    OrdinaryRead(MirStoragePlaceUse),
    OrdinaryWrite {
        place: MirStoragePlaceUse,
        authorization: MirStorageWriteAuthorization,
    },
    CheckedProtocol,
    ProofMetadata,
    Alias,
    Call,
    OwnershipOrLifecycle,
    InputOutput,
    OtherExecutable,
}

/// One deterministic classified occurrence of a storage identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MirStorageUseSite {
    site: MirLocalIdentitySite,
    role: MirStorageUseRole,
}

impl MirStorageUseSite {
    pub(crate) const fn new(site: MirLocalIdentitySite, role: MirStorageUseRole) -> Self {
        Self { site, role }
    }

    pub(crate) const fn site(self) -> MirLocalIdentitySite {
        self.site
    }

    pub(crate) const fn role(self) -> MirStorageUseRole {
        self.role
    }
}

/// Classified uses of one storage declaration in dense declaration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirStorageUseCensusEntry {
    storage: StorageId,
    declaration: MirLocalIdentitySite,
    uses: Vec<MirStorageUseSite>,
}

impl MirStorageUseCensusEntry {
    pub(crate) const fn storage(&self) -> StorageId {
        self.storage
    }

    pub(crate) const fn declaration(&self) -> MirLocalIdentitySite {
        self.declaration
    }

    pub(crate) fn uses(&self) -> &[MirStorageUseSite] {
        &self.uses
    }
}

/// Seal-local, deterministic storage-access inventory for one callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirStorageUseCensus {
    callable: crate::identity::CallableId,
    entries: Vec<MirStorageUseCensusEntry>,
}

impl MirStorageUseCensus {
    pub(crate) fn get(&self, storage: StorageId) -> Option<&MirStorageUseCensusEntry> {
        (storage.callable() == self.callable)
            .then(|| self.entries.get(storage.index()))
            .flatten()
            .filter(|entry| entry.storage == storage)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &MirStorageUseCensusEntry> {
        self.entries.iter()
    }
}

pub(crate) fn storage_use_census_for_definition(
    definition: crate::mir::MirDefinitionRef<'_>,
) -> Result<MirStorageUseCensus, MirRewriteError> {
    use super::{map::observe_definition_local_identities, MirLocalIdentity};

    let callable = definition.callable();
    let mut entries = Vec::with_capacity(definition.storage_entries().len());
    for (index, declaration) in definition.storage_entries().iter().enumerate() {
        let expected = StorageId::new(callable, index);
        if declaration.id.callable() != callable {
            return Err(MirRewriteError::ForeignIdentity {
                expected: callable,
                identity: MirLocalIdentity::Storage(declaration.id),
            });
        }
        if declaration.id != expected {
            return Err(MirRewriteError::DeclarationIdentityMismatch {
                expected: MirLocalIdentity::Storage(expected),
                actual: MirLocalIdentity::Storage(declaration.id),
            });
        }
        entries.push(MirStorageUseCensusEntry {
            storage: declaration.id,
            declaration: MirLocalIdentitySite::StorageDeclaration(index),
            uses: Vec::new(),
        });
    }

    let mut collector = StorageUseCollector { callable, entries };
    observe_definition_local_identities(definition, &mut collector)?;
    Ok(MirStorageUseCensus {
        callable,
        entries: collector.entries,
    })
}

struct StorageUseCollector {
    callable: crate::identity::CallableId,
    entries: Vec<MirStorageUseCensusEntry>,
}

impl super::MirLocalIdentityObserver for StorageUseCollector {
    type Error = MirRewriteError;

    fn observe_storage(
        &mut self,
        site: MirLocalIdentitySite,
        identity: StorageId,
    ) -> Result<(), Self::Error> {
        Err(MirRewriteError::UnclassifiedStorageReference {
            storage: identity,
            site,
        })
    }

    fn observe_storage_use(
        &mut self,
        site: MirLocalIdentitySite,
        role: MirStorageUseRole,
        identity: StorageId,
    ) -> Result<(), Self::Error> {
        use super::{MirLocalIdentity, MirReferenceFailure};

        if identity.callable() != self.callable {
            return Err(MirRewriteError::InvalidReference {
                expected: self.callable,
                identity: MirLocalIdentity::Storage(identity),
                site,
                failure: MirReferenceFailure::Foreign,
            });
        }
        let Some(entry) = self.entries.get_mut(identity.index()) else {
            return Err(MirRewriteError::InvalidReference {
                expected: self.callable,
                identity: MirLocalIdentity::Storage(identity),
                site,
                failure: MirReferenceFailure::Unknown,
            });
        };
        if role == MirStorageUseRole::Declaration {
            if site != entry.declaration {
                return Err(MirRewriteError::DeclarationIdentityMismatch {
                    expected: MirLocalIdentity::Storage(entry.storage),
                    actual: MirLocalIdentity::Storage(identity),
                });
            }
        } else {
            entry.uses.push(MirStorageUseSite::new(site, role));
        }
        Ok(())
    }
}
