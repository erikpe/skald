//! Source-callable activation classification and initial locations.

use std::collections::BTreeMap;

use crate::{backend::BackendError, identity::CallableId, mir::MirProgram};

use super::Metadata;

/// Initial trace locations for source-authored executable bodies.
///
/// Constructing this table from `executable_definitions` is the eligibility
/// boundary: target-generated helpers and wrappers never enter the table and
/// therefore cannot acquire trace frames.
pub(in crate::backend::x86_64_sysv) struct Activations {
    locations: BTreeMap<CallableId, String>,
}

impl Activations {
    pub(in crate::backend::x86_64_sysv) fn plan(
        program: &MirProgram,
        metadata: &Metadata<'_>,
    ) -> Result<Self, BackendError> {
        let mut locations = BTreeMap::new();
        for definition in program.executable_definitions() {
            if let Some(location) =
                metadata.request_location(definition.callable(), definition.span())?
            {
                locations.insert(definition.callable(), location);
            }
        }
        Ok(Self { locations })
    }

    pub(in crate::backend::x86_64_sysv) fn initial_location(
        &self,
        callable: CallableId,
    ) -> Option<&str> {
        self.locations.get(&callable).map(String::as_str)
    }
}
