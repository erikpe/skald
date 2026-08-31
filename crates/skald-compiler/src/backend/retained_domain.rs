//! Narrow projection of verified target-independent reachability for targets.

use crate::{
    identity::{
        ArrayTypeId, ClassId, FunctionTypeId, InterfaceRequirementId, LiteralDataId,
        OptionalBoxTypeId, OptionalTypeId, StaticFieldId, VirtualFamilyId,
    },
    passes::reachability::MirRuntimeEntity,
};

use super::BackendInput;

/// Target-independent runtime obligation projected for backend planning.
///
/// This backend-owned vocabulary prevents target implementations from
/// depending on the reachability analysis representation or its pass policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackendRequiredRuntimeEntity {
    ClassDispatch(ClassId),
    VirtualFamily(VirtualFamilyId),
    InterfaceRequirement(InterfaceRequirementId),
    FunctionType(FunctionTypeId),
    ArrayLifecycle(ArrayTypeId),
    OptionalLifecycle(OptionalTypeId),
    OptionalBoxLayout(OptionalBoxTypeId),
    StaticStorage(StaticFieldId),
    LiteralBacking(LiteralDataId),
}

impl<'input> BackendInput<'input> {
    /// Iterates required target-independent runtime entities in canonical
    /// identity order without exposing the analysis product to a backend.
    pub(crate) fn required_runtime_entities(
        self,
    ) -> impl ExactSizeIterator<Item = BackendRequiredRuntimeEntity> + 'input {
        self.verified
            .reachability()
            .runtime_entities()
            .iter()
            .copied()
            .map(BackendRequiredRuntimeEntity::from)
    }

    pub(crate) fn uses_virtual_family(self, family: VirtualFamilyId) -> bool {
        self.verified
            .reachability()
            .used_virtual_families()
            .binary_search(&family)
            .is_ok()
    }

    pub(crate) fn uses_interface_requirement(self, requirement: InterfaceRequirementId) -> bool {
        self.verified
            .reachability()
            .used_interface_requirements()
            .binary_search(&requirement)
            .is_ok()
    }
}

impl From<MirRuntimeEntity> for BackendRequiredRuntimeEntity {
    fn from(entity: MirRuntimeEntity) -> Self {
        match entity {
            MirRuntimeEntity::ClassDispatch(class) => Self::ClassDispatch(class),
            MirRuntimeEntity::VirtualFamily(family) => Self::VirtualFamily(family),
            MirRuntimeEntity::InterfaceRequirement(requirement) => {
                Self::InterfaceRequirement(requirement)
            }
            MirRuntimeEntity::FunctionType(function_type) => Self::FunctionType(function_type),
            MirRuntimeEntity::ArrayLifecycle(array) => Self::ArrayLifecycle(array),
            MirRuntimeEntity::OptionalLifecycle(optional) => Self::OptionalLifecycle(optional),
            MirRuntimeEntity::OptionalBoxLayout(box_type) => Self::OptionalBoxLayout(box_type),
            MirRuntimeEntity::StaticStorage(field) => Self::StaticStorage(field),
            MirRuntimeEntity::LiteralBacking(data) => Self::LiteralBacking(data),
        }
    }
}
