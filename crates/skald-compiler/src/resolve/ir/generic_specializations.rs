//! Deterministic identities and provenance for closed generic-class requests.

use crate::{
    identity::{ClassId, ClassTemplateId, InterfaceId, InterfaceRequirementId, ModuleId},
    source::Span,
};

use super::{GenericInterfaceInstanceKey, ResolvedSharedTarget, ResolvedTypeKind};

/// Canonical identity input for one closed generic class.
///
/// Source spelling and spans are deliberately excluded from equality. Every
/// compound argument has already been interned into its ordinary semantic ID.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct GenericClassInstanceKey {
    pub(crate) template: ClassTemplateId,
    pub(crate) arguments: Vec<ResolvedTypeKind>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum GenericSpecializationKey {
    Class(GenericClassInstanceKey),
    Interface(GenericInterfaceInstanceKey),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GenericSpecializationState {
    Requested,
    InProgress(ClassId),
    Complete(ClassId),
    Failed { reserved_class: Option<ClassId> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GenericSpecializationTransition {
    Requested,
    InProgress(ClassId),
    Complete(ClassId),
    Failed { reserved_class: Option<ClassId> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GenericApplicationOrigin {
    pub(crate) module: ModuleId,
    pub(crate) span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GenericSpecializationProvenance {
    pub(crate) template_span: Span,
    pub(crate) origins: Vec<GenericApplicationOrigin>,
    pub(crate) recursion_path: Vec<GenericSpecializationKey>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GenericSpecialization {
    pub(crate) key: GenericClassInstanceKey,
    pub(crate) state: GenericSpecializationState,
    pub(crate) transitions: Vec<GenericSpecializationTransition>,
    pub(crate) provenance: GenericSpecializationProvenance,
    pub(crate) closed_type_uses: Vec<Option<ResolvedTypeKind>>,
    pub(crate) closed_requirements: Vec<Option<ClosedGenericRequirementSubject>>,
    pub(crate) closed_interface_claims: Vec<Option<InterfaceId>>,
    pub(crate) closed_interface_bounds: Vec<Option<InterfaceId>>,
    pub(crate) closed_bound_members: Vec<Option<ClosedGenericBoundMember>>,
    pub(crate) closed_iteration_selections: Vec<Option<ClosedGenericIterationSelection>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ClosedGenericBoundMember {
    pub(crate) interface: InterfaceId,
    pub(crate) requirement: InterfaceRequirementId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ClosedGenericIterationSelection {
    pub(crate) interface: InterfaceId,
    pub(crate) iter_state: InterfaceRequirementId,
    pub(crate) iter_next: InterfaceRequirementId,
    pub(crate) item: ResolvedTypeKind,
    pub(crate) state: ResolvedTypeKind,
    pub(crate) origin_span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClosedGenericRequirementSubject {
    Type(ResolvedTypeKind),
    SharedTarget(ResolvedSharedTarget),
}

impl GenericSpecialization {
    pub(crate) const fn class(&self) -> Option<ClassId> {
        match self.state {
            GenericSpecializationState::Requested => None,
            GenericSpecializationState::InProgress(class)
            | GenericSpecializationState::Complete(class) => Some(class),
            GenericSpecializationState::Failed { reserved_class } => reserved_class,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct GenericSpecializationTable {
    entries: Vec<GenericSpecialization>,
}

impl GenericSpecializationTable {
    pub(crate) fn new(entries: Vec<GenericSpecialization>) -> Self {
        Self { entries }
    }

    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &GenericSpecialization> {
        self.entries.iter()
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut GenericSpecialization> {
        self.entries.iter_mut()
    }

    /// Used by declaration specialization to publish the complete class behind
    /// an already allocated specialization identity.
    #[allow(dead_code)]
    pub(crate) fn for_class(&self, class: ClassId) -> Option<&GenericSpecialization> {
        self.entries
            .iter()
            .find(|entry| entry.class() == Some(class))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn class_at_application(&self, module: ModuleId, span: Span) -> Option<ClassId> {
        self.entries
            .iter()
            .find(|entry| {
                entry
                    .provenance
                    .origins
                    .contains(&GenericApplicationOrigin { module, span })
            })
            .and_then(|entry| match entry.state {
                GenericSpecializationState::Complete(class) => Some(class),
                GenericSpecializationState::Requested
                | GenericSpecializationState::InProgress(_)
                | GenericSpecializationState::Failed { .. } => None,
            })
    }

    pub(crate) fn fail_class(&mut self, class: ClassId) {
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.class() == Some(class))
        else {
            return;
        };
        if matches!(entry.state, GenericSpecializationState::Failed { .. }) {
            return;
        }
        entry.state = GenericSpecializationState::Failed {
            reserved_class: Some(class),
        };
        entry
            .transitions
            .push(GenericSpecializationTransition::Failed {
                reserved_class: Some(class),
            });
    }
}
