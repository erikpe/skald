//! Canonical identities, cache states, and provenance for generic interfaces.

use crate::{
    identity::{InterfaceId, InterfaceTemplateId, ModuleId},
    source::Span,
};

use super::{GenericSpecializationKey, ResolvedTypeKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenericInterfaceApplicationOrigin {
    pub module: ModuleId,
    pub span: Span,
}

/// Canonical identity input for one closed generic interface.
///
/// Source spelling and spans are deliberately excluded from equality. Every
/// compound argument has already been interned into its ordinary semantic ID.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct GenericInterfaceInstanceKey {
    pub template: InterfaceTemplateId,
    pub arguments: Vec<ResolvedTypeKind>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenericInterfaceSpecializationState {
    Requested,
    InProgress(InterfaceId),
    Complete(InterfaceId),
    Failed { reserved_interface: InterfaceId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenericInterfaceSpecializationTransition {
    Requested,
    InProgress(InterfaceId),
    Complete(InterfaceId),
    Failed { reserved_interface: InterfaceId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericInterfaceSpecializationProvenance {
    pub template_span: Span,
    pub origins: Vec<GenericInterfaceApplicationOrigin>,
    pub(crate) recursion_path: Vec<GenericSpecializationKey>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericInterfaceSpecialization {
    pub key: GenericInterfaceInstanceKey,
    pub state: GenericInterfaceSpecializationState,
    pub transitions: Vec<GenericInterfaceSpecializationTransition>,
    pub provenance: GenericInterfaceSpecializationProvenance,
    pub(crate) closed_type_uses: Vec<Option<ResolvedTypeKind>>,
}

impl GenericInterfaceSpecialization {
    pub const fn interface(&self) -> Option<InterfaceId> {
        match self.state {
            GenericInterfaceSpecializationState::Requested => None,
            GenericInterfaceSpecializationState::InProgress(interface)
            | GenericInterfaceSpecializationState::Complete(interface) => Some(interface),
            GenericInterfaceSpecializationState::Failed { reserved_interface } => {
                Some(reserved_interface)
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GenericInterfaceSpecializationTable {
    entries: Vec<GenericInterfaceSpecialization>,
}

impl GenericInterfaceSpecializationTable {
    pub(crate) fn new(entries: Vec<GenericInterfaceSpecialization>) -> Self {
        Self { entries }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &GenericInterfaceSpecialization> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn for_interface(&self, interface: InterfaceId) -> Option<&GenericInterfaceSpecialization> {
        self.entries
            .iter()
            .find(|entry| entry.interface() == Some(interface))
    }
}
