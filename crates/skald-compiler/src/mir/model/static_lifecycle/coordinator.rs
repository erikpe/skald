//! Structured activation and destruction coordinator representation.

use crate::{identity::StaticFieldId, source::Span};

use super::super::{
    MirAggregateOptionalCleanup, MirArrayInstruction, MirClassOptionalCleanup, MirCleanup,
    MirOptionalSharedCleanup, MirOptionalStorage, MirOptionalTypeTable, MirPlace, MirSharedTarget,
    MirStaticInitializerBody, MirType,
};
use super::{MirProgramLifecycle, MirStaticLifecycleTransition};

/// Value work performed at one planned activation position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirStaticActivationWork {
    ZeroDefault,
    Explicit(crate::identity::StaticInitializerId),
}

/// One exact activation region in coordinator execution order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStaticActivationRegion {
    pub field: StaticFieldId,
    pub work: MirStaticActivationWork,
    /// A zero-default activation has one direct-to-live transition. Explicit
    /// initialization has begin and publish transitions in that order.
    pub transitions: Vec<MirStaticLifecycleTransition>,
}

/// Static shared-owner cleanup uses an ordinary live static place, unlike a
/// local `MirSharedRelease`, whose owner is addressed by `StorageId`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStaticSharedCleanup {
    pub destination: MirPlace,
    pub target: MirSharedTarget,
    pub span: Span,
}

/// Exact cleanup semantics for the current value of one live static slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirStaticValueCleanup {
    None,
    CompleteObject(MirCleanup),
    OptionalClass(MirClassOptionalCleanup),
    Shared(MirStaticSharedCleanup),
    OptionalShared(MirOptionalSharedCleanup),
    AggregateOptional(MirAggregateOptionalCleanup),
    Array(MirArrayInstruction),
}

impl MirStaticValueCleanup {
    pub(crate) fn for_field(
        optional_types: &MirOptionalTypeTable,
        ty: MirType,
        field: StaticFieldId,
        span: Span,
    ) -> Option<Self> {
        let destination = MirPlace::static_field(field);
        Some(match ty {
            MirType::Class(target) => Self::CompleteObject(MirCleanup {
                destination,
                target,
                span,
            }),
            MirType::Shared(target) => Self::Shared(MirStaticSharedCleanup {
                destination,
                target,
                span,
            }),
            MirType::Optional(optional) => {
                let metadata = optional_types.get(optional)?;
                match metadata.storage {
                    MirOptionalStorage::Scalar => Self::None,
                    MirOptionalStorage::InlineClass(class) => {
                        Self::OptionalClass(MirClassOptionalCleanup {
                            optional,
                            destination,
                            class,
                            span,
                        })
                    }
                    MirOptionalStorage::SharedOwner(target) => {
                        Self::OptionalShared(MirOptionalSharedCleanup {
                            optional,
                            destination,
                            target,
                            span,
                        })
                    }
                    MirOptionalStorage::Nested(_) | MirOptionalStorage::InlineArray(_) => {
                        Self::AggregateOptional(MirAggregateOptionalCleanup {
                            optional,
                            destination,
                            span,
                        })
                    }
                }
            }
            MirType::Array(array) => Self::Array(MirArrayInstruction::Release {
                owner: destination,
                array,
                span,
            }),
            MirType::I64
            | MirType::U64
            | MirType::U8
            | MirType::F64
            | MirType::Bool
            | MirType::Function(_) => Self::None,
            MirType::Interface(_) | MirType::Obj | MirType::Unit => return None,
        })
    }
}

/// One exact destruction region in reverse activation order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStaticDestructionRegion {
    pub field: StaticFieldId,
    pub begin: MirStaticLifecycleTransition,
    pub cleanup: MirStaticValueCleanup,
    pub finish: MirStaticLifecycleTransition,
}

/// Final program-owned lifecycle code and the compact proof that justifies it.
///
/// Initializer bodies remain independently identified CFGs so their existing
/// storage/value/block IDs and full-expression order never need rewriting.
/// Activation regions place their publication transition on the body's
/// checked publication edge; the next region begins only after that body has
/// completed its post-publication cleanup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStaticLifecycleCoordinator {
    lifecycle: MirProgramLifecycle,
    initializers: Vec<MirStaticInitializerBody>,
    activation: Vec<MirStaticActivationRegion>,
    shutdown: Vec<MirStaticDestructionRegion>,
}

impl MirStaticLifecycleCoordinator {
    pub(crate) fn new(
        lifecycle: MirProgramLifecycle,
        initializers: Vec<MirStaticInitializerBody>,
        activation: Vec<MirStaticActivationRegion>,
        shutdown: Vec<MirStaticDestructionRegion>,
    ) -> Self {
        Self {
            lifecycle,
            initializers,
            activation,
            shutdown,
        }
    }

    pub fn lifecycle(&self) -> &MirProgramLifecycle {
        &self.lifecycle
    }

    pub fn initializers(&self) -> &[MirStaticInitializerBody] {
        &self.initializers
    }

    pub fn activation(&self) -> &[MirStaticActivationRegion] {
        &self.activation
    }

    pub fn shutdown(&self) -> &[MirStaticDestructionRegion] {
        &self.shutdown
    }

    #[cfg(test)]
    pub(crate) fn activation_mut_for_test(&mut self) -> &mut Vec<MirStaticActivationRegion> {
        &mut self.activation
    }

    #[cfg(test)]
    pub(crate) fn shutdown_mut_for_test(&mut self) -> &mut Vec<MirStaticDestructionRegion> {
        &mut self.shutdown
    }

    #[cfg(test)]
    pub(crate) fn initializers_mut_for_test(&mut self) -> &mut Vec<MirStaticInitializerBody> {
        &mut self.initializers
    }

    #[cfg(test)]
    pub(crate) fn lifecycle_mut_for_test(&mut self) -> &mut MirProgramLifecycle {
        &mut self.lifecycle
    }
}
