//! Closed compiler evidence for canonical integer successor applications.

use super::{
    GenericInterfaceSpecializationTable, ResolvedPrimitiveBoundOperation, ResolvedPrimitiveType,
    ResolvedProgram, ResolvedTypeKind,
};
use crate::identity::{InterfaceId, InterfaceTemplateId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedPrimitiveSuccessorEvidence {
    primitive: ResolvedPrimitiveType,
}

impl ResolvedPrimitiveSuccessorEvidence {
    pub const fn primitive(self) -> ResolvedPrimitiveType {
        self.primitive
    }

    pub const fn operation(self) -> ResolvedPrimitiveBoundOperation {
        ResolvedPrimitiveBoundOperation::Successor(self.primitive)
    }
}

const PRIMITIVE_SUCCESSOR_REGISTRY: [ResolvedPrimitiveSuccessorEvidence; 3] = [
    evidence(ResolvedPrimitiveType::U8),
    evidence(ResolvedPrimitiveType::U64),
    evidence(ResolvedPrimitiveType::I64),
];

const fn evidence(primitive: ResolvedPrimitiveType) -> ResolvedPrimitiveSuccessorEvidence {
    ResolvedPrimitiveSuccessorEvidence { primitive }
}

pub(crate) fn primitive_successor_registry() -> &'static [ResolvedPrimitiveSuccessorEvidence] {
    debug_assert!(validate_primitive_successor_registry(&PRIMITIVE_SUCCESSOR_REGISTRY).is_ok());
    &PRIMITIVE_SUCCESSOR_REGISTRY
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrimitiveSuccessorRegistryError {
    WrongEntryCount { actual: usize },
    DuplicatePrimitive { first: usize, duplicate: usize },
    UnsupportedPrimitive { index: usize },
}

fn validate_primitive_successor_registry(
    registry: &[ResolvedPrimitiveSuccessorEvidence],
) -> Result<(), PrimitiveSuccessorRegistryError> {
    if registry.len() != PRIMITIVE_SUCCESSOR_REGISTRY.len() {
        return Err(PrimitiveSuccessorRegistryError::WrongEntryCount {
            actual: registry.len(),
        });
    }
    for (index, entry) in registry.iter().enumerate() {
        if !supports_successor(entry.primitive) {
            return Err(PrimitiveSuccessorRegistryError::UnsupportedPrimitive { index });
        }
        if let Some(first) = registry[..index]
            .iter()
            .position(|previous| previous.primitive == entry.primitive)
        {
            return Err(PrimitiveSuccessorRegistryError::DuplicatePrimitive {
                first,
                duplicate: index,
            });
        }
    }
    Ok(())
}

const fn supports_successor(primitive: ResolvedPrimitiveType) -> bool {
    matches!(
        primitive,
        ResolvedPrimitiveType::U8 | ResolvedPrimitiveType::U64 | ResolvedPrimitiveType::I64
    )
}

/// Finds evidence only for `T: std::range::Successor<T>` where `T` is one of
/// the three compiler-supported integer primitives.
pub(crate) fn primitive_successor_evidence(
    program: &ResolvedProgram,
    receiver: ResolvedTypeKind,
    interface: InterfaceId,
) -> Option<ResolvedPrimitiveSuccessorEvidence> {
    let language_item = program.range_language_item.as_ref()?;
    primitive_successor_operation(
        receiver,
        interface,
        language_item.successor_template,
        &program.generic_interface_specializations,
    )?;
    primitive_successor_registry()
        .iter()
        .copied()
        .find(|evidence| ResolvedTypeKind::from(evidence.primitive) == receiver)
}

pub(crate) fn primitive_successor_operation(
    receiver: ResolvedTypeKind,
    interface: InterfaceId,
    successor_template: InterfaceTemplateId,
    applications: &GenericInterfaceSpecializationTable,
) -> Option<ResolvedPrimitiveBoundOperation> {
    let application = applications.for_interface(interface)?;
    if application.key.template != successor_template
        || application.key.arguments.as_slice() != [receiver]
    {
        return None;
    }
    primitive_successor_registry()
        .iter()
        .find(|entry| ResolvedTypeKind::from(entry.primitive) == receiver)
        .map(|entry| entry.operation())
}

pub(crate) fn canonical_successor_application(
    program: &ResolvedProgram,
    interface: InterfaceId,
) -> bool {
    let Some(language_item) = program.range_language_item.as_ref() else {
        return false;
    };
    program
        .generic_interface_specializations
        .for_interface(interface)
        .is_some_and(|application| application.key.template == language_item.successor_template)
}

#[cfg(test)]
mod tests;
