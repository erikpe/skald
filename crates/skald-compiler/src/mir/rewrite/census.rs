//! Exhaustive pass-local value declaration, definition, and use census.

use crate::{
    identity::CallableId,
    mir::{MirDefinitionRef, ValueId},
};

use super::{
    edit::MirCallableEdit, map::observe_body_local_identities, MirLocalIdentity,
    MirLocalIdentityObserver, MirLocalIdentitySite, MirReferenceFailure, MirRewriteError,
};

/// Definition and actual-use facts for one live callable-local value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MirValueCensusEntry {
    value: ValueId,
    definition: Option<MirLocalIdentitySite>,
    uses: usize,
}

impl MirValueCensusEntry {
    pub(crate) const fn value(self) -> ValueId {
        self.value
    }

    pub(crate) const fn definition(self) -> Option<MirLocalIdentitySite> {
        self.definition
    }

    pub(crate) const fn uses(self) -> usize {
        self.uses
    }
}

/// Deterministic value-indexed facts for one observed callable state.
///
/// The census is invalid after any rewrite. Callers must recompute it before
/// making another decision keyed by values or instruction positions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirValueUseCensus {
    callable: CallableId,
    entries: Vec<Option<MirValueCensusEntry>>,
}

impl MirValueUseCensus {
    pub(crate) const fn callable(&self) -> CallableId {
        self.callable
    }

    pub(crate) fn get(&self, value: ValueId) -> Option<&MirValueCensusEntry> {
        (value.callable() == self.callable)
            .then(|| self.entries.get(value.index()))
            .flatten()
            .and_then(Option::as_ref)
            .filter(|entry| entry.value == value)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &MirValueCensusEntry> {
        self.entries.iter().filter_map(Option::as_ref)
    }

    pub(crate) fn len(&self) -> usize {
        self.iter().count()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl MirCallableEdit {
    /// Computes read-only value facts through the exhaustive identity observer.
    pub(crate) fn value_use_census(&self) -> Result<MirValueUseCensus, MirRewriteError> {
        let mut entries = vec![None; self.allocated_value_slots()];
        for value in self.value_ids() {
            entries[value.index()] = Some(MirValueCensusEntry {
                value,
                definition: None,
                uses: 0,
            });
        }

        let mut collector = ValueCensusCollector {
            callable: self.callable(),
            entries,
        };
        self.observe_live_references(&mut collector)?;
        Ok(MirValueUseCensus {
            callable: collector.callable,
            entries: collector.entries,
        })
    }
}

/// Computes value facts for one dense, read-only executable definition.
///
/// This is the analysis-side entry point for passes that must preserve the
/// verified seal when they have no work. It borrows dense MIR directly and
/// shares the same exhaustive structural traversal used by rewriting.
pub(crate) fn value_use_census_for_definition(
    definition: MirDefinitionRef<'_>,
) -> Result<MirValueUseCensus, MirRewriteError> {
    let callable = definition.callable();
    let mut entries = Vec::with_capacity(definition.values().len());
    for (index, declaration) in definition.values().iter().enumerate() {
        let value = declaration.id;
        if value.callable() != callable {
            return Err(MirRewriteError::ForeignIdentity {
                expected: callable,
                identity: MirLocalIdentity::Value(value),
            });
        }
        let expected = ValueId::new(callable, index);
        if value != expected {
            return Err(MirRewriteError::DeclarationIdentityMismatch {
                expected: MirLocalIdentity::Value(expected),
                actual: MirLocalIdentity::Value(value),
            });
        }
        entries.push(Some(MirValueCensusEntry {
            value,
            definition: None,
            uses: 0,
        }));
    }
    let mut collector = ValueCensusCollector { callable, entries };
    observe_body_local_identities(definition.body(), &mut collector)?;
    Ok(MirValueUseCensus {
        callable: collector.callable,
        entries: collector.entries,
    })
}

struct ValueCensusCollector {
    callable: CallableId,
    entries: Vec<Option<MirValueCensusEntry>>,
}

impl ValueCensusCollector {
    fn entry_mut(
        &mut self,
        site: MirLocalIdentitySite,
        value: ValueId,
    ) -> Result<&mut MirValueCensusEntry, MirRewriteError> {
        if value.callable() != self.callable {
            return Err(MirRewriteError::InvalidReference {
                expected: self.callable,
                identity: MirLocalIdentity::Value(value),
                site,
                failure: MirReferenceFailure::Foreign,
            });
        }
        match self.entries.get_mut(value.index()) {
            Some(Some(entry)) => Ok(entry),
            Some(None) => Err(MirRewriteError::InvalidReference {
                expected: self.callable,
                identity: MirLocalIdentity::Value(value),
                site,
                failure: MirReferenceFailure::Deleted,
            }),
            None => Err(MirRewriteError::InvalidReference {
                expected: self.callable,
                identity: MirLocalIdentity::Value(value),
                site,
                failure: MirReferenceFailure::Unknown,
            }),
        }
    }
}

impl MirLocalIdentityObserver for ValueCensusCollector {
    type Error = MirRewriteError;

    fn observe_value(
        &mut self,
        site: MirLocalIdentitySite,
        value: ValueId,
    ) -> Result<(), Self::Error> {
        self.entry_mut(site, value)?.uses += 1;
        Ok(())
    }

    fn observe_value_definition(
        &mut self,
        site: MirLocalIdentitySite,
        value: ValueId,
    ) -> Result<(), Self::Error> {
        let entry = self.entry_mut(site, value)?;
        if let Some(first) = entry.definition {
            return Err(MirRewriteError::DuplicateValueDefinition {
                value,
                first,
                duplicate: site,
            });
        }
        entry.definition = Some(site);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
